use std::path::{Path, PathBuf};
use std::sync::Mutex;

use budget_tracker_lib::commands::backup;
use budget_tracker_lib::commands::meta::{AppInner, AppState};
use budget_tracker_lib::commands::recovery::{recover_import, recover_start_empty};
use budget_tracker_lib::domain::account::AccountKind;
use budget_tracker_lib::error::AppError;
use budget_tracker_lib::infra::boot::{boot, BootOutcome, KeyStore, RecoveryReason, DB_FILENAME};
use budget_tracker_lib::infra::keychain::{DbKey, KEY_LEN};
use budget_tracker_lib::infra::repo::account_repo;
use budget_tracker_lib::infra::{db, migrations};
use rusqlite::Connection;

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
    fn preserve_corrupt(&self) -> budget_tracker_lib::error::AppResult<Option<String>> {
        Ok(None)
    }
}

fn snapshot_with_account(name: &str) -> String {
    let mut conn = Connection::open_in_memory().unwrap();
    migrations::run(&mut conn).unwrap();
    account_repo::insert(
        &conn,
        &account_repo::InsertInput {
            name,
            kind: AccountKind::Cash,
            currency: "JPY",
            initial_balance: 1_000,
            display_order: 0,
            note: "",
            now: "2026-08-29T00:00:00+00:00",
        },
    )
    .unwrap();
    backup::export_snapshot_json(&conn).unwrap()
}

fn corrupt_backups(dir: &Path) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap()
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("data.db.corrupt-"))
        })
        .collect();
    out.sort();
    out
}

fn recovery_inner(outcome: BootOutcome) -> Mutex<AppInner> {
    Mutex::new(match outcome {
        BootOutcome::Recovery {
            reason,
            data_dir,
            db_path,
        } => AppInner::Recovery {
            reason,
            data_dir,
            db_path,
        },
        _ => panic!("expected Recovery"),
    })
}

#[test]
fn decrypt_failed_valid_json_becomes_ready_and_quarantines() {
    let dir = tempfile::TempDir::new().unwrap();
    let right = [1u8; KEY_LEN];
    let wrong = [2u8; KEY_LEN];
    let db_path = dir.path().join(DB_FILENAME);
    {
        let mut conn = db::open_encrypted(&db_path, &right).unwrap();
        migrations::run(&mut conn).unwrap();
        account_repo::insert(
            &conn,
            &account_repo::InsertInput {
                name: "旧口座",
                kind: AccountKind::Cash,
                currency: "JPY",
                initial_balance: 0,
                display_order: 0,
                note: "",
                now: "2026-08-29T00:00:00+00:00",
            },
        )
        .unwrap();
    }
    let original = std::fs::read(&db_path).unwrap();

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
    let inner = recovery_inner(outcome);

    let payload = snapshot_with_account("復元口座");
    recover_import(&inner, &keys, &payload).unwrap();

    let guard = inner.lock().unwrap();
    match &*guard {
        AppInner::Ready { conn, .. } => {
            let accounts = account_repo::list(conn, true).unwrap();
            assert!(
                accounts.iter().any(|account| account.name == "復元口座"),
                "imported accounts: {:?}",
                accounts
                    .iter()
                    .map(|account| &account.name)
                    .collect::<Vec<_>>()
            );
            assert!(accounts.iter().all(|account| account.name != "旧口座"));
        }
        _ => panic!("expected Ready after successful recover_import"),
    }
    drop(guard);

    let backups = corrupt_backups(dir.path());
    assert_eq!(backups.len(), 1);
    assert_eq!(std::fs::read(&backups[0]).unwrap(), original);
    assert_eq!(*keys.create_calls.lock().unwrap(), 0);
    assert!(!dir.path().join("data.db.new").exists());
}

#[test]
fn decrypt_failed_invalid_json_leaves_original_untouched() {
    let dir = tempfile::TempDir::new().unwrap();
    let right = [1u8; KEY_LEN];
    let wrong = [2u8; KEY_LEN];
    let db_path = dir.path().join(DB_FILENAME);
    {
        let conn = db::open_encrypted(&db_path, &right).unwrap();
        drop(conn);
    }
    let original = std::fs::read(&db_path).unwrap();

    let keys = MemKeys {
        key: Mutex::new(Some(wrong)),
        create_calls: Mutex::new(0),
    };
    let outcome = boot(dir.path(), &keys).unwrap();
    let inner = recovery_inner(outcome);

    let err = recover_import(&inner, &keys, "not-json").unwrap_err();
    assert!(matches!(err, AppError::InvalidArgument(_)));

    assert!(matches!(
        *inner.lock().unwrap(),
        AppInner::Recovery {
            reason: RecoveryReason::DecryptFailed,
            ..
        }
    ));
    assert_eq!(std::fs::read(&db_path).unwrap(), original);
    assert!(!dir.path().join("data.db.new").exists());
    assert!(corrupt_backups(dir.path()).is_empty());
}

#[test]
fn key_missing_start_empty_creates_key_and_ready() {
    let dir = tempfile::TempDir::new().unwrap();
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
    let inner = recovery_inner(outcome);

    recover_start_empty(&inner, &keys).unwrap();
    assert_eq!(*keys.create_calls.lock().unwrap(), 1);

    let guard = inner.lock().unwrap();
    match &*guard {
        AppInner::Ready { conn, .. } => {
            let version: String = conn
                .query_row(
                    "SELECT value FROM app_meta WHERE key = 'schema_version'",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert!(!version.is_empty());
        }
        _ => panic!("expected Ready after recover_start_empty"),
    }
    drop(guard);

    assert_eq!(corrupt_backups(dir.path()).len(), 1);
    assert!(dir.path().join(DB_FILENAME).exists());
}

#[test]
fn import_json_while_recovery_is_unavailable() {
    let dir = tempfile::TempDir::new().unwrap();
    let state = AppState {
        inner: Mutex::new(AppInner::Recovery {
            reason: RecoveryReason::DecryptFailed,
            data_dir: dir.path().to_path_buf(),
            db_path: dir.path().join(DB_FILENAME),
        }),
    };
    let err = state
        .with_conn_mut(|conn| backup::import_snapshot_json(conn, "{}", "overwrite"))
        .unwrap_err();
    assert!(matches!(err, AppError::Unavailable(_)));
    assert_eq!(
        err.to_string(),
        "unavailable: database is in recovery (DecryptFailed)"
    );
}
