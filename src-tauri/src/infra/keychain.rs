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
    let raw = STANDARD_NO_PAD.decode(b64.as_bytes())?;
    if raw.len() != KEY_LEN {
        return Err(AppError::Corrupt(format!(
            "corrupt keychain entry: stored key has wrong length: {}",
            raw.len()
        )));
    }
    let mut out = [0u8; KEY_LEN];
    out.copy_from_slice(&raw);
    Ok(out)
}

/// Retrieve the stored DB key or, if absent, generate a new 32-byte key,
/// store it in the OS keychain, and return it.
///
/// Test-only / legacy wrapper. Production boot must call `get_key` / `create_key`
/// explicitly so an existing `data.db` never gets a freshly minted key.
///
/// `service` typically: "jp.budget-tracker"
/// `account` typically: "db_key"
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

    static STORE: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();

    fn store() -> &'static Mutex<HashMap<String, String>> {
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
            let password = String::from_utf8(secret.to_vec())
                .map_err(|_| keyring::Error::BadEncoding(secret.to_vec()))?;
            store().lock().unwrap().insert(self.key.clone(), password);
            Ok(())
        }

        fn get_secret(&self) -> keyring::Result<Vec<u8>> {
            match store().lock().unwrap().get(&self.key).cloned() {
                Some(v) => Ok(v.into_bytes()),
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

    #[test]
    fn corrupt_entry_returns_corrupt_error() {
        ensure_mock();
        let account = unique_account("corrupt");
        // "c2hvcnQ" is the base64-NO-PAD encoding of "short" (5 bytes), which is != KEY_LEN
        let entry = keyring::Entry::new("test", &account).unwrap();
        entry.set_password("c2hvcnQ").unwrap();
        let result = get_or_create_key("test", &account);
        assert!(
            matches!(result, Err(AppError::Corrupt(_))),
            "expected Corrupt error, got: {:?}",
            result
        );
        delete_key("test", &account).unwrap();
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
