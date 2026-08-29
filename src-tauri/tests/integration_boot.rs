use budget_tracker_lib::error::AppError;
use budget_tracker_lib::infra::boot::{boot, BootOutcome, KeyStore, RecoveryReason, DB_FILENAME};
use budget_tracker_lib::infra::keychain::{DbKey, KEY_LEN};
use std::sync::Mutex;

struct MemKeys {
    key: Mutex<Option<DbKey>>,
    create_calls: Mutex<u32>,
}

impl KeyStore for MemKeys {
    fn get(&self) -> budget_tracker_lib::error::AppResult<Option<DbKey>> {
        Ok(*self.key.lock().unwrap())
    }
    fn create(&self) -> budget_tracker_lib::error::AppResult<DbKey> {
        *self.create_calls.lock().unwrap() += 1;
        if self.key.lock().unwrap().is_some() {
            return Err(AppError::Conflict("exists".into()));
        }
        let key = [7u8; KEY_LEN];
        *self.key.lock().unwrap() = Some(key);
        Ok(key)
    }
    fn delete(&self) -> budget_tracker_lib::error::AppResult<()> {
        *self.key.lock().unwrap() = None;
        Ok(())
    }
}

fn data_dir() -> tempfile::TempDir {
    tempfile::TempDir::new().unwrap()
}

#[test]
fn first_launch_creates_key_and_db() {
    let dir = data_dir();
    let keys = MemKeys {
        key: Mutex::new(None),
        create_calls: Mutex::new(0),
    };
    let outcome = boot(dir.path(), &keys).unwrap();
    assert!(matches!(outcome, BootOutcome::Ready { .. }));
    assert_eq!(*keys.create_calls.lock().unwrap(), 1);
    assert!(dir.path().join(DB_FILENAME).exists());
}

#[test]
fn existing_db_and_missing_key_is_recovery_and_does_not_create_key() {
    let dir = data_dir();
    std::fs::write(dir.path().join(DB_FILENAME), b"ciphertext-placeholder").unwrap();
    let keys = MemKeys {
        key: Mutex::new(None),
        create_calls: Mutex::new(0),
    };
    let outcome = boot(dir.path(), &keys).unwrap();
    assert!(matches!(
        outcome,
        BootOutcome::Recovery {
            reason: RecoveryReason::KeyMissing,
            ..
        }
    ));
    assert_eq!(*keys.create_calls.lock().unwrap(), 0);
}

#[test]
fn existing_db_and_wrong_key_is_decrypt_failed_and_does_not_create_key() {
    let dir = data_dir();
    let right = [1u8; KEY_LEN];
    let wrong = [2u8; KEY_LEN];
    {
        let path = dir.path().join(DB_FILENAME);
        let mut conn = budget_tracker_lib::infra::db::open_encrypted(&path, &right).unwrap();
        // SQLCipher can treat an empty file as a fresh DB. Apply migrations
        // under the right key so reopening it with another key must decrypt.
        budget_tracker_lib::infra::migrations::run(&mut conn).unwrap();
        drop(conn);
    }
    let keys = MemKeys {
        key: Mutex::new(Some(wrong)),
        create_calls: Mutex::new(0),
    };
    let outcome = boot(dir.path(), &keys).unwrap();
    assert!(matches!(
        outcome,
        BootOutcome::Recovery {
            reason: RecoveryReason::DecryptFailed,
            ..
        }
    ));
    assert_eq!(*keys.create_calls.lock().unwrap(), 0);
}

#[test]
fn existing_db_and_matching_key_is_ready() {
    let dir = data_dir();
    let key = [1u8; KEY_LEN];
    {
        let path = dir.path().join(DB_FILENAME);
        let mut conn = budget_tracker_lib::infra::db::open_encrypted(&path, &key).unwrap();
        budget_tracker_lib::infra::migrations::run(&mut conn).unwrap();
        drop(conn);
    }
    let keys = MemKeys {
        key: Mutex::new(Some(key)),
        create_calls: Mutex::new(0),
    };
    let outcome = boot(dir.path(), &keys).unwrap();
    assert!(matches!(outcome, BootOutcome::Ready { .. }));
    assert_eq!(*keys.create_calls.lock().unwrap(), 0);
}
