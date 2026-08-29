use serde::Serialize;
use tauri::State;

use crate::commands::meta::AppState;
use crate::domain::balance;
use crate::error::AppResult;
use crate::infra::repo::balance_repo::{self, AccountBalance};

#[derive(Debug, Clone, Serialize)]
pub struct BalanceList {
    pub accounts: Vec<AccountBalance>,
    pub total_assets: i64,
}

#[tauri::command]
pub fn list_balances(state: State<'_, AppState>) -> AppResult<BalanceList> {
    state.with_conn(|conn| {
        let accounts = balance_repo::list_balances(conn)?;
        let total_assets = balance::total_assets(
            accounts
                .iter()
                .map(|row| (row.archived_at.is_some(), row.balance)),
        );
        Ok(BalanceList {
            accounts,
            total_assets,
        })
    })
}
