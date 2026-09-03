use std::collections::HashMap;
use std::path::Path;

use chrono::Datelike;
use rusqlite::{params, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tauri::{AppHandle, State};

use crate::commands::meta::AppState;
use crate::commands::snapshots;
use crate::domain::account::{self, AccountKind};
use crate::domain::budget::{self, RawSetBudgetInput};
use crate::domain::category::{self, Category, CategoryType};
use crate::domain::date::parse_iso_date;
use crate::domain::ledger::{self, TxType};
use crate::error::{AppError, AppResult, ImportRowError};
use crate::infra::events::{emit_changed, ChangedDomain};
use crate::infra::repo::meta_repo;

const BACKUP_FORMAT_VERSION: u32 = 1;
const MAX_PAYLOAD_BYTES: usize = 10 * 1024 * 1024;

#[derive(Debug, Serialize, Deserialize)]
struct Snapshot {
    schema_version: u32,
    exported_at: String,
    categories: Vec<serde_json::Value>,
    accounts: Vec<serde_json::Value>,
    recurring_rules: Vec<serde_json::Value>,
    transactions: Vec<serde_json::Value>,
    budgets: Vec<serde_json::Value>,
    app_meta: Vec<serde_json::Value>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ImportMode {
    Overwrite,
    Append,
}

impl ImportMode {
    fn parse(raw: &str) -> AppResult<Self> {
        match raw {
            "overwrite" => Ok(Self::Overwrite),
            "append" => Ok(Self::Append),
            other => Err(AppError::InvalidArgument(format!("unknown mode '{other}'"))),
        }
    }
}

fn select_all(
    conn: &rusqlite::Connection,
    sql: &str,
    cols: &[&str],
) -> AppResult<Vec<serde_json::Value>> {
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map([], |row| {
        let mut obj = serde_json::Map::new();
        for (i, col) in cols.iter().enumerate() {
            let value: rusqlite::types::Value = row.get(i)?;
            let json_value = match value {
                rusqlite::types::Value::Null => serde_json::Value::Null,
                rusqlite::types::Value::Integer(v) => json!(v),
                rusqlite::types::Value::Real(v) => json!(v),
                rusqlite::types::Value::Text(v) => json!(v),
                rusqlite::types::Value::Blob(_) => serde_json::Value::Null,
            };
            obj.insert((*col).to_string(), json_value);
        }
        Ok(serde_json::Value::Object(obj))
    })?;
    rows.collect::<rusqlite::Result<_>>().map_err(AppError::Db)
}

pub fn export_snapshot_json(conn: &rusqlite::Connection) -> AppResult<String> {
    let snap = Snapshot {
        schema_version: BACKUP_FORMAT_VERSION,
        exported_at: chrono::Utc::now().to_rfc3339(),
        categories: select_all(
            conn,
            "SELECT id, name, type, color, icon, display_order, archived_at
               FROM categories ORDER BY id",
            &[
                "id",
                "name",
                "type",
                "color",
                "icon",
                "display_order",
                "archived_at",
            ],
        )?,
        accounts: select_all(
            conn,
            "SELECT id, name, kind, currency, initial_balance, display_order, note,
                    archived_at, created_at, updated_at
               FROM accounts ORDER BY id",
            &[
                "id",
                "name",
                "kind",
                "currency",
                "initial_balance",
                "display_order",
                "note",
                "archived_at",
                "created_at",
                "updated_at",
            ],
        )?,
        recurring_rules: select_all(
            conn,
            "SELECT id, name, type, amount, account_id, counter_account_id, category_id,
                    description, frequency, day_of_month, day_of_week, starts_on, ends_on,
                    last_generated_on, active
               FROM recurring_rules ORDER BY id",
            &[
                "id",
                "name",
                "type",
                "amount",
                "account_id",
                "counter_account_id",
                "category_id",
                "description",
                "frequency",
                "day_of_month",
                "day_of_week",
                "starts_on",
                "ends_on",
                "last_generated_on",
                "active",
            ],
        )?,
        transactions: select_all(
            conn,
            "SELECT id, occurred_on, type, amount, account_id, counter_account_id,
                    category_id, description, recurring_id, created_at, updated_at
               FROM transactions ORDER BY id",
            &[
                "id",
                "occurred_on",
                "type",
                "amount",
                "account_id",
                "counter_account_id",
                "category_id",
                "description",
                "recurring_id",
                "created_at",
                "updated_at",
            ],
        )?,
        budgets: select_all(
            conn,
            "SELECT id, category_id, period, amount, starts_on, ends_on, alert_threshold
               FROM budgets ORDER BY id",
            &[
                "id",
                "category_id",
                "period",
                "amount",
                "starts_on",
                "ends_on",
                "alert_threshold",
            ],
        )?,
        app_meta: select_all(
            conn,
            "SELECT key, value FROM app_meta ORDER BY key",
            &["key", "value"],
        )?,
    };
    serde_json::to_string_pretty(&snap)
        .map_err(|e| AppError::Corrupt(format!("encode snapshot: {e}")))
}

#[tauri::command]
pub fn export_json(state: State<'_, AppState>) -> AppResult<String> {
    state.with_conn(export_snapshot_json)
}

#[derive(Debug, Deserialize)]
pub struct ImportArgs {
    pub payload: String,
    pub mode: String,
}

#[derive(Debug, Serialize)]
pub struct ImportResult {
    pub categories: u32,
    pub accounts: u32,
    pub transactions: u32,
    pub budgets: u32,
    pub warnings: Vec<String>,
}

fn value_i64(row: &serde_json::Value, key: &str) -> Option<i64> {
    row.get(key).and_then(|v| v.as_i64())
}

fn value_str<'a>(row: &'a serde_json::Value, key: &str) -> Option<&'a str> {
    row.get(key).and_then(|v| v.as_str())
}

