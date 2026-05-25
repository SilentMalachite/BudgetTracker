use serde::Deserialize;
use rusqlite::TransactionBehavior;
use tauri::{AppHandle, State};

use crate::commands::meta::AppState;
use crate::domain::account::{self, Account, AccountKind};
use crate::error::{AppError, AppResult};
use crate::infra::events::{emit_changed, ChangedDomain};
use crate::infra::repo::account_repo;

fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339()
}

#[tauri::command]
pub fn list_accounts(
    state: State<'_, AppState>,
    include_archived: bool,
) -> AppResult<Vec<Account>> {
    let conn = state
        .conn
        .lock()
        .map_err(|_| AppError::Corrupt("connection mutex poisoned".into()))?;
    account_repo::list(&conn, include_archived)
}

#[derive(Debug, Deserialize)]
pub struct CreateAccountInput {
    pub name: String,
    pub kind: String,
    pub initial_balance: i64,
    #[serde(default)]
    pub note: String,
}

#[tauri::command]
pub fn create_account(
    app: AppHandle,
    state: State<'_, AppState>,
    input: CreateAccountInput,
) -> AppResult<Account> {
    let name = account::validate_name(&input.name)?;
    let note = account::validate_note(&input.note)?;
    let kind = AccountKind::parse(&input.kind)?;
    let now = now_iso();
    let mut conn = state
        .conn
        .lock()
        .map_err(|_| AppError::Corrupt("connection mutex poisoned".into()))?;
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let order = account_repo::next_display_order(&tx)?;
    let id = account_repo::insert(
        &tx,
        &account_repo::InsertInput {
            name: &name,
            kind,
            currency: "JPY",
            initial_balance: input.initial_balance,
            display_order: order,
            note: &note,
            now: &now,
        },
    )?;
    let account = account_repo::find_by_id(&tx, id)?;
    tx.commit()?;
    drop(conn);
    emit_changed(&app, ChangedDomain::Accounts);
    Ok(account)
}

#[derive(Debug, Deserialize)]
pub struct UpdateAccountPatch {
    pub name: Option<String>,
    pub kind: Option<String>,
    pub initial_balance: Option<i64>,
    pub note: Option<String>,
    pub display_order: Option<i64>,
}

#[tauri::command]
pub fn update_account(
    app: AppHandle,
    state: State<'_, AppState>,
    id: i64,
    patch: UpdateAccountPatch,
) -> AppResult<Account> {
    let name = patch
        .name
        .as_deref()
        .map(account::validate_name)
        .transpose()?;
    let note = patch
        .note
        .as_deref()
        .map(account::validate_note)
        .transpose()?;
    let kind = patch.kind.as_deref().map(AccountKind::parse).transpose()?;
    let now = now_iso();
    let mut conn = state
        .conn
        .lock()
        .map_err(|_| AppError::Corrupt("connection mutex poisoned".into()))?;
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    account_repo::update(
        &tx,
        id,
        &account_repo::UpdatePatch {
            name: name.as_deref(),
            kind,
            initial_balance: patch.initial_balance,
            note: note.as_deref(),
            display_order: patch.display_order,
        },
        &now,
    )?;
    let account = account_repo::find_by_id(&tx, id)?;
    tx.commit()?;
    drop(conn);
    emit_changed(&app, ChangedDomain::Accounts);
    Ok(account)
}

#[tauri::command]
pub fn archive_account(app: AppHandle, state: State<'_, AppState>, id: i64) -> AppResult<()> {
    let now = now_iso();
    let mut conn = state
        .conn
        .lock()
        .map_err(|_| AppError::Corrupt("connection mutex poisoned".into()))?;
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    account_repo::set_archived(&tx, id, Some(&now), &now)?;
    tx.commit()?;
    drop(conn);
    emit_changed(&app, ChangedDomain::Accounts);
    Ok(())
}

#[tauri::command]
pub fn unarchive_account(app: AppHandle, state: State<'_, AppState>, id: i64) -> AppResult<()> {
    let now = now_iso();
    let mut conn = state
        .conn
        .lock()
        .map_err(|_| AppError::Corrupt("connection mutex poisoned".into()))?;
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    account_repo::set_archived(&tx, id, None, &now)?;
    tx.commit()?;
    drop(conn);
    emit_changed(&app, ChangedDomain::Accounts);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_patch_allows_empty_note_to_clear() {
        let patch: UpdateAccountPatch = serde_json::from_str(r#"{"note":""}"#).unwrap();
        assert_eq!(patch.note.as_deref(), Some(""));
    }

    #[test]
    fn update_patch_leaves_note_missing_when_omitted() {
        let patch: UpdateAccountPatch = serde_json::from_str("{}").unwrap();
        assert!(patch.note.is_none());
    }
}
