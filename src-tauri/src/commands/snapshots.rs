//! Safety copies of the live database taken right before an overwrite
//! import, plus the commands that list and restore them.
//!
//! File names mirror the recovery quarantine scheme:
//! `data.db.pre-import-<UTC stamp>[-n]` for a copy and
//! `data.db.replaced-<UTC stamp>[-n]` for the database a restore moved aside.
//! Nothing here deletes user data except the retention pruning of the copies
//! themselves; a restore keeps the replaced database on disk.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use chrono::{DateTime, NaiveDateTime, SecondsFormat, Utc};
use rusqlite::Connection;
use serde::Serialize;
use tauri::{AppHandle, State};

use crate::commands::meta::{AppInner, AppState};
use crate::error::{AppError, AppResult};
use crate::infra::boot::{
    KeyStore, OsKeyStore, RecoveryReason, KEYCHAIN_ACCOUNT, KEYCHAIN_SERVICE,
};
use crate::infra::events::{emit_changed, ChangedDomain};
use crate::infra::keychain::DbKey;
use crate::infra::{db, migrations};

/// Prefix of a safety copy written right before an overwrite import.
pub const PRE_IMPORT_PREFIX: &str = "data.db.pre-import-";
/// Prefix the live database is renamed to when a safety copy is restored.
pub const REPLACED_PREFIX: &str = "data.db.replaced-";
/// Suffix of the staging file a restore builds beside `data.db`.
pub const RESTORING_SUFFIX: &str = ".restoring";
/// How many pre-import copies survive pruning.
pub const KEEP_NEWEST: usize = 3;

const STAMP_FORMAT: &str = "%Y%m%dT%H%M%SZ";
/// SQLite side files that must travel with `data.db`; a hot `-journal` left
/// beside a freshly installed copy would be rolled back into it.
const SIDECAR_SUFFIXES: [&str; 3] = ["-journal", "-wal", "-shm"];

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PreImportSnapshot {
    pub file_name: String,
    /// RFC3339 UTC, parsed from the file name's stamp.
    pub created_at: String,
    pub size_bytes: u64,
}

/// The parts of a pre-import file name: when it was taken and its collision
/// ordinal (`1` for the plain name, `n` for the `-n` suffix). Ordering is
/// chronological, so `max` is the newest copy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SnapshotName {
    pub created_at: DateTime<Utc>,
    pub ordinal: u32,
}

pub fn stamp(now: DateTime<Utc>) -> String {
    now.format(STAMP_FORMAT).to_string()
}

fn is_stamp(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 16
        && b[..8].iter().all(u8::is_ascii_digit)
        && b[8] == b'T'
        && b[9..15].iter().all(u8::is_ascii_digit)
        && b[15] == b'Z'
}

/// Ordinal encoded by the text after a stamp: `""` is `1`, `"-n"` is `n`
/// (n >= 2, no leading zero). Anything else, including SQLite sidecar
/// suffixes such as `-journal`, is `None`.
fn parse_ordinal(rest: &str) -> Option<u32> {
    if rest.is_empty() {
        return Some(1);
    }
    let digits = rest.strip_prefix('-')?;
    if digits.is_empty() || digits.starts_with('0') || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    digits.parse::<u32>().ok().filter(|n| *n >= 2)
}

/// Parse `data.db.pre-import-<stamp>[-n]`. Anything else — including names
/// with path separators — is `None`, so a name that parses is safe to join
/// onto the data directory.
pub fn parse_snapshot_file_name(name: &str) -> Option<SnapshotName> {
    let rest = name.strip_prefix(PRE_IMPORT_PREFIX)?;
    let stamp = rest.get(..16)?;
    let tail = rest.get(16..)?;
    if !is_stamp(stamp) {
        return None;
    }
    let ordinal = parse_ordinal(tail)?;
    let naive = NaiveDateTime::parse_from_str(stamp, STAMP_FORMAT).ok()?;
    Some(SnapshotName {
        created_at: naive.and_utc(),
        ordinal,
    })
}

/// Highest ordinal already used for `base_name` in `data_dir` (0 when none).
/// Pruning may have removed the plain name while `-2` survives, and the next
/// copy must still sort as the newest, so a freed slot is never reused.
fn highest_ordinal(data_dir: &Path, base_name: &str) -> u32 {
    let Ok(entries) = std::fs::read_dir(data_dir) else {
        return 0;
    };
    entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let file_name = entry.file_name();
            parse_ordinal(file_name.to_str()?.strip_prefix(base_name)?)
        })
        .max()
        .unwrap_or(0)
}

