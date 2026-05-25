use std::collections::HashMap;

use rusqlite::{params, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tauri::{AppHandle, State};

use crate::commands::meta::AppState;
use crate::error::{AppError, AppResult};
use crate::infra::events::{emit_changed, ChangedDomain};
use crate::infra::repo::meta_repo;

const SUPPORTED_SCHEMA_VERSION: u32 = 1;
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
        schema_version: SUPPORTED_SCHEMA_VERSION,
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
    let conn = state
        .conn
        .lock()
        .map_err(|_| AppError::Corrupt("connection mutex poisoned".into()))?;
    export_snapshot_json(&conn)
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

pub fn import_snapshot_json(
    conn: &mut rusqlite::Connection,
    payload: &str,
    mode: &str,
) -> AppResult<ImportResult> {
    if payload.len() > MAX_PAYLOAD_BYTES {
        return Err(AppError::InvalidArgument(format!(
            "payload exceeds {MAX_PAYLOAD_BYTES} bytes"
        )));
    }
    let snap: Snapshot = serde_json::from_str(payload)
        .map_err(|e| AppError::InvalidArgument(format!("invalid backup JSON: {e}")))?;
    if snap.schema_version != SUPPORTED_SCHEMA_VERSION {
        return Err(AppError::InvalidArgument(format!(
            "unsupported schema_version {}",
            snap.schema_version
        )));
    }
    let mode = ImportMode::parse(mode)?;

    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let mut warnings = Vec::new();
    let mut category_count = 0u32;
    let mut account_count = 0u32;
    let mut transaction_count = 0u32;
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
        let _inserted = insert_budget(&tx, budget, mode, &category_map, &mut warnings)?;
    }

    if mode == ImportMode::Overwrite {
        for meta in &snap.app_meta {
            let key = value_str(meta, "key").unwrap_or("");
            if key.is_empty() || key == "schema_version" {
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
        warnings,
    })
}

#[tauri::command]
pub fn import_json(
    app: AppHandle,
    state: State<'_, AppState>,
    args: ImportArgs,
) -> AppResult<ImportResult> {
    let mut conn = state
        .conn
        .lock()
        .map_err(|_| AppError::Corrupt("connection mutex poisoned".into()))?;
    let result = import_snapshot_json(&mut conn, &args.payload, &args.mode)?;
    drop(conn);

    emit_changed(&app, ChangedDomain::Categories);
    emit_changed(&app, ChangedDomain::Accounts);
    emit_changed(&app, ChangedDomain::Transactions);
    emit_changed(&app, ChangedDomain::Meta);

    Ok(result)
}
