use tauri::State;

use crate::commands::meta::AppState;
use crate::error::AppResult;
use crate::infra::repo::balance_repo::{self, AccountBalance};

#[tauri::command]
pub fn list_balances(state: State<'_, AppState>) -> AppResult<Vec<AccountBalance>> {
    state.with_conn(balance_repo::list_balances)
}
