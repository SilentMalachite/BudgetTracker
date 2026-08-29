use std::path::{Path, PathBuf};
use std::sync::Mutex;

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
        Ok(None) | Err(_) => {
            keys.delete()?;
            keys.create()
        }
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

fn quarantine_if_exists(data_dir: &Path, db_path: &Path) -> AppResult<()> {
    if db_path.exists() {
        let dest = unique_quarantine_path(data_dir, chrono::Utc::now());
        std::fs::rename(db_path, dest)?;
    }
    Ok(())
}

pub fn recover_import(
    inner: &Mutex<AppInner>,
    keys: &dyn KeyStore,
    payload: &str,
) -> AppResult<ImportResult> {
    let mut guard = lock_inner(inner)?;
    let (data_dir, db_path) = match &*guard {
        AppInner::Recovery {
            data_dir, db_path, ..
        } => (data_dir.clone(), db_path.clone()),
        AppInner::Ready { .. } => {
            return Err(AppError::InvalidArgument(
                "recover_import is only available during recovery".into(),
            ));
        }
    };

    let new_path = data_dir.join("data.db.new");
    if new_path.exists() {
        std::fs::remove_file(&new_path)?;
    }
    drop(std::fs::File::create(&new_path)?);

    let result = (|| {
        let key = obtain_recovery_key(keys)?;
        let mut conn = db::open_encrypted(&new_path, &key)?;
        migrations::run(&mut conn)?;
        seed::seed_default_categories_if_needed(&mut conn)?;
        let imported = backup::import_snapshot_json(&mut conn, payload, "overwrite")?;
        drop(conn);
        Ok((key, imported))
    })();

    match result {
        Err(err) => {
            let _ = std::fs::remove_file(&new_path);
            Err(err)
        }
        Ok((key, imported)) => {
            quarantine_if_exists(&data_dir, &db_path)?;
            std::fs::rename(&new_path, &db_path)?;
            let conn = db::open_encrypted(&db_path, &key)?;
            *guard = AppInner::Ready { conn, db_path };
            Ok(imported)
        }
    }
}

pub fn recover_start_empty(inner: &Mutex<AppInner>, keys: &dyn KeyStore) -> AppResult<()> {
    let mut guard = lock_inner(inner)?;
    let (data_dir, db_path) = match &*guard {
        AppInner::Recovery {
            data_dir, db_path, ..
        } => (data_dir.clone(), db_path.clone()),
        AppInner::Ready { .. } => {
            return Err(AppError::InvalidArgument(
                "recover_start_empty is only available during recovery".into(),
            ));
        }
    };

    quarantine_if_exists(&data_dir, &db_path)?;
    let key = obtain_recovery_key(keys)?;
    let mut conn = db::open_encrypted(&db_path, &key)?;
    migrations::run(&mut conn)?;
    seed::seed_default_categories_if_needed(&mut conn)?;
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
    use tempfile::TempDir;

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
}
