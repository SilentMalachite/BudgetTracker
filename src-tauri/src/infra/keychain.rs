use base64::engine::general_purpose::STANDARD_NO_PAD;
use base64::Engine as _;
use rand::RngCore;

use crate::error::{AppError, AppResult};

pub const KEY_LEN: usize = 32;
pub type DbKey = [u8; KEY_LEN];

pub fn get_key(service: &str, account: &str) -> AppResult<Option<DbKey>> {
    let entry = keyring::Entry::new(service, account)?;
    match entry.get_password() {
        Ok(b64) => Ok(Some(decode_key(&b64)?)),
        Err(keyring::Error::NoEntry) => Ok(None),
        // The entry exists but its bytes are not a string (non-UTF-8 on
        // macOS / Linux, malformed UTF-16 on Windows). That is content
        // corruption, not a platform failure: classify it like an
        // undecodable base64 value so recovery is allowed to replace it.
        Err(keyring::Error::BadEncoding(_)) => Err(AppError::Corrupt(
            "corrupt keychain entry: stored secret is not valid text".into(),
        )),
        Err(e) => Err(e.into()),
    }
}

pub fn create_key(service: &str, account: &str) -> AppResult<DbKey> {
    if get_key(service, account)?.is_some() {
        return Err(AppError::Conflict("keychain entry already exists".into()));
    }
    let mut key = [0u8; KEY_LEN];
    rand::thread_rng().fill_bytes(&mut key);
    let entry = keyring::Entry::new(service, account)?;
    entry.set_password(&STANDARD_NO_PAD.encode(key))?;
    Ok(key)
}

fn decode_key(b64: &str) -> AppResult<DbKey> {
    let raw = STANDARD_NO_PAD.decode(b64.as_bytes()).map_err(|err| {
        AppError::Corrupt(format!("corrupt keychain entry: invalid encoding: {err}"))
    })?;
    raw.try_into().map_err(|raw: Vec<u8>| {
        AppError::Corrupt(format!(
            "corrupt keychain entry: stored key has wrong length: {}",
            raw.len()
        ))
    })
}

/// Retrieve the stored DB key or, if absent, generate a new 32-byte key,
/// store it in the OS keychain, and return it.
///
/// Test-only wrapper. Production boot must call `get_key` / `create_key`
/// explicitly so an existing `data.db` never gets a freshly minted key; this
/// is compiled out of release builds so no production path can reach it.
///
/// `service` typically: "jp.budget-tracker"
/// `account` typically: "db_key"
#[cfg(test)]
pub fn get_or_create_key(service: &str, account: &str) -> AppResult<DbKey> {
    match get_key(service, account)? {
        Some(key) => Ok(key),
        None => create_key(service, account),
    }
}

pub fn delete_key(service: &str, account: &str) -> AppResult<()> {
    let entry = keyring::Entry::new(service, account)?;
    match entry.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e.into()),
    }
}

