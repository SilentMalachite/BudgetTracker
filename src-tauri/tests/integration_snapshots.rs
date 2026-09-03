use std::path::{Path, PathBuf};
use std::sync::Mutex;

use budget_tracker_lib::commands::backup;
use budget_tracker_lib::commands::meta::AppInner;
use budget_tracker_lib::commands::snapshots::{
    self, PRE_IMPORT_PREFIX, REPLACED_PREFIX, RESTORING_SUFFIX,
};
use budget_tracker_lib::domain::account::AccountKind;
use budget_tracker_lib::error::{AppError, AppResult};
use budget_tracker_lib::infra::boot::{KeyStore, RecoveryReason, DB_FILENAME};
use budget_tracker_lib::infra::keychain::{DbKey, KEY_LEN};
use budget_tracker_lib::infra::repo::account_repo;
use budget_tracker_lib::infra::{db, migrations};
use chrono::Utc;
use rusqlite::Connection;

const KEY: DbKey = [5u8; KEY_LEN];
const NOW: &str = "2026-09-03T00:00:00+00:00";

/// A key store that only ever answers `get`; restore must never mint or
/// delete a key, so `create` / `delete` fail loudly.
struct FixedKeys(Option<DbKey>);

impl KeyStore for FixedKeys {
    fn get(&self) -> AppResult<Option<DbKey>> {
        Ok(self.0)
    }
    fn create(&self) -> AppResult<DbKey> {
        Err(AppError::Conflict("create must not be called".into()))
    }
    fn delete(&self) -> AppResult<()> {
        Err(AppError::Conflict("delete must not be called".into()))
    }
    fn preserve_corrupt(&self) -> AppResult<Option<String>> {
        Err(AppError::Conflict(
            "preserve_corrupt must not be called".into(),
        ))
    }
}

fn insert_account(conn: &Connection, name: &str) {
    account_repo::insert(
        conn,
        &account_repo::InsertInput {
            name,
            kind: AccountKind::Cash,
            currency: "JPY",
            initial_balance: 1_000,
            display_order: 0,
            note: "",
            now: NOW,
        },
    )
    .unwrap();
}

fn account_names(conn: &Connection) -> Vec<String> {
    account_repo::list(conn, true)
        .unwrap()
        .into_iter()
        .map(|account| account.name)
        .collect()
}

/// An encrypted `data.db` in `dir` holding one account named `name`.
fn open_live(dir: &Path, name: &str) -> (Connection, PathBuf) {
    let db_path = dir.join(DB_FILENAME);
    let mut conn = db::open_encrypted(&db_path, &KEY).unwrap();
    migrations::run(&mut conn).unwrap();
    insert_account(&conn, name);
    (conn, db_path)
}

fn payload_with_account(name: &str) -> String {
    let mut conn = Connection::open_in_memory().unwrap();
    migrations::run(&mut conn).unwrap();
    insert_account(&conn, name);
    backup::export_snapshot_json(&conn).unwrap()
}

fn files_with_prefix(dir: &Path, prefix: &str) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(prefix))
        })
        .collect();
    found.sort();
    found
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(suffix);
    PathBuf::from(name)
}

fn overwrite(
    conn: &mut Connection,
    db_path: &Path,
    payload: &str,
) -> AppResult<backup::ImportResult> {
    backup::import_snapshot_json_guarded(conn, db_path, payload, "overwrite")
}

#[test]
fn overwrite_import_writes_a_snapshot_that_opens_with_the_same_key() {
    let dir = tempfile::TempDir::new().unwrap();
    let (mut conn, db_path) = open_live(dir.path(), "旧口座");

    let result = overwrite(&mut conn, &db_path, &payload_with_account("新口座")).unwrap();

    assert_eq!(result.accounts, 1);
    assert_eq!(account_names(&conn), vec!["新口座"]);

    let listed = snapshots::list_pre_import_snapshots_in(dir.path()).unwrap();
    assert_eq!(listed.len(), 1, "exactly one safety copy: {listed:?}");
    let snapshot = &listed[0];
    assert!(snapshot.file_name.starts_with(PRE_IMPORT_PREFIX));
    assert!(snapshot.size_bytes > 0);
    assert!(
        snapshot.created_at.ends_with('Z') && snapshot.created_at.contains('T'),
        "created_at must be RFC3339 UTC: {}",
        snapshot.created_at
    );

    let copy = db::open_encrypted(&dir.path().join(&snapshot.file_name), &KEY).unwrap();
    assert_eq!(
        account_names(&copy),
        vec!["旧口座"],
        "the copy holds the rows from before the import"
    );
}

