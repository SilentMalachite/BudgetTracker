use tauri::State;

use crate::commands::meta::AppState;
use crate::error::{AppError, AppResult};
use crate::infra::repo::balance_repo::{self, AccountBalance};

#[tauri::command]
pub fn list_balances(state: State<'_, AppState>) -> AppResult<Vec<AccountBalance>> {
    let conn = state
        .conn
        .lock()
        .map_err(|_| AppError::Corrupt("connection mutex poisoned".into()))?;
    balance_repo::list_balances(&conn)
}
