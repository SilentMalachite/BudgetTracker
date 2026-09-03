use rusqlite::{Connection, TransactionBehavior};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};

use crate::commands::meta::AppState;
use crate::domain::date;
use crate::domain::ledger::{self, Transaction, TxType};
use crate::error::{AppError, AppResult};
use crate::infra::events::{emit_changed, ChangedDomain};
use crate::infra::repo::{account_repo, category_repo, transaction_repo};

fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339()
}

fn load_account(conn: &Connection, id: i64) -> AppResult<crate::domain::account::Account> {
    account_repo::find_by_id(conn, id)
}

pub fn prepare_income_expense(
    conn: &Connection,
    validated: &ledger::ValidatedInput,
    allow: ledger::AllowedArchivedRefs,
) -> AppResult<()> {
    let account = load_account(conn, validated.account_id)?;
    ledger::assert_account_writable(&account, allow.account_id)?;
    let category = category_repo::find_by_id(conn, validated.category_id)?;
    ledger::assert_category_matches_tx(&category, validated.type_, allow.category_id)?;
    Ok(())
}

pub fn parse_page_size(page_size: u32) -> AppResult<u32> {
    if !(1..=200).contains(&page_size) {
        return Err(AppError::InvalidArgument(format!(
            "page_size must be 1..=200, got {page_size}"
        )));
    }
    Ok(page_size)
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

/// Turn the wire-level filter into the repo filter, validating everything that
/// ends up compared lexically against stored text in SQL (`occurred_on >= ?`,
/// `occurred_on <= ?`): a non-canonical `from` / `to` would silently
/// mis-filter instead of erroring.
pub fn build_list_filter(filter: ListTransactionFilter) -> AppResult<transaction_repo::ListFilter> {
    let type_ = filter.type_.as_deref().map(TxType::parse).transpose()?;
    if let Some(from) = filter.from.as_deref() {
        date::parse_iso_date("from", from)?;
    }
    if let Some(to) = filter.to.as_deref() {
        date::parse_iso_date("to", to)?;
    }
    Ok(transaction_repo::ListFilter {
        from: filter.from,
        to: filter.to,
        type_,
        category_id: filter.category_id,
        account_id: filter.account_id,
        search: filter.search,
    })
}

#[tauri::command]
pub fn list_transactions(
    state: State<'_, AppState>,
    filter: ListTransactionFilter,
    page: u32,
    page_size: u32,
) -> AppResult<ListTransactionResult> {
    let page_size = parse_page_size(page_size)?;
    let repo_filter = build_list_filter(filter)?;
    let (items, total) =
        state.with_conn(|conn| transaction_repo::list(conn, &repo_filter, page, page_size))?;
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
    let allow = ledger::AllowedArchivedRefs::none();
    let transaction = state.with_conn_mut(|conn| {
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        prepare_income_expense(&tx, &validated, allow)?;
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
        Ok(transaction)
    })?;
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
    let transaction = state.with_conn_mut(|conn| {
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let existing = transaction_repo::find_by_id(&tx, id)?;
        if existing.type_ == TxType::Transfer {
            return Err(AppError::InvalidArgument(
                "use create_transfer or update_transfer for transfer rows".into(),
            ));
        }
        let allow = ledger::AllowedArchivedRefs {
            account_id: Some(existing.account_id),
            category_id: existing.category_id,
            counter_account_id: existing.counter_account_id,
        };
        prepare_income_expense(&tx, &validated, allow)?;
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
        Ok(transaction)
    })?;
    emit_changed(&app, ChangedDomain::Transactions);
    Ok(transaction)
}

#[tauri::command]
pub fn delete_transaction(app: AppHandle, state: State<'_, AppState>, id: i64) -> AppResult<()> {
    state.with_conn_mut(|conn| {
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        transaction_repo::delete(&tx, id)?;
        tx.commit()?;
        Ok(())
    })?;
    emit_changed(&app, ChangedDomain::Transactions);
    Ok(())
}

#[derive(Debug, Deserialize)]
pub struct CreateTransferInput {
    pub occurred_on: String,
    pub amount: i64,
    pub account_id: i64,
    pub counter_account_id: i64,
    #[serde(default)]
    pub description: String,
}

#[tauri::command]
pub fn create_transfer(
    app: AppHandle,
    state: State<'_, AppState>,
    input: CreateTransferInput,
) -> AppResult<Transaction> {
    let validated = ledger::validate_transfer_input(&ledger::RawTransferInput {
        occurred_on: &input.occurred_on,
        amount: input.amount,
        account_id: input.account_id,
        counter_account_id: input.counter_account_id,
        description: &input.description,
    })?;
    let now = now_iso();
    let transaction = state.with_conn_mut(|conn| {
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let from = load_account(&tx, validated.account_id)?;
        ledger::assert_account_writable(&from, None)?;
        let to = load_account(&tx, validated.counter_account_id)?;
        ledger::assert_account_writable(&to, None)?;
        let id = transaction_repo::insert_transfer(
            &tx,
            &transaction_repo::InsertTransferInput {
                occurred_on: &validated.occurred_on,
                amount: validated.amount,
                account_id: validated.account_id,
                counter_account_id: validated.counter_account_id,
                description: &validated.description,
                now: &now,
            },
        )?;
        let transaction = transaction_repo::find_by_id(&tx, id)?;
        tx.commit()?;
        Ok(transaction)
    })?;
    emit_changed(&app, ChangedDomain::Transactions);
    Ok(transaction)
}

#[derive(Debug, Deserialize)]
pub struct UpdateTransferPatch {
    pub occurred_on: String,
    pub amount: i64,
    pub account_id: i64,
    pub counter_account_id: i64,
    #[serde(default)]
    pub description: String,
}

#[tauri::command]
pub fn update_transfer(
    app: AppHandle,
    state: State<'_, AppState>,
    id: i64,
    patch: UpdateTransferPatch,
) -> AppResult<Transaction> {
    let validated = ledger::validate_transfer_input(&ledger::RawTransferInput {
        occurred_on: &patch.occurred_on,
        amount: patch.amount,
        account_id: patch.account_id,
        counter_account_id: patch.counter_account_id,
        description: &patch.description,
    })?;
    let now = now_iso();
    let transaction = state.with_conn_mut(|conn| {
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let existing = transaction_repo::find_by_id(&tx, id)?;
        let allow = ledger::AllowedArchivedRefs {
            account_id: Some(existing.account_id),
            category_id: existing.category_id,
            counter_account_id: existing.counter_account_id,
        };
        let from = load_account(&tx, validated.account_id)?;
        ledger::assert_account_writable(&from, allow.account_id)?;
        let to = load_account(&tx, validated.counter_account_id)?;
        ledger::assert_account_writable(&to, allow.counter_account_id)?;
        transaction_repo::update_transfer(
            &tx,
            id,
            &transaction_repo::UpdateTransferInput {
                occurred_on: &validated.occurred_on,
                amount: validated.amount,
                account_id: validated.account_id,
                counter_account_id: validated.counter_account_id,
                description: &validated.description,
                now: &now,
            },
        )?;
        let transaction = transaction_repo::find_by_id(&tx, id)?;
        tx.commit()?;
        Ok(transaction)
    })?;
    emit_changed(&app, ChangedDomain::Transactions);
    Ok(transaction)
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

    #[test]
    fn create_transfer_input_accepts_minimal_payload() {
        let input: CreateTransferInput = serde_json::from_str(
            r#"{
                "occurred_on": "2026-05-25",
                "amount": 30000,
                "account_id": 1,
                "counter_account_id": 2
            }"#,
        )
        .unwrap();
        assert_eq!(input.amount, 30_000);
        assert_eq!(input.description, "");
    }

    #[test]
    fn list_filter_rejects_non_canonical_from_and_to() {
        // `from` / `to` are compared lexically against occurred_on in SQL, so
        // anything but canonical YYYY-MM-DD silently mis-filters.
        for raw in [
            "2026-5-5",
            " 2026-05-05",
            "+2026-05-05",
            "2026-02-30",
            "2026/05/05",
        ] {
            let err = build_list_filter(ListTransactionFilter {
                from: Some(raw.into()),
                ..Default::default()
            })
            .unwrap_err();
            assert!(
                matches!(err, AppError::InvalidArgument(_)),
                "from={raw:?}: {err:?}"
            );
            assert!(err.to_string().contains("from must be YYYY-MM-DD"), "{err}");

            let err = build_list_filter(ListTransactionFilter {
                to: Some(raw.into()),
                ..Default::default()
            })
            .unwrap_err();
            assert!(
                matches!(err, AppError::InvalidArgument(_)),
                "to={raw:?}: {err:?}"
            );
            assert!(err.to_string().contains("to must be YYYY-MM-DD"), "{err}");
        }
    }

    #[test]
    fn list_filter_accepts_canonical_range_and_absent_dates() {
        let filter = build_list_filter(ListTransactionFilter {
            from: Some("2026-05-01".into()),
            to: Some("2026-05-31".into()),
            type_: Some("expense".into()),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(filter.from.as_deref(), Some("2026-05-01"));
        assert_eq!(filter.to.as_deref(), Some("2026-05-31"));
        assert_eq!(filter.type_, Some(TxType::Expense));

        let empty = build_list_filter(ListTransactionFilter::default()).unwrap();
        assert!(empty.from.is_none() && empty.to.is_none());
    }

    #[test]
    fn parse_page_size_rejects_zero() {
        assert!(parse_page_size(0).is_err());
    }

    #[test]
    fn parse_page_size_rejects_over_200() {
        assert!(parse_page_size(201).is_err());
    }

    #[test]
    fn parse_page_size_accepts_50() {
        assert_eq!(parse_page_size(50).unwrap(), 50);
    }
}
