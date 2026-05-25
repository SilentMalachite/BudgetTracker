use rusqlite::TransactionBehavior;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};

use crate::commands::meta::AppState;
use crate::domain::ledger::{self, Transaction, TxType};
use crate::error::{AppError, AppResult};
use crate::infra::events::{emit_changed, ChangedDomain};
use crate::infra::repo::transaction_repo;

fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339()
}

#[derive(Debug, Deserialize, Default)]
pub struct ListTransactionFilter {
    pub from: Option<String>,
    pub to: Option<String>,
    #[serde(rename = "type")]
    pub type_: Option<String>,
    pub category_id: Option<i64>,
    pub account_id: Option<i64>,
    pub search: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ListTransactionResult {
    pub items: Vec<Transaction>,
    pub total: u32,
}

#[tauri::command]
pub fn list_transactions(
    state: State<'_, AppState>,
    filter: ListTransactionFilter,
    page: u32,
    page_size: u32,
) -> AppResult<ListTransactionResult> {
    let type_ = filter.type_.as_deref().map(TxType::parse).transpose()?;
    let repo_filter = transaction_repo::ListFilter {
        from: filter.from,
        to: filter.to,
        type_,
        category_id: filter.category_id,
        account_id: filter.account_id,
        search: filter.search,
    };
    let conn = state
        .conn
        .lock()
        .map_err(|_| AppError::Corrupt("connection mutex poisoned".into()))?;
    let (items, total) = transaction_repo::list(&conn, &repo_filter, page, page_size)?;
    Ok(ListTransactionResult { items, total })
}

#[derive(Debug, Deserialize)]
pub struct CreateTransactionInput {
    pub occurred_on: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub amount: i64,
    pub account_id: i64,
    pub category_id: Option<i64>,
    #[serde(default)]
    pub description: String,
}

#[tauri::command]
pub fn create_transaction(
    app: AppHandle,
    state: State<'_, AppState>,
    input: CreateTransactionInput,
) -> AppResult<Transaction> {
    let validated = ledger::validate_input(&ledger::RawInput {
        occurred_on: &input.occurred_on,
        type_: &input.type_,
        amount: input.amount,
        account_id: input.account_id,
        category_id: input.category_id,
        description: &input.description,
    })?;
    let now = now_iso();
    let mut conn = state
        .conn
        .lock()
        .map_err(|_| AppError::Corrupt("connection mutex poisoned".into()))?;
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let id = transaction_repo::insert(
        &tx,
        &transaction_repo::InsertInput {
            occurred_on: &validated.occurred_on,
            type_: validated.type_,
            amount: validated.amount,
            account_id: validated.account_id,
            category_id: validated.category_id,
            description: &validated.description,
            now: &now,
        },
    )?;
    let transaction = transaction_repo::find_by_id(&tx, id)?;
    tx.commit()?;
    drop(conn);
    emit_changed(&app, ChangedDomain::Transactions);
    Ok(transaction)
}

#[derive(Debug, Deserialize)]
pub struct UpdateTransactionPatch {
    pub occurred_on: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub amount: i64,
    pub account_id: i64,
    pub category_id: Option<i64>,
    #[serde(default)]
    pub description: String,
}

#[tauri::command]
pub fn update_transaction(
    app: AppHandle,
    state: State<'_, AppState>,
    id: i64,
    patch: UpdateTransactionPatch,
) -> AppResult<Transaction> {
    let validated = ledger::validate_input(&ledger::RawInput {
        occurred_on: &patch.occurred_on,
        type_: &patch.type_,
        amount: patch.amount,
        account_id: patch.account_id,
        category_id: patch.category_id,
        description: &patch.description,
    })?;
    let now = now_iso();
    let mut conn = state
        .conn
        .lock()
        .map_err(|_| AppError::Corrupt("connection mutex poisoned".into()))?;
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    transaction_repo::update(
        &tx,
        id,
        &transaction_repo::UpdateInput {
            occurred_on: &validated.occurred_on,
            type_: validated.type_,
            amount: validated.amount,
            account_id: validated.account_id,
            category_id: validated.category_id,
            description: &validated.description,
            now: &now,
        },
    )?;
    let transaction = transaction_repo::find_by_id(&tx, id)?;
    tx.commit()?;
    drop(conn);
    emit_changed(&app, ChangedDomain::Transactions);
    Ok(transaction)
}

#[tauri::command]
pub fn delete_transaction(app: AppHandle, state: State<'_, AppState>, id: i64) -> AppResult<()> {
    let mut conn = state
        .conn
        .lock()
        .map_err(|_| AppError::Corrupt("connection mutex poisoned".into()))?;
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    transaction_repo::delete(&tx, id)?;
    tx.commit()?;
    drop(conn);
    emit_changed(&app, ChangedDomain::Transactions);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_input_defaults_missing_description_to_empty() {
        let input: CreateTransactionInput = serde_json::from_str(
            r#"{
                "occurred_on":"2026-05-25",
                "type":"expense",
                "amount":100,
                "account_id":1,
                "category_id":2
            }"#,
        )
        .unwrap();
        assert_eq!(input.description, "");
    }
}
