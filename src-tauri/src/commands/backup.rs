use rusqlite::{params, TransactionBehavior};
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
    transactions: Vec<serde_json::Value>,
    app_meta: Vec<serde_json::Value>,
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

fn exists(tx: &rusqlite::Transaction<'_>, table: &str, id: i64) -> AppResult<bool> {
    let sql = format!("SELECT COUNT(*) FROM {table} WHERE id = ?1");
    let count: i64 = tx.query_row(&sql, params![id], |row| row.get(0))?;
    Ok(count > 0)
}

fn handle_category_error(
    result: rusqlite::Result<usize>,
    warnings: &mut Vec<String>,
    name: &str,
) -> AppResult<bool> {
    match result {
        Ok(_) => Ok(true),
        Err(rusqlite::Error::SqliteFailure(err, _))
            if err.code == rusqlite::ErrorCode::ConstraintViolation =>
        {
            warnings.push(format!("skipped duplicate category: {name}"));
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
    if mode != "overwrite" && mode != "append" {
        return Err(AppError::InvalidArgument(format!("unknown mode '{mode}'")));
    }

    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let mut warnings = Vec::new();
    let mut category_count = 0u32;
    let mut account_count = 0u32;
    let mut transaction_count = 0u32;

    if mode == "overwrite" {
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
              WHERE name IN ('categories','accounts','transactions')",
            [],
        )?;
    }

    for category in &snap.categories {
        let name = category.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let result = if mode == "overwrite" {
            tx.execute(
                "INSERT INTO categories(id, name, type, color, icon, display_order, archived_at)
                 VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    category.get("id").and_then(|v| v.as_i64()),
                    name,
                    category
                        .get("type")
                        .and_then(|v| v.as_str())
                        .unwrap_or("expense"),
                    category.get("color").and_then(|v| v.as_str()),
                    category.get("icon").and_then(|v| v.as_str()),
                    category
                        .get("display_order")
                        .and_then(|v| v.as_i64())
                        .unwrap_or(0),
                    category.get("archived_at").and_then(|v| v.as_str()),
                ],
            )
        } else {
            tx.execute(
                "INSERT INTO categories(name, type, color, icon, display_order, archived_at)
                 VALUES(?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    name,
                    category
                        .get("type")
                        .and_then(|v| v.as_str())
                        .unwrap_or("expense"),
                    category.get("color").and_then(|v| v.as_str()),
                    category.get("icon").and_then(|v| v.as_str()),
                    category
                        .get("display_order")
                        .and_then(|v| v.as_i64())
                        .unwrap_or(0),
                    category.get("archived_at").and_then(|v| v.as_str()),
                ],
            )
        };
        if handle_category_error(result, &mut warnings, name)? {
            category_count += 1;
        }
    }

    for account in &snap.accounts {
        let result = if mode == "overwrite" {
            tx.execute(
                "INSERT INTO accounts(id, name, kind, currency, initial_balance, display_order,
                                      note, archived_at, created_at, updated_at)
                 VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    account.get("id").and_then(|v| v.as_i64()),
                    account.get("name").and_then(|v| v.as_str()).unwrap_or(""),
                    account
                        .get("kind")
                        .and_then(|v| v.as_str())
                        .unwrap_or("cash"),
                    account
                        .get("currency")
                        .and_then(|v| v.as_str())
                        .unwrap_or("JPY"),
                    account
                        .get("initial_balance")
                        .and_then(|v| v.as_i64())
                        .unwrap_or(0),
                    account
                        .get("display_order")
                        .and_then(|v| v.as_i64())
                        .unwrap_or(0),
                    account.get("note").and_then(|v| v.as_str()).unwrap_or(""),
                    account.get("archived_at").and_then(|v| v.as_str()),
                    account
                        .get("created_at")
                        .and_then(|v| v.as_str())
                        .unwrap_or("1970-01-01T00:00:00Z"),
                    account
                        .get("updated_at")
                        .and_then(|v| v.as_str())
                        .unwrap_or("1970-01-01T00:00:00Z"),
                ],
            )
        } else {
            tx.execute(
                "INSERT INTO accounts(name, kind, currency, initial_balance, display_order,
                                      note, archived_at, created_at, updated_at)
                 VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    account.get("name").and_then(|v| v.as_str()).unwrap_or(""),
                    account
                        .get("kind")
                        .and_then(|v| v.as_str())
                        .unwrap_or("cash"),
                    account
                        .get("currency")
                        .and_then(|v| v.as_str())
                        .unwrap_or("JPY"),
                    account
                        .get("initial_balance")
                        .and_then(|v| v.as_i64())
                        .unwrap_or(0),
                    account
                        .get("display_order")
                        .and_then(|v| v.as_i64())
                        .unwrap_or(0),
                    account.get("note").and_then(|v| v.as_str()).unwrap_or(""),
                    account.get("archived_at").and_then(|v| v.as_str()),
                    account
                        .get("created_at")
                        .and_then(|v| v.as_str())
                        .unwrap_or("1970-01-01T00:00:00Z"),
                    account
                        .get("updated_at")
                        .and_then(|v| v.as_str())
                        .unwrap_or("1970-01-01T00:00:00Z"),
                ],
            )
        };
        result.map_err(AppError::Db)?;
        account_count += 1;
    }

    for transaction in &snap.transactions {
        let account_id = transaction
            .get("account_id")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        let counter_account_id = transaction
            .get("counter_account_id")
            .and_then(|v| v.as_i64());
        let category_id = transaction.get("category_id").and_then(|v| v.as_i64());

        if mode == "append" {
            if !exists(&tx, "accounts", account_id)? {
                warnings.push(format!(
                    "skipped transaction referencing missing account {account_id}"
                ));
                continue;
            }
            if let Some(id) = counter_account_id {
                if !exists(&tx, "accounts", id)? {
                    warnings.push(format!(
                        "skipped transaction referencing missing counter account {id}"
                    ));
                    continue;
                }
            }
            if let Some(id) = category_id {
                if !exists(&tx, "categories", id)? {
                    warnings.push(format!(
                        "skipped transaction referencing missing category {id}"
                    ));
                    continue;
                }
            }
        }

        let result = if mode == "overwrite" {
            tx.execute(
                "INSERT INTO transactions(id, occurred_on, type, amount, account_id,
                                          counter_account_id, category_id, description,
                                          recurring_id, created_at, updated_at)
                 VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                params![
                    transaction.get("id").and_then(|v| v.as_i64()),
                    transaction
                        .get("occurred_on")
                        .and_then(|v| v.as_str())
                        .unwrap_or(""),
                    transaction
                        .get("type")
                        .and_then(|v| v.as_str())
                        .unwrap_or("expense"),
                    transaction
                        .get("amount")
                        .and_then(|v| v.as_i64())
                        .unwrap_or(0),
                    account_id,
                    counter_account_id,
                    category_id,
                    transaction
                        .get("description")
                        .and_then(|v| v.as_str())
                        .unwrap_or(""),
                    transaction.get("recurring_id").and_then(|v| v.as_i64()),
                    transaction
                        .get("created_at")
                        .and_then(|v| v.as_str())
                        .unwrap_or("1970-01-01T00:00:00Z"),
                    transaction
                        .get("updated_at")
                        .and_then(|v| v.as_str())
                        .unwrap_or("1970-01-01T00:00:00Z"),
                ],
            )
        } else {
            tx.execute(
                "INSERT INTO transactions(occurred_on, type, amount, account_id,
                                          counter_account_id, category_id, description,
                                          recurring_id, created_at, updated_at)
                 VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    transaction
                        .get("occurred_on")
                        .and_then(|v| v.as_str())
                        .unwrap_or(""),
                    transaction
                        .get("type")
                        .and_then(|v| v.as_str())
                        .unwrap_or("expense"),
                    transaction
                        .get("amount")
                        .and_then(|v| v.as_i64())
                        .unwrap_or(0),
                    account_id,
                    counter_account_id,
                    category_id,
                    transaction
                        .get("description")
                        .and_then(|v| v.as_str())
                        .unwrap_or(""),
                    transaction.get("recurring_id").and_then(|v| v.as_i64()),
                    transaction
                        .get("created_at")
                        .and_then(|v| v.as_str())
                        .unwrap_or("1970-01-01T00:00:00Z"),
                    transaction
                        .get("updated_at")
                        .and_then(|v| v.as_str())
                        .unwrap_or("1970-01-01T00:00:00Z"),
                ],
            )
        };
        result.map_err(AppError::Db)?;
        transaction_count += 1;
    }

    if mode == "overwrite" {
        for meta in &snap.app_meta {
            let key = meta.get("key").and_then(|v| v.as_str()).unwrap_or("");
            if key.is_empty() || key == "schema_version" {
                continue;
            }
            let value = meta.get("value").and_then(|v| v.as_str()).unwrap_or("");
            meta_repo::set(&tx, key, value)?;
        }
    }

    meta_repo::set(&tx, "last_backup_at", &chrono::Utc::now().to_rfc3339())?;
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
