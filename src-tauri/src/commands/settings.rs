use rusqlite::TransactionBehavior;
use tauri::{AppHandle, State};

use crate::commands::meta::AppState;
use crate::error::{AppError, AppResult};
use crate::infra::events::{emit_changed, ChangedDomain};
use crate::infra::repo::meta_repo;

#[tauri::command]
pub fn get_last_backup_at(state: State<'_, AppState>) -> AppResult<Option<String>> {
    let conn = state
        .conn
        .lock()
        .map_err(|_| AppError::Corrupt("connection mutex poisoned".into()))?;
    meta_repo::get(&conn, "last_backup_at")
}

#[tauri::command]
pub fn set_last_backup_at(
    app: AppHandle,
    state: State<'_, AppState>,
    iso: String,
) -> AppResult<()> {
    let mut conn = state
        .conn
        .lock()
        .map_err(|_| AppError::Corrupt("connection mutex poisoned".into()))?;
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    meta_repo::set(&tx, "last_backup_at", &iso)?;
    tx.commit()?;
    drop(conn);
    emit_changed(&app, ChangedDomain::Meta);
    Ok(())
}

#[tauri::command]
pub fn get_db_path(state: State<'_, AppState>) -> AppResult<String> {
    Ok(state.db_path.to_string_lossy().to_string())
}
