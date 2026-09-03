use chrono::{DateTime, SecondsFormat, Utc};
use rusqlite::TransactionBehavior;
use serde::Serialize;
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

use crate::commands::backup;
use crate::commands::meta::AppState;
use crate::error::{AppError, AppResult};
use crate::infra::events::{emit_changed, ChangedDomain};
use crate::infra::fs::write_atomically;
use crate::infra::repo::meta_repo;

#[tauri::command]
pub fn get_last_backup_at(state: State<'_, AppState>) -> AppResult<Option<String>> {
    state.with_conn(|conn| meta_repo::get(conn, "last_backup_at"))
}

fn record_last_backup_at(app: &AppHandle, state: &AppState, iso: &str) -> AppResult<()> {
    state.with_conn_mut(|conn| {
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        meta_repo::set(&tx, "last_backup_at", iso)?;
        tx.commit()?;
        Ok(())
    })?;
    emit_changed(app, ChangedDomain::Meta);
    Ok(())
}

#[derive(Debug, Serialize)]
pub struct BackupFileResult {
    pub path: String,
    pub last_backup_at: String,
}

fn backup_file_name(now: DateTime<Utc>) -> String {
    format!("budget-backup-{}.json", now.format("%Y%m%dT%H%M%SZ"))
}

/// Ask the user where to save a JSON backup, write it, and only then record
/// `last_backup_at`. The timestamp is the user's evidence that a key-loss
/// recovery is possible, so it must never be set unless a file exists.
///
/// Runs on the blocking thread pool (`async` attribute): the native save
/// dialog blocks until dismissed. Returns `None` when the user cancels.
#[tauri::command(async)]
pub fn export_backup_to_file(
    app: AppHandle,
    state: State<'_, AppState>,
) -> AppResult<Option<BackupFileResult>> {
    let now = Utc::now();
    let Some(chosen) = app
        .dialog()
        .file()
        .set_file_name(backup_file_name(now))
        .add_filter("JSON", &["json"])
        .blocking_save_file()
    else {
        return Ok(None);
    };
    let path = chosen
        .into_path()
        .map_err(|e| AppError::InvalidArgument(format!("unsupported save location: {e}")))?;

    let json = state.with_conn(backup::export_snapshot_json)?;
    write_atomically(&path, json.as_bytes())?;

    let iso = now.to_rfc3339_opts(SecondsFormat::Millis, true);
    record_last_backup_at(&app, &state, &iso)?;
    Ok(Some(BackupFileResult {
        path: path.to_string_lossy().into_owned(),
        last_backup_at: iso,
    }))
}

#[tauri::command]
pub fn get_db_path(state: State<'_, AppState>) -> AppResult<String> {
    Ok(state.db_path()?.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backup_file_name_uses_compact_utc_stamp() {
        let now = DateTime::parse_from_rfc3339("2026-08-29T12:30:45+09:00")
            .unwrap()
            .with_timezone(&Utc);
        assert_eq!(backup_file_name(now), "budget-backup-20260829T033045Z.json");
    }
}