/// `<data_dir>/<prefix><stamp>`, or `<prefix><stamp>-<n>` when that stamp is
/// already in use, with `n` one past the highest ordinal present.
pub fn unique_snapshot_path(data_dir: &Path, prefix: &str, now: DateTime<Utc>) -> PathBuf {
    let base_name = format!("{prefix}{}", stamp(now));
    match highest_ordinal(data_dir, &base_name) {
        0 => data_dir.join(base_name),
        n => data_dir.join(format!("{base_name}-{}", n.saturating_add(1))),
    }
}

/// The directory holding `data.db`; snapshots live beside it.
pub fn data_dir_of(db_path: &Path) -> AppResult<PathBuf> {
    db_path
        .parent()
        .filter(|dir| !dir.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .ok_or_else(|| {
            AppError::Corrupt(format!(
                "database path has no parent directory: {}",
                db_path.display()
            ))
        })
}

/// Copy the live database to `data.db.pre-import-<stamp>` with `VACUUM INTO`.
/// SQLCipher (>= 4.3) writes the copy encrypted under the connection's key,
/// so it opens with `db::open_encrypted` and the same key. `VACUUM` cannot
/// run inside a transaction, so call this before the import starts one.
pub fn write_pre_import_snapshot(
    conn: &Connection,
    data_dir: &Path,
    now: DateTime<Utc>,
) -> AppResult<PathBuf> {
    let dest = unique_snapshot_path(data_dir, PRE_IMPORT_PREFIX, now);
    let dest_str = dest.to_str().ok_or_else(|| {
        AppError::Corrupt(format!(
            "snapshot path is not valid UTF-8: {}",
            dest.display()
        ))
    })?;
    // The path is bound, not interpolated: SQLite accepts any expression
    // after INTO, so no quoting is involved.
    conn.execute("VACUUM INTO ?1", [dest_str])?;
    Ok(dest)
}

struct Entry {
    name: SnapshotName,
    file_name: String,
    path: PathBuf,
    size_bytes: u64,
}

/// Pre-import copies in `data_dir`, newest first.
fn snapshot_entries(data_dir: &Path) -> AppResult<Vec<Entry>> {
    let mut found = Vec::new();
    for entry in std::fs::read_dir(data_dir)? {
        let entry = entry?;
        let Some(file_name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        let Some(name) = parse_snapshot_file_name(&file_name) else {
            continue;
        };
        let metadata = entry.metadata()?;
        if !metadata.is_file() {
            continue;
        }
        found.push(Entry {
            name,
            file_name,
            path: entry.path(),
            size_bytes: metadata.len(),
        });
    }
    found.sort_by_key(|entry| std::cmp::Reverse(entry.name));
    Ok(found)
}

pub fn list_pre_import_snapshots_in(data_dir: &Path) -> AppResult<Vec<PreImportSnapshot>> {
    Ok(snapshot_entries(data_dir)?
        .into_iter()
        .map(|entry| PreImportSnapshot {
            file_name: entry.file_name,
            created_at: entry
                .name
                .created_at
                .to_rfc3339_opts(SecondsFormat::Secs, true),
            size_bytes: entry.size_bytes,
        })
        .collect())
}

/// Delete every pre-import copy except the newest `keep`. Returns the paths
/// removed.
pub fn prune_pre_import_snapshots(data_dir: &Path, keep: usize) -> AppResult<Vec<PathBuf>> {
    let mut removed = Vec::new();
    for entry in snapshot_entries(data_dir)?.into_iter().skip(keep) {
        std::fs::remove_file(&entry.path)?;
        removed.push(entry.path);
    }
    Ok(removed)
}

fn lock_inner(inner: &Mutex<AppInner>) -> AppResult<MutexGuard<'_, AppInner>> {
    inner
        .lock()
        .map_err(|_| AppError::Corrupt("connection mutex poisoned".into()))
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(suffix);
    PathBuf::from(name)
}

/// Rename `from` to `to` together with any SQLite sidecar files.
fn rename_with_sidecars(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::rename(from, to)?;
    for suffix in SIDECAR_SUFFIXES {
        let sidecar = with_suffix(from, suffix);
        if sidecar.exists() {
            std::fs::rename(&sidecar, with_suffix(to, suffix))?;
        }
    }
    Ok(())
}

fn obtain_key(keys: &dyn KeyStore) -> AppResult<DbKey> {
    keys.get()?.ok_or_else(|| {
        AppError::Unavailable(
            "encryption key is missing; a snapshot can only be restored with the key it was written under"
                .into(),
        )
    })
}

/// Copy `snapshot` to `staging` and prove it opens and migrates under `key`
/// before anything touches the live database. A bad copy is removed again.
fn stage_snapshot(snapshot: &Path, staging: &Path, key: &DbKey) -> AppResult<()> {
    if staging.exists() {
        std::fs::remove_file(staging)?;
    }
    std::fs::copy(snapshot, staging)?;
    let result = (|| {
        let mut conn = db::open_encrypted(staging, key)?;
        migrations::run(&mut conn)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(staging);
    }
    result
}

/// Move the live file aside, install the staged copy, and open it.
fn swap_and_open(
    db_path: &Path,
    staging: &Path,
    replaced: &Path,
    key: &DbKey,
) -> AppResult<Connection> {
    rename_with_sidecars(db_path, replaced)?;
    std::fs::rename(staging, db_path)?;
    db::open_encrypted(db_path, key)
}

/// Undo a partially completed swap: drop the staging copy and, if the
/// original was already moved to `replaced`, move it back. `replaced` did not
/// exist before the swap, so if it exists it is the original, and anything at
/// `db_path` beside it can only be the staged copy.
fn rollback_swap(db_path: &Path, replaced: &Path, staging: &Path) {
    let _ = std::fs::remove_file(staging);
    if replaced.exists() {
        if db_path.exists() {
            let _ = std::fs::remove_file(db_path);
        }
        let _ = rename_with_sidecars(replaced, db_path);
    }
}

/// Replace the live database with the pre-import copy `file_name`.
///
/// The order keeps the user's data safe at every step: the name is checked
/// against the snapshot pattern (it comes from the frontend), the key is
/// fetched, and the copy is staged and opened *before* the live connection is
/// dropped. Only then is `data.db` renamed to `data.db.replaced-<stamp>` —
/// never deleted — and the staged copy moved into place. A failure after the
/// rename moves the original back. Returns the path of the replaced file.
pub fn restore_pre_import_snapshot_in(
    inner: &Mutex<AppInner>,
    keys: &dyn KeyStore,
    file_name: &str,
    now: DateTime<Utc>,
) -> AppResult<PathBuf> {
    if parse_snapshot_file_name(file_name).is_none() {
        return Err(AppError::InvalidArgument(format!(
            "'{file_name}' is not a pre-import snapshot name"
        )));
    }
    let mut guard = lock_inner(inner)?;
    let db_path = match &*guard {
        AppInner::Ready { db_path, .. } => db_path.clone(),
        AppInner::Recovery { reason, .. } => {
            return Err(AppError::Unavailable(format!(
                "database is in recovery ({reason:?})"
            )));
        }
    };
    let data_dir = data_dir_of(&db_path)?;
    let snapshot = data_dir.join(file_name);
    if !snapshot.is_file() {
        return Err(AppError::NotFound(format!("snapshot {file_name}")));
    }
    let key = obtain_key(keys)?;
    let staging = with_suffix(&db_path, RESTORING_SUFFIX);
    stage_snapshot(&snapshot, &staging, &key)?;

    // Release the live file so it can be renamed (Windows refuses otherwise).
    // The placeholder is only ever observed if the original cannot be
    // reopened after a failed swap; the mutex is held until then.
    let placeholder = AppInner::Recovery {
        reason: RecoveryReason::DecryptFailed,
        data_dir: data_dir.clone(),
        db_path: db_path.clone(),
    };
    if let AppInner::Ready { conn, .. } = std::mem::replace(&mut *guard, placeholder) {
        drop(conn);
    }

    let replaced = unique_snapshot_path(&data_dir, REPLACED_PREFIX, now);
    match swap_and_open(&db_path, &staging, &replaced, &key) {
        Ok(conn) => {
            *guard = AppInner::Ready { conn, db_path };
            Ok(replaced)
        }
        Err(err) => {
            rollback_swap(&db_path, &replaced, &staging);
            if let Ok(conn) = db::open_encrypted(&db_path, &key) {
                *guard = AppInner::Ready { conn, db_path };
            }
            Err(err)
        }
    }
}

fn os_keys() -> OsKeyStore {
    OsKeyStore {
        service: KEYCHAIN_SERVICE.to_string(),
        account: KEYCHAIN_ACCOUNT.to_string(),
    }
}

#[tauri::command]
pub fn list_pre_import_snapshots(state: State<'_, AppState>) -> AppResult<Vec<PreImportSnapshot>> {
    let data_dir = data_dir_of(&state.db_path()?)?;
    list_pre_import_snapshots_in(&data_dir)
}

/// Runs on the blocking pool (`async`): copying a large database must not
/// stall the webview.
#[tauri::command(async)]
pub fn restore_pre_import_snapshot(
    app: AppHandle,
    state: State<'_, AppState>,
    file_name: String,
) -> AppResult<()> {
    restore_pre_import_snapshot_in(&state.inner, &os_keys(), &file_name, Utc::now())?;
    for domain in [
        ChangedDomain::Categories,
        ChangedDomain::Accounts,
        ChangedDomain::Transactions,
        ChangedDomain::Budgets,
        ChangedDomain::Meta,
    ] {
        emit_changed(&app, domain);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn at(rfc3339: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(rfc3339)
            .unwrap()
            .with_timezone(&Utc)
    }

    fn touch(dir: &Path, name: &str, bytes: &[u8]) {
        std::fs::write(dir.join(name), bytes).unwrap();
    }

    fn file_name(path: &Path) -> String {
        path.file_name().unwrap().to_string_lossy().into_owned()
    }

    #[test]
    fn stamp_is_compact_utc() {
        assert_eq!(stamp(at("2026-08-29T12:30:45+09:00")), "20260829T033045Z");
    }

    #[test]
    fn unique_path_uses_the_plain_name_when_free() {
        let dir = TempDir::new().unwrap();
        let path = unique_snapshot_path(dir.path(), PRE_IMPORT_PREFIX, at("2026-08-29T12:30:45Z"));
        assert_eq!(path, dir.path().join("data.db.pre-import-20260829T123045Z"));
    }

    #[test]
    fn unique_path_continues_past_the_highest_ordinal_even_after_pruning() {
        let dir = TempDir::new().unwrap();
        let now = at("2026-08-29T12:30:45Z");
        let next = || file_name(&unique_snapshot_path(dir.path(), PRE_IMPORT_PREFIX, now));

        touch(dir.path(), "data.db.pre-import-20260829T123045Z", b"a");
        assert_eq!(next(), "data.db.pre-import-20260829T123045Z-2");

        touch(dir.path(), "data.db.pre-import-20260829T123045Z-2", b"b");
        std::fs::remove_file(dir.path().join("data.db.pre-import-20260829T123045Z")).unwrap();
        assert_eq!(
            next(),
            "data.db.pre-import-20260829T123045Z-3",
            "a pruned plain name must not be reused: it would sort as the oldest"
        );

        // Sidecars and other prefixes with the same stamp do not count.
        touch(
            dir.path(),
            "data.db.pre-import-20260829T123045Z-2-journal",
            b"j",
        );
        touch(dir.path(), "data.db.replaced-20260829T123045Z-9", b"r");
        assert_eq!(next(), "data.db.pre-import-20260829T123045Z-3");
    }

    #[test]
    fn parse_accepts_plain_and_suffixed_names_in_chronological_order() {
        let plain = parse_snapshot_file_name("data.db.pre-import-20260829T123045Z").unwrap();
        assert_eq!(
            plain,
            SnapshotName {
                created_at: at("2026-08-29T12:30:45Z"),
                ordinal: 1
            }
        );
        let second = parse_snapshot_file_name("data.db.pre-import-20260829T123045Z-2").unwrap();
        assert_eq!(second.ordinal, 2);
        assert!(second > plain);
        let later = parse_snapshot_file_name("data.db.pre-import-20260829T123046Z").unwrap();
        assert!(later > second);
    }

    #[test]
    fn parse_rejects_traversal_sidecars_and_foreign_names() {
        for bad in [
            "",
            "data.db",
            "../x",
            "data.db.pre-import-",
            "data.db.pre-import-2026",
            "data.db.pre-import-20260829T123045Z/../x",
            "data.db.pre-import-20260829T123045Z-journal",
            "data.db.pre-import-20260829T123045Z-0",
            "data.db.pre-import-20260829T123045Z-1",
            "data.db.pre-import-20260829T123045Z-02",
            "data.db.pre-import-20260829T123045Z-",
            "data.db.pre-import-20261329T123045Z",
            "data.db.pre-import-20260829T123045Zx",
            "data.db.corrupt-20260829T123045Z",
            "data.db.pre-import-２０２６0829T123045Z",
        ] {
            assert!(parse_snapshot_file_name(bad).is_none(), "{bad:?}");
        }
    }

    #[test]
    fn list_orders_newest_first_with_sizes() {
        let dir = TempDir::new().unwrap();
        touch(dir.path(), "data.db.pre-import-20260101T000000Z", b"a");
        touch(dir.path(), "data.db.pre-import-20260102T000000Z-2", b"ccc");
        touch(dir.path(), "data.db.pre-import-20260102T000000Z", b"bb");
        touch(dir.path(), "data.db", b"live");
        touch(dir.path(), "data.db.corrupt-20260103T000000Z", b"decoy");

        let listed = list_pre_import_snapshots_in(dir.path()).unwrap();

        assert_eq!(
            listed,
            vec![
                PreImportSnapshot {
                    file_name: "data.db.pre-import-20260102T000000Z-2".into(),
                    created_at: "2026-01-02T00:00:00Z".into(),
                    size_bytes: 3,
                },
                PreImportSnapshot {
                    file_name: "data.db.pre-import-20260102T000000Z".into(),
                    created_at: "2026-01-02T00:00:00Z".into(),
                    size_bytes: 2,
                },
                PreImportSnapshot {
                    file_name: "data.db.pre-import-20260101T000000Z".into(),
                    created_at: "2026-01-01T00:00:00Z".into(),
                    size_bytes: 1,
                },
            ]
        );
    }

    #[test]
    fn prune_keeps_the_newest_three_and_ignores_other_files() {
        let dir = TempDir::new().unwrap();
        for name in [
            "data.db.pre-import-20260101T000000Z",
            "data.db.pre-import-20260102T000000Z",
            "data.db.pre-import-20260102T000000Z-2",
            "data.db.pre-import-20260103T000000Z",
            "data.db.pre-import-20260103T000000Z-2",
        ] {
            touch(dir.path(), name, b"x");
        }
        touch(dir.path(), "data.db", b"live");
        touch(dir.path(), "data.db.corrupt-20250101T000000Z", b"decoy");
        touch(dir.path(), "data.db.replaced-20250101T000000Z", b"decoy");

        let removed = prune_pre_import_snapshots(dir.path(), KEEP_NEWEST).unwrap();

        let mut removed_names: Vec<String> = removed.iter().map(|p| file_name(p)).collect();
        removed_names.sort();
        assert_eq!(
            removed_names,
            [
                "data.db.pre-import-20260101T000000Z",
                "data.db.pre-import-20260102T000000Z",
            ]
        );
        let remaining: Vec<String> = list_pre_import_snapshots_in(dir.path())
            .unwrap()
            .into_iter()
            .map(|s| s.file_name)
            .collect();
        assert_eq!(
            remaining,
            [
                "data.db.pre-import-20260103T000000Z-2",
                "data.db.pre-import-20260103T000000Z",
                "data.db.pre-import-20260102T000000Z-2",
            ]
        );
        assert!(dir.path().join("data.db").exists());
        assert!(dir.path().join("data.db.corrupt-20250101T000000Z").exists());
        assert!(dir
            .path()
            .join("data.db.replaced-20250101T000000Z")
            .exists());
    }

    #[test]
    fn prune_with_at_most_keep_copies_removes_nothing() {
        let dir = TempDir::new().unwrap();
        touch(dir.path(), "data.db.pre-import-20260101T000000Z", b"x");
        touch(dir.path(), "data.db.pre-import-20260102T000000Z", b"x");

        let removed = prune_pre_import_snapshots(dir.path(), KEEP_NEWEST).unwrap();

        assert!(removed.is_empty());
        assert_eq!(list_pre_import_snapshots_in(dir.path()).unwrap().len(), 2);
    }

    #[test]
    fn data_dir_is_the_parent_of_the_database() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("data.db");
        assert_eq!(data_dir_of(&db_path).unwrap(), dir.path());
        assert!(data_dir_of(Path::new("data.db")).is_err());
    }
}
