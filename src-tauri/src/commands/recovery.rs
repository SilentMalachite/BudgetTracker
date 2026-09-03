use std::path::{Path, PathBuf};
use std::sync::Mutex;

use rusqlite::Connection;
use serde::Serialize;
use tauri::State;

use crate::commands::backup::{self, ImportResult};
use crate::commands::meta::{AppInner, AppState};
use crate::domain::seed;
use crate::error::{AppError, AppResult};
use crate::infra::boot::{
    KeyStore, OsKeyStore, RecoveryReason, KEYCHAIN_ACCOUNT, KEYCHAIN_SERVICE,
};
use crate::infra::keychain::DbKey;
use crate::infra::{db, migrations};

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BootState {
    Ready,
    Recovery,
}

#[derive(Debug, Serialize)]
pub struct BootStatus {
    pub state: BootState,
    pub recovery_reason: Option<RecoveryReason>,
    pub db_path: String,
}

fn os_keys() -> OsKeyStore {
    OsKeyStore {
        service: KEYCHAIN_SERVICE.to_string(),
        account: KEYCHAIN_ACCOUNT.to_string(),
    }
}

fn lock_inner(inner: &Mutex<AppInner>) -> AppResult<std::sync::MutexGuard<'_, AppInner>> {
    inner
        .lock()
        .map_err(|_| AppError::Corrupt("connection mutex poisoned".into()))
}

fn obtain_recovery_key(keys: &dyn KeyStore) -> AppResult<DbKey> {
    match keys.get() {
        Ok(Some(key)) => Ok(key),
        Ok(None) => keys.create(),
        Err(AppError::Corrupt(_)) => {
            // The entry could not be decoded, but its bytes may still be the
            // only thing that decrypts the quarantined data.db. Park them
            // under a stamped entry before the slot is deleted and re-minted.
            keys.preserve_corrupt()?;
            keys.delete()?;
            keys.create()
        }
        Err(err) => Err(err),
    }
}

fn unique_quarantine_path(data_dir: &Path, now: chrono::DateTime<chrono::Utc>) -> PathBuf {
    let stamp = now.format("%Y%m%dT%H%M%SZ");
    let base = data_dir.join(format!("data.db.corrupt-{stamp}"));
    if !base.exists() {
        return base;
    }
    let mut n = 2u32;
    loop {
        let candidate = data_dir.join(format!("data.db.corrupt-{stamp}-{n}"));
        if !candidate.exists() {
            return candidate;
        }
        n = n.saturating_add(1);
        if n == u32::MAX {
            return candidate;
        }
    }
}

/// SQLite side files that must travel with `data.db`. A hot `-journal` left
/// beside a freshly created database would be rolled back into it.
const SIDECAR_SUFFIXES: [&str; 3] = ["-journal", "-wal", "-shm"];

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(suffix);
    PathBuf::from(name)
}

fn quarantine_if_exists(data_dir: &Path, db_path: &Path) -> AppResult<()> {
    if !db_path.exists() {
        return Ok(());
    }
    let dest = unique_quarantine_path(data_dir, chrono::Utc::now());
    std::fs::rename(db_path, &dest)?;
    for suffix in SIDECAR_SUFFIXES {
        let sidecar = with_suffix(db_path, suffix);
        if sidecar.exists() {
            std::fs::rename(&sidecar, with_suffix(&dest, suffix))?;
        }
    }
    Ok(())
}

fn recovery_paths(inner: &AppInner, command: &str) -> AppResult<(PathBuf, PathBuf)> {
    match inner {
        AppInner::Recovery {
            data_dir, db_path, ..
        } => Ok((data_dir.clone(), db_path.clone())),
        AppInner::Ready { .. } => Err(AppError::InvalidArgument(format!(
            "{command} is only available during recovery"
        ))),
    }
}

/// Build a fresh encrypted database at `data.db.new`, let `populate` fill it,
/// then swap it into place. The existing `data.db` (if any) is not touched
/// until the new file is complete, so a failure part-way through leaves the
/// user's data exactly where it was.
fn install_fresh_db<T>(
    data_dir: &Path,
    db_path: &Path,
    key: &DbKey,
    populate: impl FnOnce(&mut Connection) -> AppResult<T>,
) -> AppResult<(Connection, T)> {
    let new_path = data_dir.join("data.db.new");
    if new_path.exists() {
        std::fs::remove_file(&new_path)?;
    }
    drop(std::fs::File::create(&new_path)?);

    let result = (|| {
        let mut conn = db::open_encrypted(&new_path, key)?;
        migrations::run(&mut conn)?;
        seed::seed_default_categories_if_needed(&mut conn)?;
        let value = populate(&mut conn)?;
        drop(conn);
        Ok(value)
    })();

    match result {
        Err(err) => {
            let _ = std::fs::remove_file(&new_path);
            Err(err)
        }
        Ok(value) => {
            quarantine_if_exists(data_dir, db_path)?;
            std::fs::rename(&new_path, db_path)?;
            let conn = db::open_encrypted(db_path, key)?;
            Ok((conn, value))
        }
    }
}