fn require_i64(row: &serde_json::Value, key: &str, label: &str) -> AppResult<i64> {
    value_i64(row, key)
        .ok_or_else(|| AppError::InvalidArgument(format!("{label} missing required {key}")))
}

fn resolve_required(
    map: &HashMap<i64, i64>,
    old_id: i64,
    label: &str,
    mode: ImportMode,
    warnings: &mut Vec<String>,
) -> AppResult<Option<i64>> {
    if let Some(new_id) = map.get(&old_id) {
        return Ok(Some(*new_id));
    }
    let message = format!("skipped {label} referencing missing id {old_id}");
    if mode == ImportMode::Append {
        warnings.push(message);
        Ok(None)
    } else {
        Err(AppError::InvalidArgument(message))
    }
}

fn resolve_optional(
    map: &HashMap<i64, i64>,
    old_id: Option<i64>,
    label: &str,
    mode: ImportMode,
    warnings: &mut Vec<String>,
) -> AppResult<Option<Option<i64>>> {
    match old_id {
        Some(id) => resolve_required(map, id, label, mode, warnings).map(|v| v.map(Some)),
        None => Ok(Some(None)),
    }
}

fn existing_category_id(
    tx: &rusqlite::Transaction<'_>,
    name: &str,
    type_: &str,
) -> AppResult<Option<i64>> {
    tx.query_row(
        "SELECT id FROM categories WHERE name = ?1 AND type = ?2",
        params![name, type_],
        |row| row.get(0),
    )
    .optional()
    .map_err(AppError::Db)
}

fn insert_account(
    tx: &rusqlite::Transaction<'_>,
    account: &serde_json::Value,
    mode: ImportMode,
) -> AppResult<i64> {
    if mode == ImportMode::Overwrite {
        tx.execute(
            "INSERT INTO accounts(id, name, kind, currency, initial_balance, display_order,
                                  note, archived_at, created_at, updated_at)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                value_i64(account, "id"),
                value_str(account, "name").unwrap_or(""),
                value_str(account, "kind").unwrap_or("cash"),
                value_str(account, "currency").unwrap_or("JPY"),
                value_i64(account, "initial_balance").unwrap_or(0),
                value_i64(account, "display_order").unwrap_or(0),
                value_str(account, "note").unwrap_or(""),
                value_str(account, "archived_at"),
                value_str(account, "created_at").unwrap_or("1970-01-01T00:00:00Z"),
                value_str(account, "updated_at").unwrap_or("1970-01-01T00:00:00Z"),
            ],
        )?;
        Ok(value_i64(account, "id").unwrap_or_else(|| tx.last_insert_rowid()))
    } else {
        tx.execute(
            "INSERT INTO accounts(name, kind, currency, initial_balance, display_order,
                                  note, archived_at, created_at, updated_at)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                value_str(account, "name").unwrap_or(""),
                value_str(account, "kind").unwrap_or("cash"),
                value_str(account, "currency").unwrap_or("JPY"),
                value_i64(account, "initial_balance").unwrap_or(0),
                value_i64(account, "display_order").unwrap_or(0),
                value_str(account, "note").unwrap_or(""),
                value_str(account, "archived_at"),
                value_str(account, "created_at").unwrap_or("1970-01-01T00:00:00Z"),
                value_str(account, "updated_at").unwrap_or("1970-01-01T00:00:00Z"),
            ],
        )?;
        Ok(tx.last_insert_rowid())
    }
}