/// Copy the raw secret stored under `account` to `<account>.corrupt-<stamp>`
/// in the same service. Recovery calls this before deleting an entry it could
/// not decode, so bytes that might still decrypt the quarantined `data.db`
/// are parked rather than destroyed. Returns the account the copy was stored
/// under, or `None` when there was no entry to preserve.
pub fn preserve_corrupt_key(
    service: &str,
    account: &str,
    now: chrono::DateTime<chrono::Utc>,
) -> AppResult<Option<String>> {
    let raw = match keyring::Entry::new(service, account)?.get_secret() {
        Ok(raw) => raw,
        Err(keyring::Error::NoEntry) => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    let stamp = now.format("%Y%m%dT%H%M%SZ");
    let mut backup_account = format!("{account}.corrupt-{stamp}");
    let mut n = 2u32;
    while entry_exists(service, &backup_account)? {
        backup_account = format!("{account}.corrupt-{stamp}-{n}");
        n += 1;
    }
    keyring::Entry::new(service, &backup_account)?.set_secret(&raw)?;
    Ok(Some(backup_account))
}

fn entry_exists(service: &str, account: &str) -> AppResult<bool> {
    match keyring::Entry::new(service, account)?.get_secret() {
        Ok(_) => Ok(true),
        Err(keyring::Error::NoEntry) => Ok(false),
        Err(e) => Err(e.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use keyring::credential::{Credential, CredentialApi, CredentialBuilder, CredentialBuilderApi};
    use std::collections::HashMap;
    use std::sync::{Mutex, Once, OnceLock};

    // ---------------------------------------------------------------------------
    // Persistent in-process mock credential store.
    // The keyring built-in mock has EntryOnly persistence (no cross-entry state),
    // so we provide a custom builder backed by a global HashMap.
    // ---------------------------------------------------------------------------

    // Secrets are kept as raw bytes so tests can seed non-UTF-8 content and
    // exercise keyring's BadEncoding path through `get_password`.
    static STORE: OnceLock<Mutex<HashMap<String, Vec<u8>>>> = OnceLock::new();

    fn store() -> &'static Mutex<HashMap<String, Vec<u8>>> {
        STORE.get_or_init(|| Mutex::new(HashMap::new()))
    }

    fn store_key(service: &str, account: &str) -> String {
        format!("{service}\x00{account}")
    }

    struct PersistentMockCredential {
        key: String,
    }

    impl CredentialApi for PersistentMockCredential {
        fn set_secret(&self, secret: &[u8]) -> keyring::Result<()> {
            store()
                .lock()
                .unwrap()
                .insert(self.key.clone(), secret.to_vec());
            Ok(())
        }

        fn get_secret(&self) -> keyring::Result<Vec<u8>> {
            match store().lock().unwrap().get(&self.key).cloned() {
                Some(v) => Ok(v),
                None => Err(keyring::Error::NoEntry),
            }
        }

        fn delete_credential(&self) -> keyring::Result<()> {
            match store().lock().unwrap().remove(&self.key) {
                Some(_) => Ok(()),
                None => Err(keyring::Error::NoEntry),
            }
        }

        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
    }

    struct PersistentMockBuilder;

    impl CredentialBuilderApi for PersistentMockBuilder {
        fn build(
            &self,
            _target: Option<&str>,
            service: &str,
            user: &str,
        ) -> keyring::Result<Box<Credential>> {
            Ok(Box::new(PersistentMockCredential {
                key: store_key(service, user),
            }))
        }

        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
    }

    fn persistent_credential_builder() -> Box<CredentialBuilder> {
        Box::new(PersistentMockBuilder)
    }

    // ---------------------------------------------------------------------------

    static INIT: Once = Once::new();
    fn ensure_mock() {
        INIT.call_once(|| {
            keyring::set_default_credential_builder(persistent_credential_builder());
        });
    }

    fn unique_account(prefix: &str) -> String {
        use std::sync::atomic::{AtomicU64, Ordering};
        static N: AtomicU64 = AtomicU64::new(0);
        format!("{}-{}", prefix, N.fetch_add(1, Ordering::SeqCst))
    }

    #[test]
    fn creates_key_when_absent() {
        ensure_mock();
        let account = unique_account("create");
        let key = get_or_create_key("test", &account).unwrap();
        assert!(key.iter().any(|&b| b != 0), "key should not be all zeros");
        delete_key("test", &account).unwrap();
    }

    #[test]
    fn returns_same_key_on_second_call() {
        ensure_mock();
        let account = unique_account("idempotent");
        let first = get_or_create_key("test", &account).unwrap();
        let second = get_or_create_key("test", &account).unwrap();
        assert_eq!(first, second);
        delete_key("test", &account).unwrap();
    }

    #[test]
    fn different_accounts_get_different_keys() {
        ensure_mock();
        let a1 = unique_account("diff-a");
        let a2 = unique_account("diff-b");
        let k1 = get_or_create_key("test", &a1).unwrap();
        let k2 = get_or_create_key("test", &a2).unwrap();
        assert_ne!(k1, k2);
        delete_key("test", &a1).unwrap();
        delete_key("test", &a2).unwrap();
    }

    /// Seed a raw secret under a fresh account and assert `get_key` reports it
    /// as `Corrupt` -- the classification recovery relies on to know the entry
    /// is safe to delete and replace (a `Keychain` error would leave the user
    /// stuck, see `commands::recovery::obtain_recovery_key`).
    fn assert_stored_secret_is_corrupt(prefix: &str, secret: &[u8]) {
        ensure_mock();
        let account = unique_account(prefix);
        let entry = keyring::Entry::new("test", &account).unwrap();
        entry.set_secret(secret).unwrap();
        let result = get_key("test", &account);
        assert!(
            matches!(result, Err(AppError::Corrupt(_))),
            "unreadable keychain secret must be Corrupt so recovery can replace it, got: {:?}",
            result
        );
        delete_key("test", &account).unwrap();
    }

    #[test]
    fn wrong_length_entry_returns_corrupt_error() {
        // "c2hvcnQ" is the base64-NO-PAD encoding of "short" (5 bytes), which is != KEY_LEN
        assert_stored_secret_is_corrupt("wrong-len", b"c2hvcnQ");
    }

    fn fixed_now() -> chrono::DateTime<chrono::Utc> {
        chrono::DateTime::parse_from_rfc3339("2026-08-29T12:30:45Z")
            .unwrap()
            .with_timezone(&chrono::Utc)
    }

    #[test]
    fn preserve_corrupt_key_copies_raw_bytes_to_stamped_account() {
        ensure_mock();
        let account = unique_account("preserve");
        let entry = keyring::Entry::new("test", &account).unwrap();
        entry.set_secret(b"\xff\xfe not text").unwrap();

        let backup = preserve_corrupt_key("test", &account, fixed_now())
            .unwrap()
            .unwrap();

        assert_eq!(backup, format!("{account}.corrupt-20260829T123045Z"));
        let copy = keyring::Entry::new("test", &backup).unwrap();
        assert_eq!(copy.get_secret().unwrap(), b"\xff\xfe not text");
        assert_eq!(
            entry.get_secret().unwrap(),
            b"\xff\xfe not text",
            "the original entry is left for delete_key"
        );
        delete_key("test", &account).unwrap();
        delete_key("test", &backup).unwrap();
    }

    #[test]
    fn preserve_corrupt_key_never_overwrites_an_earlier_copy() {
        ensure_mock();
        let account = unique_account("preserve-twice");
        let entry = keyring::Entry::new("test", &account).unwrap();
        entry.set_secret(b"first").unwrap();
        let first = preserve_corrupt_key("test", &account, fixed_now())
            .unwrap()
            .unwrap();
        entry.set_secret(b"second").unwrap();

        let second = preserve_corrupt_key("test", &account, fixed_now())
            .unwrap()
            .unwrap();

        assert_eq!(second, format!("{first}-2"));
        let secret_of = |name: &str| {
            keyring::Entry::new("test", name)
                .unwrap()
                .get_secret()
                .unwrap()
        };
        assert_eq!(secret_of(&first), b"first");
        assert_eq!(secret_of(&second), b"second");
        for name in [&account, &first, &second] {
            delete_key("test", name).unwrap();
        }
    }

    #[test]
    fn preserve_corrupt_key_is_noop_when_entry_is_absent() {
        ensure_mock();
        let account = unique_account("preserve-absent");
        assert_eq!(
            preserve_corrupt_key("test", &account, fixed_now()).unwrap(),
            None
        );
    }

    #[test]
    fn invalid_base64_entry_returns_corrupt_error() {
        assert_stored_secret_is_corrupt("bad-b64", b"not valid base64!!!");
    }

    #[test]
    fn padded_standard_base64_entry_returns_corrupt_error() {
        let padded = base64::engine::general_purpose::STANDARD.encode([7u8; KEY_LEN]);
        assert!(
            padded.contains('='),
            "fixture must use padded STANDARD encoding, got {padded}"
        );
        assert_stored_secret_is_corrupt("padded-b64", padded.as_bytes());
    }

    #[test]
    fn non_utf8_entry_returns_corrupt_error() {
        // keyring's get_password fails with BadEncoding before base64 is reached.
        assert_stored_secret_is_corrupt("non-utf8", &[0xff, 0xfe, 0x80, 0x00]);
    }

    #[test]
    fn get_returns_none_when_absent() {
        ensure_mock();
        let account = unique_account("get-none");
        let got = get_key("test", &account).unwrap();
        assert!(got.is_none());
    }

    #[test]
    fn get_does_not_create_an_entry() {
        ensure_mock();
        let account = unique_account("get-no-create");
        let _ = get_key("test", &account).unwrap();
        let again = get_key("test", &account).unwrap();
        assert!(again.is_none());
    }

    #[test]
    fn create_persists_and_get_returns_it() {
        ensure_mock();
        let account = unique_account("create-get");
        let created = create_key("test", &account).unwrap();
        let got = get_key("test", &account).unwrap().unwrap();
        assert_eq!(created, got);
        delete_key("test", &account).unwrap();
    }

    #[test]
    fn create_fails_if_entry_exists() {
        ensure_mock();
        let account = unique_account("create-exists");
        create_key("test", &account).unwrap();
        let err = create_key("test", &account).unwrap_err();
        assert!(matches!(err, AppError::Conflict(_)));
        delete_key("test", &account).unwrap();
    }
}
