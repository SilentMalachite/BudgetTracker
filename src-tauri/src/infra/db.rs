use std::path::Path;

use rusqlite::Connection;

use crate::error::{AppError, AppResult};
use crate::infra::keychain::{DbKey, KEY_LEN};

/// SQLCipher file-format generation every `data.db` is written and read with.
pub const SQLCIPHER_FORMAT_VERSION: i64 = 4;

fn key_to_hex(key: &DbKey) -> String {
    let mut s = String::with_capacity(KEY_LEN * 2);
    for b in key {
        use std::fmt::Write as _;
        let _ = write!(&mut s, "{:02x}", b);
    }
    s
}

/// Open a SQLCipher-encrypted SQLite database at `path`. Creates the file if
/// absent. The connection has `PRAGMA key` applied. Foreign keys are enabled.
pub fn open_encrypted(path: &Path, key: &DbKey) -> AppResult<Connection> {
    let conn = Connection::open(path)?;
    let hex = key_to_hex(key);
    // PRAGMA key with raw bytes form `x'<hex>'` is safe against SQL injection
    // because hex is from our own 32-byte buffer.
    conn.pragma_update(None, "key", format!("x'{hex}'"))?;
    // Pin the on-disk format. SQLCipher 3.x -> 4.x changed the KDF / HMAC /
    // page-size defaults; without this pin a future major bump would turn
    // every existing data.db into a "decrypt failed" recovery prompt even
    // though the key is intact. Moving to a newer format is a deliberate
    // step: open under this pin, run `PRAGMA cipher_migrate`, then raise
    // SQLCIPHER_FORMAT_VERSION.
    conn.pragma_update(None, "cipher_compatibility", SQLCIPHER_FORMAT_VERSION)?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    // Touch the database to force PRAGMA key to take effect on first use.
    conn.query_row("SELECT count(*) FROM sqlite_master", [], |_| Ok(()))?;
    // SELECT does not persist pages. A 0-byte file would later open under any
    // key as a new database, so VACUUM empty files to write the encrypted header.
    if path.metadata()?.len() == 0 {
        conn.execute_batch("VACUUM")?;
    }
    Ok(conn)
}

pub fn looks_like_decrypt_failure(err: &AppError) -> bool {
    let text = err.to_string().to_lowercase();
    text.contains("not a database")
        || text.contains("file is encrypted")
        || text.contains("hmac check failed")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_key(seed: u8) -> DbKey {
        let mut k = [0u8; KEY_LEN];
        for (i, slot) in k.iter_mut().enumerate() {
            *slot = seed.wrapping_add(i as u8);
        }
        k
    }

    #[test]
    fn round_trip_data_through_encrypted_db() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("data.db");
        let key = make_key(7);

        {
            let conn = open_encrypted(&path, &key).unwrap();
            conn.execute_batch(
                "CREATE TABLE t(id INTEGER PRIMARY KEY, v TEXT NOT NULL);
                 INSERT INTO t(v) VALUES('hello');",
            )
            .unwrap();
        }

        let conn = open_encrypted(&path, &key).unwrap();
        let v: String = conn
            .query_row("SELECT v FROM t WHERE id=1", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, "hello");
    }

    #[test]
    fn bundled_sqlcipher_matches_pinned_format_version() {
        let tmp = TempDir::new().unwrap();
        let conn = open_encrypted(&tmp.path().join("data.db"), &make_key(3)).unwrap();
        let version: String = conn
            .query_row("PRAGMA cipher_version", [], |r| r.get(0))
            .unwrap();
        assert!(
            version.starts_with(&format!("{SQLCIPHER_FORMAT_VERSION}.")),
            "bundled SQLCipher is {version}: existing databases need `PRAGMA cipher_migrate` \
             before SQLCIPHER_FORMAT_VERSION can move"
        );
    }

    #[test]
    fn wrong_key_fails_to_decrypt() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("data.db");
        let right = make_key(1);
        let wrong = make_key(2);

        {
            let conn = open_encrypted(&path, &right).unwrap();
            conn.execute_batch("CREATE TABLE t(id INTEGER); INSERT INTO t VALUES(1);")
                .unwrap();
        }

        let bad = open_encrypted(&path, &wrong);
        assert!(bad.is_err(), "opening with wrong key must fail");
        let err = bad.unwrap_err();
        assert!(
            looks_like_decrypt_failure(&err),
            "wrong key should look like decrypt failure: {err}"
        );
    }

    #[test]
    fn raw_file_does_not_contain_plaintext() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("data.db");
        let key = make_key(42);
        let conn = open_encrypted(&path, &key).unwrap();
        conn.execute_batch("CREATE TABLE t(v TEXT); INSERT INTO t(v) VALUES('SUPERSECRET_TOKEN');")
            .unwrap();
        drop(conn);
        let bytes = std::fs::read(&path).unwrap();
        assert!(
            !bytes.windows(17).any(|w| w == b"SUPERSECRET_TOKEN"),
            "plaintext leaked into encrypted db file"
        );
    }
}