fn insert_category(
    tx: &rusqlite::Transaction<'_>,
    category: &serde_json::Value,
    mode: ImportMode,
    warnings: &mut Vec<String>,
) -> AppResult<Option<(i64, bool)>> {
    let name = value_str(category, "name").unwrap_or("");
    let type_ = value_str(category, "type").unwrap_or("expense");
    let result = if mode == ImportMode::Overwrite {
        tx.execute(
            "INSERT INTO categories(id, name, type, color, icon, display_order, archived_at)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                value_i64(category, "id"),
                name,
                type_,
                value_str(category, "color"),
                value_str(category, "icon"),
                value_i64(category, "display_order").unwrap_or(0),
                value_str(category, "archived_at"),
            ],
        )
    } else {
        tx.execute(
            "INSERT INTO categories(name, type, color, icon, display_order, archived_at)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                name,
                type_,
                value_str(category, "color"),
                value_str(category, "icon"),
                value_i64(category, "display_order").unwrap_or(0),
                value_str(category, "archived_at"),
            ],
        )
    };

    match result {
        Ok(_) => Ok(Some((
            if mode == ImportMode::Overwrite {
                value_i64(category, "id").unwrap_or_else(|| tx.last_insert_rowid())
            } else {
                tx.last_insert_rowid()
            },
            true,
        ))),
        Err(rusqlite::Error::SqliteFailure(err, _))
            if mode == ImportMode::Append
                && err.code == rusqlite::ErrorCode::ConstraintViolation =>
        {
            let Some(existing_id) = existing_category_id(tx, name, type_)? else {
                return Err(AppError::Conflict(format!(
                    "duplicate category without matching row: {name}"
                )));
            };
            warnings.push(format!("skipped duplicate category: {name}"));
            Ok(Some((existing_id, false)))
        }
        Err(e) => Err(AppError::Db(e)),
    }
}

fn insert_recurring_rule(
    tx: &rusqlite::Transaction<'_>,
    rule: &serde_json::Value,
    mode: ImportMode,
    accounts: &HashMap<i64, i64>,
    categories: &HashMap<i64, i64>,
    warnings: &mut Vec<String>,
) -> AppResult<Option<i64>> {
    let old_account = require_i64(rule, "account_id", "recurring rule")?;
    let Some(account_id) = resolve_required(
        accounts,
        old_account,
        "recurring rule account",
        mode,
        warnings,
    )?
    else {
        return Ok(None);
    };
    let Some(counter_account_id) = resolve_optional(
        accounts,
        value_i64(rule, "counter_account_id"),
        "recurring rule counter account",
        mode,
        warnings,
    )?
    else {
        return Ok(None);
    };
    let Some(category_id) = resolve_optional(
        categories,
        value_i64(rule, "category_id"),
        "recurring rule category",
        mode,
        warnings,
    )?
    else {
        return Ok(None);
    };

    if mode == ImportMode::Overwrite {
        tx.execute(
            "INSERT INTO recurring_rules(id, name, type, amount, account_id, counter_account_id,
                                         category_id, description, frequency, day_of_month,
                                         day_of_week, starts_on, ends_on, last_generated_on, active)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            params![
                value_i64(rule, "id"),
                value_str(rule, "name").unwrap_or(""),
                value_str(rule, "type").unwrap_or("expense"),
                value_i64(rule, "amount").unwrap_or(0),
                account_id,
                counter_account_id,
                category_id,
                value_str(rule, "description").unwrap_or(""),
                value_str(rule, "frequency").unwrap_or("monthly"),
                value_i64(rule, "day_of_month"),
                value_i64(rule, "day_of_week"),
                value_str(rule, "starts_on").unwrap_or("1970-01-01"),
                value_str(rule, "ends_on"),
                value_str(rule, "last_generated_on"),
                value_i64(rule, "active").unwrap_or(1),
            ],
        )?;
        Ok(Some(
            value_i64(rule, "id").unwrap_or_else(|| tx.last_insert_rowid()),
        ))
    } else {
        tx.execute(
            "INSERT INTO recurring_rules(name, type, amount, account_id, counter_account_id,
                                         category_id, description, frequency, day_of_month,
                                         day_of_week, starts_on, ends_on, last_generated_on, active)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                value_str(rule, "name").unwrap_or(""),
                value_str(rule, "type").unwrap_or("expense"),
                value_i64(rule, "amount").unwrap_or(0),
                account_id,
                counter_account_id,
                category_id,
                value_str(rule, "description").unwrap_or(""),
                value_str(rule, "frequency").unwrap_or("monthly"),
                value_i64(rule, "day_of_month"),
                value_i64(rule, "day_of_week"),
                value_str(rule, "starts_on").unwrap_or("1970-01-01"),
                value_str(rule, "ends_on"),
                value_str(rule, "last_generated_on"),
                value_i64(rule, "active").unwrap_or(1),
            ],
        )?;
        Ok(Some(tx.last_insert_rowid()))
    }
}