pub fn recover_import(
    inner: &Mutex<AppInner>,
    keys: &dyn KeyStore,
    payload: &str,
) -> AppResult<ImportResult> {
    let mut guard = lock_inner(inner)?;
    let (data_dir, db_path) = recovery_paths(&guard, "recover_import")?;
    // Obtain the key before touching any file: a Keychain failure must leave
    // the existing database untouched.
    let key = obtain_recovery_key(keys)?;
    let (conn, imported) = install_fresh_db(&data_dir, &db_path, &key, |conn| {
        backup::import_snapshot_json(conn, payload, "overwrite")
    })?;
    *guard = AppInner::Ready { conn, db_path };
    Ok(imported)
}

pub fn recover_start_empty(inner: &Mutex<AppInner>, keys: &dyn KeyStore) -> AppResult<()> {
    let mut guard = lock_inner(inner)?;
    let (data_dir, db_path) = recovery_paths(&guard, "recover_start_empty")?;
    // Same ordering as recover_import: never quarantine a healthy data.db
    // until a key is in hand and the replacement file is complete.
    let key = obtain_recovery_key(keys)?;
    let (conn, ()) = install_fresh_db(&data_dir, &db_path, &key, |_| Ok(()))?;
    *guard = AppInner::Ready { conn, db_path };
    Ok(())
}

#[tauri::command]
pub fn boot_status(state: State<'_, AppState>) -> AppResult<BootStatus> {
    let inner = lock_inner(&state.inner)?;
    match &*inner {
        AppInner::Ready { db_path, .. } => Ok(BootStatus {
            state: BootState::Ready,
            recovery_reason: None,
            db_path: db_path.to_string_lossy().into_owned(),
        }),
        AppInner::Recovery {
            reason, db_path, ..
        } => Ok(BootStatus {
            state: BootState::Recovery,
            recovery_reason: Some(*reason),
            db_path: db_path.to_string_lossy().into_owned(),
        }),
    }
}

#[tauri::command]
pub fn recover_import_json(state: State<'_, AppState>, payload: String) -> AppResult<ImportResult> {
    recover_import(&state.inner, &os_keys(), &payload)
}

