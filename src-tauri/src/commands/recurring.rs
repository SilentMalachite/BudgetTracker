//! 定期取引ルールの CRUD と起動時展開。
//!
//! spec §5.4 のとおり、展開は `setup()` ではなくフロントからの明示コマンドで走らせる。
//! 生成件数とスキップ理由を UI に返せること、展開の失敗が起動を止めないことが理由。

use chrono::NaiveDate;
use rusqlite::{Connection, TransactionBehavior};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};

use crate::commands::meta::AppState;
use crate::domain::ledger::{assert_account_writable, assert_category_matches_tx};
use crate::domain::recurring::{
    self, RawRuleInput, RecurringRule, ValidatedRule,
};
use crate::error::AppResult;
use crate::infra::events::{emit_changed, ChangedDomain};
use crate::infra::repo::{account_repo, category_repo, recurring_repo};

#[derive(Debug, Clone, Deserialize)]
pub struct RecurringRuleInput {
    pub name: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub amount: i64,
    pub account_id: i64,
    pub counter_account_id: Option<i64>,
    pub category_id: Option<i64>,
    pub description: String,
    pub frequency: String,
    pub day_of_month: Option<u32>,
    pub day_of_week: Option<u32>,
    pub starts_on: String,
    pub ends_on: Option<String>,
}

/// 一覧の 1 行。ルール本体に「次回予定」を添える。
#[derive(Debug, Clone, Serialize)]
pub struct RecurringRuleView {
    pub rule: RecurringRule,
    pub next_occurrence: Option<String>,
}

fn validate(input: &RecurringRuleInput) -> AppResult<ValidatedRule> {
    recurring::validate_rule_input(&RawRuleInput {
        name: &input.name,
        type_: &input.type_,
        amount: input.amount,
        account_id: input.account_id,
        counter_account_id: input.counter_account_id,
        category_id: input.category_id,
        description: &input.description,
        frequency: &input.frequency,
        day_of_month: input.day_of_month,
        day_of_week: input.day_of_week,
        starts_on: &input.starts_on,
        ends_on: input.ends_on.as_deref(),
    })
}

/// 参照先の口座 / カテゴリが実在し、アーカイブされておらず、種別が噛み合うか。
fn assert_references_usable(conn: &Connection, rule: &ValidatedRule) -> AppResult<()> {
    let account = account_repo::find_by_id(conn, rule.account_id)?;
    assert_account_writable(&account, None)?;

    if let Some(counter_id) = rule.counter_account_id {
        let counter = account_repo::find_by_id(conn, counter_id)?;
        assert_account_writable(&counter, None)?;
    }

    if let Some(category_id) = rule.category_id {
        let category = category_repo::find_by_id(conn, category_id)?;
        assert_category_matches_tx(&category, rule.type_, None)?;
    }

    Ok(())
}

fn to_insert_input<'a>(rule: &'a ValidatedRule, starts_on: &'a str, ends_on: Option<&'a str>) -> recurring_repo::InsertInput<'a> {
    recurring_repo::InsertInput {
        name: &rule.name,
        type_: rule.type_,
        amount: rule.amount,
        account_id: rule.account_id,
        counter_account_id: rule.counter_account_id,
        category_id: rule.category_id,
        description: &rule.description,
        frequency: rule.frequency,
        day_of_month: rule.day_of_month,
        day_of_week: rule.day_of_week,
        starts_on,
        ends_on,
    }
}

pub fn create_rule_for_conn(
    conn: &Connection,
    input: RecurringRuleInput,
) -> AppResult<RecurringRule> {
    let validated = validate(&input)?;
    assert_references_usable(conn, &validated)?;

    let starts_on = validated.starts_on_key();
    let ends_on = validated.ends_on_key();
    let id = recurring_repo::insert(
        conn,
        &to_insert_input(&validated, &starts_on, ends_on.as_deref()),
    )?;
    recurring_repo::find_by_id(conn, id)
}

pub fn update_rule_for_conn(
    conn: &Connection,
    id: i64,
    input: RecurringRuleInput,
) -> AppResult<RecurringRule> {
    let validated = validate(&input)?;
    assert_references_usable(conn, &validated)?;

    let starts_on = validated.starts_on_key();
    let ends_on = validated.ends_on_key();
    recurring_repo::update(
        conn,
        id,
        &recurring_repo::UpdateInput {
            name: &validated.name,
            type_: validated.type_,
            amount: validated.amount,
            account_id: validated.account_id,
            counter_account_id: validated.counter_account_id,
            category_id: validated.category_id,
            description: &validated.description,
            frequency: validated.frequency,
            day_of_month: validated.day_of_month,
            day_of_week: validated.day_of_week,
            starts_on: &starts_on,
            ends_on: ends_on.as_deref(),
        },
    )?;
    recurring_repo::find_by_id(conn, id)
}

pub fn list_rule_views_for_conn(
    conn: &Connection,
    include_inactive: bool,
    today: NaiveDate,
) -> AppResult<Vec<RecurringRuleView>> {
    let rules = recurring_repo::list(conn, include_inactive)?;
    let mut out = Vec::with_capacity(rules.len());
    for rule in rules {
        let schedule = rule.schedule()?;
        // 「次回」は今日より後の最初の 1 件。生成は行わない。
        let next = recurring::next_occurrence(&schedule, today)
            .map(|d| d.format("%Y-%m-%d").to_string());
        out.push(RecurringRuleView {
            rule,
            next_occurrence: next,
        });
    }
    Ok(out)
}

#[tauri::command]
pub fn list_recurring_rules(
    state: State<'_, AppState>,
    include_inactive: bool,
) -> AppResult<Vec<RecurringRuleView>> {
    let today = chrono::Local::now().date_naive();
    state.with_conn(|conn| list_rule_views_for_conn(conn, include_inactive, today))
}

#[tauri::command]
pub fn create_recurring_rule(
    app: AppHandle,
    state: State<'_, AppState>,
    input: RecurringRuleInput,
) -> AppResult<RecurringRule> {
    let rule = state.with_conn_mut(|conn| {
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let rule = create_rule_for_conn(&tx, input)?;
        tx.commit()?;
        Ok(rule)
    })?;
    emit_changed(&app, ChangedDomain::Recurring);
    Ok(rule)
}

#[tauri::command]
pub fn update_recurring_rule(
    app: AppHandle,
    state: State<'_, AppState>,
    id: i64,
    input: RecurringRuleInput,
) -> AppResult<RecurringRule> {
    let rule = state.with_conn_mut(|conn| {
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let rule = update_rule_for_conn(&tx, id, input)?;
        tx.commit()?;
        Ok(rule)
    })?;
    emit_changed(&app, ChangedDomain::Recurring);
    Ok(rule)
}

#[tauri::command]
pub fn set_recurring_rule_active(
    app: AppHandle,
    state: State<'_, AppState>,
    id: i64,
    active: bool,
) -> AppResult<RecurringRule> {
    let rule = state.with_conn_mut(|conn| {
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        recurring_repo::set_active(&tx, id, active)?;
        let rule = recurring_repo::find_by_id(&tx, id)?;
        tx.commit()?;
        Ok(rule)
    })?;
    emit_changed(&app, ChangedDomain::Recurring);
    Ok(rule)
}