fn insert_transaction(
    tx: &rusqlite::Transaction<'_>,
    transaction: &serde_json::Value,
    mode: ImportMode,
    accounts: &HashMap<i64, i64>,
    categories: &HashMap<i64, i64>,
    recurring_rules: &HashMap<i64, i64>,
    warnings: &mut Vec<String>,
) -> AppResult<bool> {
    let old_account = require_i64(transaction, "account_id", "transaction")?;
    let Some(account_id) =
        resolve_required(accounts, old_account, "transaction account", mode, warnings)?
    else {
        return Ok(false);
    };
    let Some(counter_account_id) = resolve_optional(
        accounts,
        value_i64(transaction, "counter_account_id"),
        "transaction counter account",
        mode,
        warnings,
    )?
    else {
        return Ok(false);
    };
    let Some(category_id) = resolve_optional(
        categories,
        value_i64(transaction, "category_id"),
        "transaction category",
        mode,
        warnings,
    )?
    else {
        return Ok(false);
    };
    let Some(recurring_id) = resolve_optional(
        recurring_rules,
        value_i64(transaction, "recurring_id"),
        "transaction recurring rule",
        mode,
        warnings,
    )?
    else {
        return Ok(false);
    };

    if mode == ImportMode::Overwrite {
        tx.execute(
            "INSERT INTO transactions(id, occurred_on, type, amount, account_id,
                                      counter_account_id, category_id, description,
                                      recurring_id, created_at, updated_at)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                value_i64(transaction, "id"),
                value_str(transaction, "occurred_on").unwrap_or(""),
                value_str(transaction, "type").unwrap_or("expense"),
                value_i64(transaction, "amount").unwrap_or(0),
                account_id,
                counter_account_id,
                category_id,
                value_str(transaction, "description").unwrap_or(""),
                recurring_id,
                value_str(transaction, "created_at").unwrap_or("1970-01-01T00:00:00Z"),
                value_str(transaction, "updated_at").unwrap_or("1970-01-01T00:00:00Z"),
            ],
        )?;
    } else {
        tx.execute(
            "INSERT INTO transactions(occurred_on, type, amount, account_id,
                                      counter_account_id, category_id, description,
                                      recurring_id, created_at, updated_at)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                value_str(transaction, "occurred_on").unwrap_or(""),
                value_str(transaction, "type").unwrap_or("expense"),
                value_i64(transaction, "amount").unwrap_or(0),
                account_id,
                counter_account_id,
                category_id,
                value_str(transaction, "description").unwrap_or(""),
                recurring_id,
                value_str(transaction, "created_at").unwrap_or("1970-01-01T00:00:00Z"),
                value_str(transaction, "updated_at").unwrap_or("1970-01-01T00:00:00Z"),
            ],
        )?;
    }
    Ok(true)
}

fn insert_budget(
    tx: &rusqlite::Transaction<'_>,
    budget: &serde_json::Value,
    mode: ImportMode,
    categories: &HashMap<i64, i64>,
    warnings: &mut Vec<String>,
) -> AppResult<bool> {
    let old_category = require_i64(budget, "category_id", "budget")?;
    let Some(category_id) =
        resolve_required(categories, old_category, "budget category", mode, warnings)?
    else {
        return Ok(false);
    };
    let result = if mode == ImportMode::Overwrite {
        tx.execute(
            "INSERT INTO budgets(id, category_id, period, amount, starts_on, ends_on, alert_threshold)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                value_i64(budget, "id"),
                category_id,
                value_str(budget, "period").unwrap_or("monthly"),
                value_i64(budget, "amount").unwrap_or(0),
                value_str(budget, "starts_on").unwrap_or("1970-01-01"),
                value_str(budget, "ends_on"),
                value_i64(budget, "alert_threshold").unwrap_or(80),
            ],
        )
    } else {
        tx.execute(
            "INSERT INTO budgets(category_id, period, amount, starts_on, ends_on, alert_threshold)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                category_id,
                value_str(budget, "period").unwrap_or("monthly"),
                value_i64(budget, "amount").unwrap_or(0),
                value_str(budget, "starts_on").unwrap_or("1970-01-01"),
                value_str(budget, "ends_on"),
                value_i64(budget, "alert_threshold").unwrap_or(80),
            ],
        )
    };

    match result {
        Ok(_) => Ok(true),
        Err(rusqlite::Error::SqliteFailure(err, _))
            if mode == ImportMode::Append
                && err.code == rusqlite::ErrorCode::ConstraintViolation =>
        {
            warnings.push(format!(
                "skipped duplicate budget for category {category_id}"
            ));
            Ok(false)
        }
        Err(e) => Err(AppError::Db(e)),
    }
}

fn push_error(
    errors: &mut Vec<ImportRowError>,
    index: u32,
    entity: &str,
    message: impl Into<String>,
) {
    errors.push(ImportRowError {
        index,
        entity: entity.to_string(),
        message: message.into(),
    });
}

fn domain_message(err: AppError) -> String {
    match err {
        AppError::InvalidArgument(msg) => msg,
        other => other.to_string(),
    }
}

