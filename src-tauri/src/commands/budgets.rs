use rusqlite::{Connection, TransactionBehavior};
use serde::Deserialize;
use tauri::{AppHandle, State};

use crate::commands::meta::AppState;
use crate::domain::budget::{
    self, Budget, BudgetStatus, RawSetBudgetInput, ValidatedSetBudgetInput,
};
use crate::domain::category::CategoryType;
use crate::domain::YearMonth;
use crate::error::{AppError, AppResult};
use crate::infra::events::{emit_changed, ChangedDomain};
use crate::infra::repo::{budget_repo, category_repo};

#[derive(Debug, Clone, Deserialize)]
pub struct SetBudgetInput {
    pub category_id: i64,
    pub year_month: String,
    pub amount: i64,
    pub alert_threshold: i64,
}

fn validate_category_for_budget(conn: &Connection, category_id: i64) -> AppResult<()> {
    let category = category_repo::find_by_id(conn, category_id)?;
    if category.type_ != CategoryType::Expense {
        return Err(AppError::InvalidArgument(format!(
            "category {category_id} is not an expense category"
        )));
    }
    if category.archived_at.is_some() {
        return Err(AppError::InvalidArgument(format!(
            "category {category_id} is archived"
        )));
    }
    Ok(())
}

fn validate_input(input: &SetBudgetInput) -> AppResult<ValidatedSetBudgetInput> {
    budget::validate_set_budget_input(&RawSetBudgetInput {
        category_id: input.category_id,
        year_month: &input.year_month,
        amount: input.amount,
        alert_threshold: input.alert_threshold,
    })
}

pub fn set_budget_for_conn(conn: &Connection, input: SetBudgetInput) -> AppResult<Budget> {
    let validated = validate_input(&input)?;
    validate_category_for_budget(conn, validated.category_id)?;
    budget_repo::set_monthly_budget(conn, &validated)
}

pub fn list_budget_statuses_for_conn(
    conn: &Connection,
    year_month: &str,
    today: chrono::NaiveDate,
) -> AppResult<Vec<BudgetStatus>> {
    budget_repo::list_statuses(conn, YearMonth::parse_key(year_month)?, today)
}

#[tauri::command]
pub fn list_budget_statuses(
    state: State<'_, AppState>,
    year_month: String,
) -> AppResult<Vec<BudgetStatus>> {
    let today = chrono::Local::now().date_naive();
    state.with_conn(|conn| list_budget_statuses_for_conn(conn, &year_month, today))
}

#[tauri::command]
pub fn list_top_budget_statuses(
    state: State<'_, AppState>,
    year_month: String,
    limit: u32,
) -> AppResult<Vec<BudgetStatus>> {
    if !(1..=10).contains(&limit) {
        return Err(AppError::InvalidArgument(format!(
            "limit must be 1..=10, got {limit}"
        )));
    }
    let today = chrono::Local::now().date_naive();
    state.with_conn(|conn| {
        let all = list_budget_statuses_for_conn(conn, &year_month, today)?;
        Ok(budget::select_top_statuses(all, limit as usize))
    })
}

#[tauri::command]
pub fn set_budget(
    app: AppHandle,
    state: State<'_, AppState>,
    input: SetBudgetInput,
) -> AppResult<Budget> {
    let budget = state.with_conn_mut(|conn| {
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let budget = set_budget_for_conn(&tx, input)?;
        tx.commit()?;
        Ok(budget)
    })?;
    emit_changed(&app, ChangedDomain::Budgets);
    Ok(budget)
}