#[tauri::command(rename = "recover_start_empty")]
pub fn recover_start_empty_cmd(state: State<'_, AppState>) -> AppResult<()> {
    recover_start_empty(&state.inner, &os_keys())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infra::keychain::KEY_LEN;
    use tempfile::TempDir;

    struct RecordingKeys {
        get: fn() -> AppResult<Option<DbKey>>,
        calls: Mutex<Vec<&'static str>>,
    }

    impl RecordingKeys {
        fn new(get: fn() -> AppResult<Option<DbKey>>) -> Self {
            Self {
                get,
                calls: Mutex::new(Vec::new()),
            }
        }

        fn calls(&self) -> Vec<&'static str> {
            self.calls.lock().unwrap().clone()
        }
    }

    impl KeyStore for RecordingKeys {
        fn get(&self) -> AppResult<Option<DbKey>> {
            (self.get)()
        }
        fn create(&self) -> AppResult<DbKey> {
            self.calls.lock().unwrap().push("create");
            Ok([9u8; KEY_LEN])
        }
        fn delete(&self) -> AppResult<()> {
            self.calls.lock().unwrap().push("delete");
            Ok(())
        }
        fn preserve_corrupt(&self) -> AppResult<Option<String>> {
            self.calls.lock().unwrap().push("preserve");
            Ok(Some("db_key.corrupt-test".into()))
        }
    }

    fn keychain_error() -> AppError {
        AppError::Keychain(keyring::Error::Invalid("target".into(), "transient".into()))
    }

    #[test]
    fn obtain_recovery_key_does_not_replace_on_keychain_error() {
        let keys = RecordingKeys::new(|| Err(keychain_error()));
        let err = obtain_recovery_key(&keys).unwrap_err();
        assert!(matches!(err, AppError::Keychain(_)));
        assert!(keys.calls().is_empty(), "got {:?}", keys.calls());
    }

    #[test]
    fn obtain_recovery_key_mints_when_missing() {
        let keys = RecordingKeys::new(|| Ok(None));
        let key = obtain_recovery_key(&keys).unwrap();
        assert_eq!(key, [9u8; KEY_LEN]);
        assert_eq!(keys.calls(), ["create"]);
    }

    #[test]
    fn obtain_recovery_key_preserves_corrupt_entry_before_replacing_it() {
        let keys = RecordingKeys::new(|| Err(AppError::Corrupt("wrong length".into())));
        let key = obtain_recovery_key(&keys).unwrap();
        assert_eq!(key, [9u8; KEY_LEN]);
        assert_eq!(
            keys.calls(),
            ["preserve", "delete", "create"],
            "the undecodable secret must be parked before the slot is deleted"
        );
    }

    #[test]
    fn quarantine_name_uses_utc_stamp() {
        let dir = TempDir::new().unwrap();
        let now = chrono::DateTime::parse_from_rfc3339("2026-08-29T12:30:45Z")
            .unwrap()
            .with_timezone(&chrono::Utc);
        let path = unique_quarantine_path(dir.path(), now);
        assert_eq!(
            path.file_name().unwrap().to_str().unwrap(),
            "data.db.corrupt-20260829T123045Z"
        );
    }

    #[test]
    fn quarantine_name_appends_suffix_when_taken() {
        let dir = TempDir::new().unwrap();
        let now = chrono::DateTime::parse_from_rfc3339("2026-08-29T12:30:45Z")
            .unwrap()
            .with_timezone(&chrono::Utc);
        std::fs::write(dir.path().join("data.db.corrupt-20260829T123045Z"), b"a").unwrap();
        std::fs::write(dir.path().join("data.db.corrupt-20260829T123045Z-2"), b"b").unwrap();
        let path = unique_quarantine_path(dir.path(), now);
        assert_eq!(
            path.file_name().unwrap().to_str().unwrap(),
            "data.db.corrupt-20260829T123045Z-3"
        );
    }

    fn recovery_state(dir: &TempDir) -> (Mutex<AppInner>, PathBuf) {
        let db_path = dir.path().join("data.db");
        let inner = Mutex::new(AppInner::Recovery {
            reason: RecoveryReason::KeychainError,
            data_dir: dir.path().to_path_buf(),
            db_path: db_path.clone(),
        });
        (inner, db_path)
    }

    /// Quarantined database files in `dir`, excluding their SQLite sidecars.
    fn quarantined_databases(dir: &Path) -> Vec<PathBuf> {
        let mut found: Vec<PathBuf> = std::fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| {
                let name = path.file_name().unwrap().to_string_lossy();
                name.starts_with("data.db.corrupt-")
                    && !SIDECAR_SUFFIXES.iter().any(|s| name.ends_with(s))
            })
            .collect();
        found.sort();
        found
    }

    #[test]
    fn start_empty_keeps_existing_db_when_key_cannot_be_obtained() {
        let dir = TempDir::new().unwrap();
        let (inner, db_path) = recovery_state(&dir);
        std::fs::write(&db_path, b"old ledger").unwrap();
        let keys = RecordingKeys::new(|| Err(keychain_error()));

        let err = recover_start_empty(&inner, &keys).unwrap_err();

        assert!(matches!(err, AppError::Keychain(_)));
        assert_eq!(
            std::fs::read(&db_path).unwrap(),
            b"old ledger",
            "a healthy data.db must stay in place when no key is available"
        );
        assert!(quarantined_databases(dir.path()).is_empty());
        assert!(!dir.path().join("data.db.new").exists());
        assert!(matches!(*inner.lock().unwrap(), AppInner::Recovery { .. }));
    }

    #[test]
    fn start_empty_quarantines_old_db_and_opens_fresh_one() {
        let dir = TempDir::new().unwrap();
        let (inner, db_path) = recovery_state(&dir);
        std::fs::write(&db_path, b"old ledger").unwrap();
        std::fs::write(with_suffix(&db_path, "-journal"), b"hot journal").unwrap();
        let keys = RecordingKeys::new(|| Ok(Some([9u8; KEY_LEN])));

        recover_start_empty(&inner, &keys).unwrap();

        let quarantined = quarantined_databases(dir.path());
        assert_eq!(quarantined.len(), 1);
        assert_eq!(std::fs::read(&quarantined[0]).unwrap(), b"old ledger");
        assert_eq!(
            std::fs::read(with_suffix(&quarantined[0], "-journal")).unwrap(),
            b"hot journal"
        );
        assert!(
            !with_suffix(&db_path, "-journal").exists(),
            "a stale journal must not be left beside the fresh database"
        );
        assert!(!dir.path().join("data.db.new").exists());
        assert!(matches!(*inner.lock().unwrap(), AppInner::Ready { .. }));

        let conn = db::open_encrypted(&db_path, &[9u8; KEY_LEN]).unwrap();
        let categories: i64 = conn
            .query_row("SELECT count(*) FROM categories", [], |r| r.get(0))
            .unwrap();
        assert!(categories > 0, "fresh database must be migrated and seeded");
    }

    #[test]
    fn quarantine_moves_sqlite_sidecars_with_the_database() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("data.db");
        std::fs::write(&db_path, b"db").unwrap();
        for suffix in SIDECAR_SUFFIXES {
            std::fs::write(with_suffix(&db_path, suffix), suffix.as_bytes()).unwrap();
        }

        quarantine_if_exists(dir.path(), &db_path).unwrap();

        assert!(!db_path.exists());
        let quarantined = quarantined_databases(dir.path());
        assert_eq!(quarantined.len(), 1);
        for suffix in SIDECAR_SUFFIXES {
            assert!(!with_suffix(&db_path, suffix).exists());
            assert_eq!(
                std::fs::read(with_suffix(&quarantined[0], suffix)).unwrap(),
                suffix.as_bytes()
            );
        }
    }
}