fn required_str<'a>(
    row: &'a serde_json::Value,
    key: &str,
    index: u32,
    entity: &str,
    errors: &mut Vec<ImportRowError>,
) -> Option<&'a str> {
    match value_str(row, key) {
        Some(value) => Some(value),
        None => {
            push_error(errors, index, entity, format!("missing required {key}"));
            None
        }
    }
}

fn required_i64(
    row: &serde_json::Value,
    key: &str,
    index: u32,
    entity: &str,
    errors: &mut Vec<ImportRowError>,
) -> Option<i64> {
    match value_i64(row, key) {
        Some(value) => Some(value),
        None => {
            push_error(errors, index, entity, format!("missing required {key}"));
            None
        }
    }
}

fn validate_account_row(index: u32, row: &serde_json::Value, errors: &mut Vec<ImportRowError>) {
    const ENTITY: &str = "account";
    if let Some(name) = required_str(row, "name", index, ENTITY, errors) {
        if let Err(err) = account::validate_name(name) {
            push_error(errors, index, ENTITY, domain_message(err));
        }
    }
    if let Some(kind) = required_str(row, "kind", index, ENTITY, errors) {
        if let Err(err) = AccountKind::parse(kind) {
            push_error(errors, index, ENTITY, domain_message(err));
        }
    }
    let _ = required_i64(row, "initial_balance", index, ENTITY, errors);
    if let Some(note) = value_str(row, "note") {
        if let Err(err) = account::validate_note(note) {
            push_error(errors, index, ENTITY, domain_message(err));
        }
    }
}

fn validate_category_row(index: u32, row: &serde_json::Value, errors: &mut Vec<ImportRowError>) {
    const ENTITY: &str = "category";
    if let Some(name) = required_str(row, "name", index, ENTITY, errors) {
        if let Err(err) = category::validate_name(name) {
            push_error(errors, index, ENTITY, domain_message(err));
        }
    }
    if let Some(type_) = required_str(row, "type", index, ENTITY, errors) {
        if let Err(err) = CategoryType::parse(type_) {
            push_error(errors, index, ENTITY, domain_message(err));
        }
    }
    // The frontend interpolates `color` into an inline style attribute, so it
    // must be a strict #RRGGBB exactly as create_category / update_category require.
    if let Some(color) = value_str(row, "color") {
        if let Err(err) = category::validate_color(color) {
            push_error(errors, index, ENTITY, domain_message(err));
        }
    }
}

/// Categories declared in the snapshot, keyed by their *snapshot* id, so that
/// transaction and budget rows can be checked against the category they will
/// be attached to before anything is inserted. Rows that fail their own
/// validation are omitted here — they already produce a `category[i]` error —
/// and rows referencing an id that is absent are left to the insert path,
/// which warns-and-skips in append mode and errors in overwrite mode.
fn snapshot_categories(snap: &Snapshot) -> HashMap<i64, Category> {
    snap.categories
        .iter()
        .filter_map(|row| {
            let id = value_i64(row, "id")?;
            let type_ = CategoryType::parse(value_str(row, "type")?).ok()?;
            Some((
                id,
                Category {
                    id,
                    name: value_str(row, "name").unwrap_or("").to_string(),
                    type_,
                    color: value_str(row, "color").map(str::to_string),
                    icon: value_str(row, "icon").map(str::to_string),
                    display_order: value_i64(row, "display_order").unwrap_or(0),
                    archived_at: value_str(row, "archived_at").map(str::to_string),
                },
            ))
        })
        .collect()
}

fn validate_income_expense_row(
    index: u32,
    type_: &str,
    row: &serde_json::Value,
    categories: &HashMap<i64, Category>,
    errors: &mut Vec<ImportRowError>,
) {
    const ENTITY: &str = "transaction";
    let occurred_on = required_str(row, "occurred_on", index, ENTITY, errors);
    let amount = required_i64(row, "amount", index, ENTITY, errors);
    let account_id = required_i64(row, "account_id", index, ENTITY, errors);
    let (Some(occurred_on), Some(amount), Some(account_id)) = (occurred_on, amount, account_id)
    else {
        return;
    };
    let validated = match ledger::validate_input(&ledger::RawInput {
        occurred_on,
        type_,
        amount,
        account_id,
        category_id: value_i64(row, "category_id"),
        description: value_str(row, "description").unwrap_or(""),
    }) {
        Ok(validated) => validated,
        Err(err) => {
            push_error(errors, index, ENTITY, domain_message(err));
            return;
        }
    };
    if let Some(category) = categories.get(&validated.category_id) {
        // Same check as create_transaction, except that an archived category is
        // allowed: a backup taken after archiving must still restore its history
        // (rule 5), so `allow_id` is the category itself and only the type is enforced.
        if let Err(err) =
            ledger::assert_category_matches_tx(category, validated.type_, Some(category.id))
        {
            push_error(errors, index, ENTITY, domain_message(err));
        }
    }
}