#[test]
fn append_import_writes_no_snapshot() {
    let dir = tempfile::TempDir::new().unwrap();
    let (mut conn, db_path) = open_live(dir.path(), "旧口座");

    backup::import_snapshot_json_guarded(
        &mut conn,
        &db_path,
        &payload_with_account("追加口座"),
        "append",
    )
    .unwrap();

    assert_eq!(account_names(&conn), vec!["旧口座", "追加口座"]);
    assert!(files_with_prefix(dir.path(), PRE_IMPORT_PREFIX).is_empty());
}

#[test]
fn rejected_payload_writes_no_snapshot() {
    let dir = tempfile::TempDir::new().unwrap();
    let (mut conn, db_path) = open_live(dir.path(), "旧口座");

    let err = overwrite(&mut conn, &db_path, "not-json").unwrap_err();

    assert!(matches!(err, AppError::InvalidArgument(_)), "{err}");
    assert!(files_with_prefix(dir.path(), PRE_IMPORT_PREFIX).is_empty());
    assert_eq!(account_names(&conn), vec!["旧口座"]);
}

#[test]
fn failed_overwrite_transaction_discards_its_snapshot() {
    let dir = tempfile::TempDir::new().unwrap();
    let (mut conn, db_path) = open_live(dir.path(), "旧口座");
    // Two accounts sharing an id pass row validation but violate the primary
    // key on insert, so the transaction rolls back after the copy was taken.
    let payload = serde_json::json!({
        "schema_version": 1,
        "exported_at": "2026-09-03T00:00:00Z",
        "categories": [],
        "accounts": [
            {"id": 1, "name": "A", "kind": "cash", "initial_balance": 0},
            {"id": 1, "name": "B", "kind": "cash", "initial_balance": 0}
        ],
        "recurring_rules": [],
        "transactions": [],
        "budgets": [],
        "app_meta": []
    })
    .to_string();

    let err = overwrite(&mut conn, &db_path, &payload).unwrap_err();

    assert!(matches!(err, AppError::Db(_)), "{err}");
    assert!(
        files_with_prefix(dir.path(), PRE_IMPORT_PREFIX).is_empty(),
        "nothing changed, so the copy must not linger"
    );
    assert_eq!(account_names(&conn), vec!["旧口座"]);
}

#[test]
fn repeated_overwrite_imports_keep_only_the_newest_three_snapshots() {
    let dir = tempfile::TempDir::new().unwrap();
    let (mut conn, db_path) = open_live(dir.path(), "口座0");

    for n in 1..=5 {
        overwrite(
            &mut conn,
            &db_path,
            &payload_with_account(&format!("口座{n}")),
        )
        .unwrap();
    }

    let listed = snapshots::list_pre_import_snapshots_in(dir.path()).unwrap();
    assert_eq!(listed.len(), 3, "{listed:?}");
    // Newest first: the copies taken before imports 5, 4 and 3 hold 口座4, 口座3, 口座2.
    for (snapshot, expected) in listed.iter().zip(["口座4", "口座3", "口座2"]) {
        let copy = db::open_encrypted(&dir.path().join(&snapshot.file_name), &KEY).unwrap();
        assert_eq!(
            account_names(&copy),
            vec![expected],
            "{}",
            snapshot.file_name
        );
    }
    assert_eq!(files_with_prefix(dir.path(), PRE_IMPORT_PREFIX).len(), 3);
}

#[test]
fn restore_brings_back_pre_import_rows_and_keeps_the_replaced_file() {
    let dir = tempfile::TempDir::new().unwrap();
    let (mut conn, db_path) = open_live(dir.path(), "旧口座");
    overwrite(&mut conn, &db_path, &payload_with_account("新口座")).unwrap();
    let name = snapshots::list_pre_import_snapshots_in(dir.path()).unwrap()[0]
        .file_name
        .clone();
    let inner = Mutex::new(AppInner::Ready {
        conn,
        db_path: db_path.clone(),
    });

    let replaced =
        snapshots::restore_pre_import_snapshot_in(&inner, &FixedKeys(Some(KEY)), &name, Utc::now())
            .unwrap();

    {
        let guard = inner.lock().unwrap();
        match &*guard {
            AppInner::Ready {
                conn,
                db_path: live_path,
            } => {
                assert_eq!(account_names(conn), vec!["旧口座"]);
                assert_eq!(live_path, &db_path);
            }
            _ => panic!("expected Ready after restore"),
        }
    }

    assert!(
        replaced
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with(REPLACED_PREFIX)),
        "{}",
        replaced.display()
    );
    assert_eq!(
        files_with_prefix(dir.path(), REPLACED_PREFIX),
        vec![replaced.clone()]
    );
    let replaced_conn = db::open_encrypted(&replaced, &KEY).unwrap();
    assert_eq!(
        account_names(&replaced_conn),
        vec!["新口座"],
        "the imported data is kept, never deleted"
    );
    assert!(
        dir.path().join(&name).exists(),
        "the snapshot stays available"
    );
    assert!(!with_suffix(&db_path, RESTORING_SUFFIX).exists());

    drop(inner);
    let reopened = db::open_encrypted(&db_path, &KEY).unwrap();
    assert_eq!(account_names(&reopened), vec!["旧口座"]);
}