fn validate_transfer_row(index: u32, row: &serde_json::Value, errors: &mut Vec<ImportRowError>) {
    const ENTITY: &str = "transfer";
    let occurred_on = required_str(row, "occurred_on", index, ENTITY, errors);
    let amount = required_i64(row, "amount", index, ENTITY, errors);
    let account_id = required_i64(row, "account_id", index, ENTITY, errors);
    let counter_account_id = required_i64(row, "counter_account_id", index, ENTITY, errors);
    if row.get("category_id").is_some_and(|v| !v.is_null()) {
        push_error(
            errors,
            index,
            ENTITY,
            "transfer rows must not have a category_id",
        );
    }
    let (Some(occurred_on), Some(amount), Some(account_id), Some(counter_account_id)) =
        (occurred_on, amount, account_id, counter_account_id)
    else {
        return;
    };
    if let Err(err) = ledger::validate_transfer_input(&ledger::RawTransferInput {
        occurred_on,
        amount,
        account_id,
        counter_account_id,
        description: value_str(row, "description").unwrap_or(""),
    }) {
        push_error(errors, index, ENTITY, domain_message(err));
    }
}

fn validate_transaction_row(
    index: u32,
    row: &serde_json::Value,
    categories: &HashMap<i64, Category>,
    errors: &mut Vec<ImportRowError>,
) {
    let Some(type_) = required_str(row, "type", index, "transaction", errors) else {
        return;
    };
    if type_ == "transfer" {
        validate_transfer_row(index, row, errors);
    } else {
        validate_income_expense_row(index, type_, row, categories, errors);
    }
}

fn validate_budget_row(
    index: u32,
    row: &serde_json::Value,
    categories: &HashMap<i64, Category>,
    errors: &mut Vec<ImportRowError>,
) {
    const ENTITY: &str = "budget";
    let category_id = required_i64(row, "category_id", index, ENTITY, errors);
    let period = required_str(row, "period", index, ENTITY, errors);
    let amount = required_i64(row, "amount", index, ENTITY, errors);
    let starts_on = required_str(row, "starts_on", index, ENTITY, errors);
    let alert_threshold = required_i64(row, "alert_threshold", index, ENTITY, errors);
    let (Some(category_id), Some(period), Some(amount), Some(starts_on), Some(alert_threshold)) =
        (category_id, period, amount, starts_on, alert_threshold)
    else {
        return;
    };

    // set_budget only ever writes monthly budgets anchored on the 1st, so an
    // imported row must have the same shape or the month lookups will miss it.
    if period != "monthly" {
        push_error(
            errors,
            index,
            ENTITY,
            format!("period must be 'monthly', got '{period}'"),
        );
    }
    let starts = match parse_iso_date("starts_on", starts_on) {
        Ok(date) => date,
        Err(err) => {
            push_error(errors, index, ENTITY, domain_message(err));
            return;
        }
    };
    if starts.day() != 1 {
        push_error(
            errors,
            index,
            ENTITY,
            format!("starts_on must be the first day of the month, got '{starts_on}'"),
        );
    }
    if let Some(ends_on) = value_str(row, "ends_on") {
        match parse_iso_date("ends_on", ends_on) {
            Ok(ends) if ends < starts => push_error(
                errors,
                index,
                ENTITY,
                format!("ends_on '{ends_on}' is before starts_on '{starts_on}'"),
            ),
            Ok(_) => {}
            Err(err) => push_error(errors, index, ENTITY, domain_message(err)),
        }
    }
    if let Err(err) = budget::validate_set_budget_input(&RawSetBudgetInput {
        category_id,
        // Safe: parse_iso_date guaranteed exactly ten ASCII bytes.
        year_month: &starts_on[..7],
        amount,
        alert_threshold,
    }) {
        push_error(errors, index, ENTITY, domain_message(err));
    }
    // Same type rule as commands::budgets::validate_category_for_budget. The
    // archived check is deliberately *not* mirrored: exports contain budgets on
    // categories archived later, and those must round-trip (rule 5).
    if let Some(category) = categories.get(&category_id) {
        if category.type_ != CategoryType::Expense {
            push_error(
                errors,
                index,
                ENTITY,
                format!("category {category_id} is not an expense category"),
            );
        }
    }
}

/// Mirror the V001 CHECK constraints on `recurring_rules` so a bad row yields
/// a `recurring_rule[i]` error instead of a raw SQLite constraint failure.
fn validate_recurring_rule_row(
    index: u32,
    row: &serde_json::Value,
    errors: &mut Vec<ImportRowError>,
) {
    const ENTITY: &str = "recurring_rule";
    if let Some(type_) = required_str(row, "type", index, ENTITY, errors) {
        if let Err(err) = TxType::parse(type_) {
            push_error(errors, index, ENTITY, domain_message(err));
        }
    }
    if let Some(amount) = required_i64(row, "amount", index, ENTITY, errors) {
        if amount <= 0 {
            push_error(
                errors,
                index,
                ENTITY,
                format!("amount must be positive, got {amount}"),
            );
        }
    }
    let _ = required_i64(row, "account_id", index, ENTITY, errors);
    if let Some(frequency) = required_str(row, "frequency", index, ENTITY, errors) {
        if !matches!(frequency, "monthly" | "weekly" | "yearly") {
            push_error(
                errors,
                index,
                ENTITY,
                format!("frequency must be monthly|weekly|yearly, got '{frequency}'"),
            );
        }
    }
    if let Some(day) = value_i64(row, "day_of_month") {
        if !(1..=31).contains(&day) {
            push_error(
                errors,
                index,
                ENTITY,
                format!("day_of_month must be 1..=31, got {day}"),
            );
        }
    }
    if let Some(day) = value_i64(row, "day_of_week") {
        if !(0..=6).contains(&day) {
            push_error(
                errors,
                index,
                ENTITY,
                format!("day_of_week must be 0..=6, got {day}"),
            );
        }
    }
    if let Some(active) = value_i64(row, "active") {
        if !matches!(active, 0 | 1) {
            push_error(
                errors,
                index,
                ENTITY,
                format!("active must be 0 or 1, got {active}"),
            );
        }
    }
    let starts_on =
        required_str(row, "starts_on", index, ENTITY, errors).and_then(|raw| match parse_iso_date(
            "starts_on",
            raw,
        ) {
            Ok(date) => Some((raw, date)),
            Err(err) => {
                push_error(errors, index, ENTITY, domain_message(err));
                None
            }
        });
    if let Some(raw_ends_on) = value_str(row, "ends_on") {
        match parse_iso_date("ends_on", raw_ends_on) {
            Ok(ends_on) => {
                if let Some((raw_starts_on, starts_on)) = starts_on {
                    if ends_on < starts_on {
                        push_error(
                            errors,
                            index,
                            ENTITY,
                            format!(
                                "ends_on '{raw_ends_on}' is before starts_on '{raw_starts_on}'"
                            ),
                        );
                    }
                }
            }
            Err(err) => push_error(errors, index, ENTITY, domain_message(err)),
        }
    }
    if let Some(last_generated_on) = value_str(row, "last_generated_on") {
        if let Err(err) = parse_iso_date("last_generated_on", last_generated_on) {
            push_error(errors, index, ENTITY, domain_message(err));
        }
    }
}

fn collect_import_errors(snap: &Snapshot) -> Vec<ImportRowError> {
    let mut errors = Vec::new();
    let categories = snapshot_categories(snap);
    for (index, row) in snap.accounts.iter().enumerate() {
        validate_account_row(index as u32, row, &mut errors);
    }
    for (index, row) in snap.categories.iter().enumerate() {
        validate_category_row(index as u32, row, &mut errors);
    }
    for (index, row) in snap.recurring_rules.iter().enumerate() {
        validate_recurring_rule_row(index as u32, row, &mut errors);
    }
    for (index, row) in snap.transactions.iter().enumerate() {
        validate_transaction_row(index as u32, row, &categories, &mut errors);
    }
    for (index, row) in snap.budgets.iter().enumerate() {
        validate_budget_row(index as u32, row, &categories, &mut errors);
    }
    errors
}

/// Parse and validate a backup payload without touching the database.
fn parse_snapshot(payload: &str, mode: &str) -> AppResult<(Snapshot, ImportMode)> {
    if payload.len() > MAX_PAYLOAD_BYTES {
        return Err(AppError::InvalidArgument(format!(
            "payload exceeds {MAX_PAYLOAD_BYTES} bytes"
        )));
    }
    let snap: Snapshot = serde_json::from_str(payload)
        .map_err(|e| AppError::InvalidArgument(format!("invalid backup JSON: {e}")))?;
    if snap.schema_version != BACKUP_FORMAT_VERSION {
        return Err(AppError::InvalidArgument(format!(
            "unsupported backup format version {} (expected {BACKUP_FORMAT_VERSION}); this is not app_meta.schema_version",
            snap.schema_version
        )));
    }
    let mode = ImportMode::parse(mode)?;

    let errors = collect_import_errors(&snap);
    if !errors.is_empty() {
        return Err(AppError::ImportValidation(ImportRowError::join_all(
            &errors,
        )));
    }
    Ok((snap, mode))
}

pub fn import_snapshot_json(
    conn: &mut rusqlite::Connection,
    payload: &str,
    mode: &str,
) -> AppResult<ImportResult> {
    let (snap, mode) = parse_snapshot(payload, mode)?;
    apply_snapshot(conn, &snap, mode)
}