#[test]
fn restore_rejects_names_outside_the_snapshot_pattern() {
    let dir = tempfile::TempDir::new().unwrap();
    let (conn, db_path) = open_live(dir.path(), "旧口座");
    // A real snapshot exists, so only the name check can reject these.
    std::fs::write(dir.path().join("x"), b"decoy").unwrap();
    let inner = Mutex::new(AppInner::Ready {
        conn,
        db_path: db_path.clone(),
    });

    for bad in [
        "../x",
        "x",
        "",
        "data.db",
        "data.db.corrupt-20260101T000000Z",
        "data.db.pre-import-20260101T000000Z/../x",
        "data.db.pre-import-2026",
        "data.db.pre-import-20260101T000000Z-journal",
        "data.db.pre-import-20260101T000000Z-0",
    ] {
        let err = snapshots::restore_pre_import_snapshot_in(
            &inner,
            &FixedKeys(Some(KEY)),
            bad,
            Utc::now(),
        )
        .unwrap_err();
        assert!(
            matches!(err, AppError::InvalidArgument(_)),
            "{bad:?}: {err}"
        );
    }

    let err = snapshots::restore_pre_import_snapshot_in(
        &inner,
        &FixedKeys(Some(KEY)),
        "data.db.pre-import-20260101T000000Z",
        Utc::now(),
    )
    .unwrap_err();
    assert!(matches!(err, AppError::NotFound(_)), "{err}");

    match &*inner.lock().unwrap() {
        AppInner::Ready { conn, .. } => assert_eq!(account_names(conn), vec!["旧口座"]),
        _ => panic!("state must stay Ready"),
    }
    assert!(files_with_prefix(dir.path(), REPLACED_PREFIX).is_empty());
    assert_eq!(std::fs::read(dir.path().join("x")).unwrap(), b"decoy");
}

#[test]
fn restore_without_a_key_leaves_the_database_untouched() {
    let dir = tempfile::TempDir::new().unwrap();
    let (mut conn, db_path) = open_live(dir.path(), "旧口座");
    overwrite(&mut conn, &db_path, &payload_with_account("新口座")).unwrap();
    let name = snapshots::list_pre_import_snapshots_in(dir.path()).unwrap()[0]
        .file_name
        .clone();
    let inner = Mutex::new(AppInner::Ready {
        conn,
        db_path: db_path.clone(),
    });

    let err =
        snapshots::restore_pre_import_snapshot_in(&inner, &FixedKeys(None), &name, Utc::now())
            .unwrap_err();

    assert!(matches!(err, AppError::Unavailable(_)), "{err}");
    match &*inner.lock().unwrap() {
        AppInner::Ready { conn, .. } => assert_eq!(account_names(conn), vec!["新口座"]),
        _ => panic!("state must stay Ready"),
    }
    assert!(files_with_prefix(dir.path(), REPLACED_PREFIX).is_empty());
    assert!(!with_suffix(&db_path, RESTORING_SUFFIX).exists());
}

#[test]
fn restore_is_unavailable_during_recovery() {
    let dir = tempfile::TempDir::new().unwrap();
    let inner = Mutex::new(AppInner::Recovery {
        reason: RecoveryReason::DecryptFailed,
        data_dir: dir.path().to_path_buf(),
        db_path: dir.path().join(DB_FILENAME),
    });

    let err = snapshots::restore_pre_import_snapshot_in(
        &inner,
        &FixedKeys(Some(KEY)),
        "data.db.pre-import-20260101T000000Z",
        Utc::now(),
    )
    .unwrap_err();

    assert!(matches!(err, AppError::Unavailable(_)), "{err}");
}