/// Same as [`import_snapshot_json`], but an overwrite first copies the live
/// database to `data.db.pre-import-<stamp>` so a wrong or stale file can be
/// undone with `restore_pre_import_snapshot`. Validation runs before the copy
/// so a rejected payload costs nothing; a failure inside the import
/// transaction rolls back and discards the copy, since the live data is
/// intact. Older copies are pruned only after the import committed.
pub fn import_snapshot_json_guarded(
    conn: &mut rusqlite::Connection,
    db_path: &Path,
    payload: &str,
    mode: &str,
) -> AppResult<ImportResult> {
    let (snap, mode) = parse_snapshot(payload, mode)?;
    if mode != ImportMode::Overwrite {
        return apply_snapshot(conn, &snap, mode);
    }
    let data_dir = snapshots::data_dir_of(db_path)?;
    let copy = snapshots::write_pre_import_snapshot(conn, &data_dir, chrono::Utc::now())?;
    match apply_snapshot(conn, &snap, mode) {
        Ok(result) => {
            // The import is committed; a pruning hiccup must not report it as failed.
            if let Err(err) =
                snapshots::prune_pre_import_snapshots(&data_dir, snapshots::KEEP_NEWEST)
            {
                eprintln!("[backup] failed to prune pre-import snapshots: {err}");
            }
            Ok(result)
        }
        Err(err) => {
            let _ = std::fs::remove_file(&copy);
            Err(err)
        }
    }
}

/// Apply a validated snapshot inside one immediate transaction.
fn apply_snapshot(
    conn: &mut rusqlite::Connection,
    snap: &Snapshot,
    mode: ImportMode,
) -> AppResult<ImportResult> {
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let mut warnings = Vec::new();
    let mut category_count = 0u32;
    let mut account_count = 0u32;
    let mut transaction_count = 0u32;
    let mut budget_count = 0u32;
    let mut account_map = HashMap::new();
    let mut category_map = HashMap::new();
    let mut recurring_map = HashMap::new();

    if mode == ImportMode::Overwrite {
        for sql in [
            "DELETE FROM transactions",
            "DELETE FROM budgets",
            "DELETE FROM recurring_rules",
            "DELETE FROM categories",
            "DELETE FROM accounts",
        ] {
            tx.execute(sql, [])?;
        }
        tx.execute(
            "DELETE FROM sqlite_sequence
              WHERE name IN ('categories','accounts','recurring_rules','transactions','budgets')",
            [],
        )?;
    }

    for account in &snap.accounts {
        let old_id = require_i64(account, "id", "account")?;
        let new_id = insert_account(&tx, account, mode)?;
        account_map.insert(old_id, new_id);
        account_count += 1;
    }

    for category in &snap.categories {
        let old_id = require_i64(category, "id", "category")?;
        if let Some((new_id, inserted)) = insert_category(&tx, category, mode, &mut warnings)? {
            category_map.insert(old_id, new_id);
            if inserted {
                category_count += 1;
            }
        }
    }

    for rule in &snap.recurring_rules {
        let old_id = require_i64(rule, "id", "recurring rule")?;
        if let Some(new_id) =
            insert_recurring_rule(&tx, rule, mode, &account_map, &category_map, &mut warnings)?
        {
            recurring_map.insert(old_id, new_id);
        }
    }

    for transaction in &snap.transactions {
        if insert_transaction(
            &tx,
            transaction,
            mode,
            &account_map,
            &category_map,
            &recurring_map,
            &mut warnings,
        )? {
            transaction_count += 1;
        }
    }

    for budget in &snap.budgets {
        if insert_budget(&tx, budget, mode, &category_map, &mut warnings)? {
            budget_count += 1;
        }
    }

    if mode == ImportMode::Overwrite {
        for meta in &snap.app_meta {
            let key = value_str(meta, "key").unwrap_or("");
            if key.is_empty() || key == "schema_version" || key == "last_backup_at" {
                continue;
            }
            let value = value_str(meta, "value").unwrap_or("");
            meta_repo::set(&tx, key, value)?;
        }
    }

    tx.commit()?;

    Ok(ImportResult {
        categories: category_count,
        accounts: account_count,
        transactions: transaction_count,
        budgets: budget_count,
        warnings,
    })
}

#[tauri::command]
pub fn import_json(
    app: AppHandle,
    state: State<'_, AppState>,
    args: ImportArgs,
) -> AppResult<ImportResult> {
    // data.db never moves while the app runs, so reading its path under a
    // separate lock is safe; the safety copy and the import then run under
    // one lock so no other command can slip in between them.
    let db_path = state.db_path()?;
    let result = state.with_conn_mut(|conn| {
        import_snapshot_json_guarded(conn, &db_path, &args.payload, &args.mode)
    })?;

    emit_changed(&app, ChangedDomain::Categories);
    emit_changed(&app, ChangedDomain::Accounts);
    emit_changed(&app, ChangedDomain::Transactions);
    emit_changed(&app, ChangedDomain::Budgets);
    emit_changed(&app, ChangedDomain::Meta);

    Ok(result)
}
