//! `recurring_rules` の SQL と、ルールから生成した取引の一括 INSERT。
//!
//! 規約どおり物理削除はしない。停止は `active = 0`。

use chrono::NaiveDate;
use rusqlite::{params, Connection, OptionalExtension};

use crate::domain::ledger::TxType;
use crate::domain::recurring::{Frequency, RecurringRule};
use crate::error::{AppError, AppResult};

const COLUMNS: &str = "id, name, type, amount, account_id, counter_account_id, category_id,
                       description, frequency, day_of_month, day_of_week, starts_on, ends_on,
                       last_generated_on, active";

fn conversion_error(what: &str, raw: &str) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        0,
        rusqlite::types::Type::Text,
        format!("unknown {what} '{raw}'").into(),
    )
}

fn small_uint(row: &rusqlite::Row<'_>, name: &str) -> rusqlite::Result<Option<u32>> {
    let raw: Option<i64> = row.get(name)?;
    Ok(raw.and_then(|v| u32::try_from(v).ok()))
}

fn row_to_rule(row: &rusqlite::Row<'_>) -> rusqlite::Result<RecurringRule> {
    let type_raw: String = row.get("type")?;
    let type_ = match type_raw.as_str() {
        "income" => TxType::Income,
        "expense" => TxType::Expense,
        "transfer" => TxType::Transfer,
        other => return Err(conversion_error("tx type", other)),
    };
    let frequency_raw: String = row.get("frequency")?;
    let frequency = match frequency_raw.as_str() {
        "monthly" => Frequency::Monthly,
        "weekly" => Frequency::Weekly,
        "yearly" => Frequency::Yearly,
        other => return Err(conversion_error("frequency", other)),
    };
    let active: i64 = row.get("active")?;

    Ok(RecurringRule {
        id: row.get("id")?,
        name: row.get("name")?,
        type_,
        amount: row.get("amount")?,
        account_id: row.get("account_id")?,
        counter_account_id: row.get("counter_account_id")?,
        category_id: row.get("category_id")?,
        description: row.get("description")?,
        frequency,
        day_of_month: small_uint(row, "day_of_month")?,
        day_of_week: small_uint(row, "day_of_week")?,
        starts_on: row.get("starts_on")?,
        ends_on: row.get("ends_on")?,
        last_generated_on: row.get("last_generated_on")?,
        active: active != 0,
    })
}

pub struct InsertInput<'a> {
    pub name: &'a str,
    pub type_: TxType,
    pub amount: i64,
    pub account_id: i64,
    pub counter_account_id: Option<i64>,
    pub category_id: Option<i64>,
    pub description: &'a str,
    pub frequency: Frequency,
    pub day_of_month: Option<u32>,
    pub day_of_week: Option<u32>,
    pub starts_on: &'a str,
    pub ends_on: Option<&'a str>,
}

pub struct UpdateInput<'a> {
    pub name: &'a str,
    pub type_: TxType,
    pub amount: i64,
    pub account_id: i64,
    pub counter_account_id: Option<i64>,
    pub category_id: Option<i64>,
    pub description: &'a str,
    pub frequency: Frequency,
    pub day_of_month: Option<u32>,
    pub day_of_week: Option<u32>,
    pub starts_on: &'a str,
    pub ends_on: Option<&'a str>,
}

fn constraint_error(e: rusqlite::Error) -> AppError {
    match e {
        rusqlite::Error::SqliteFailure(err, _)
            if err.code == rusqlite::ErrorCode::ConstraintViolation =>
        {
            AppError::InvalidArgument("recurring rule violates database constraints".into())
        }
        other => AppError::Db(other),
    }
}

pub fn insert(conn: &Connection, input: &InsertInput<'_>) -> AppResult<i64> {
    conn.execute(
        "INSERT INTO recurring_rules(name, type, amount, account_id, counter_account_id,
                                     category_id, description, frequency, day_of_month,
                                     day_of_week, starts_on, ends_on, last_generated_on, active)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, NULL, 1)",
        params![
            input.name,
            input.type_.as_sql(),
            input.amount,
            input.account_id,
            input.counter_account_id,
            input.category_id,
            input.description,
            input.frequency.as_sql(),
            input.day_of_month.map(i64::from),
            input.day_of_week.map(i64::from),
            input.starts_on,
            input.ends_on,
        ],
    )
    .map_err(constraint_error)?;
    Ok(conn.last_insert_rowid())
}

pub fn update(conn: &Connection, id: i64, input: &UpdateInput<'_>) -> AppResult<()> {
    let changed = conn
        .execute(
            "UPDATE recurring_rules
                SET name = ?2, type = ?3, amount = ?4, account_id = ?5,
                    counter_account_id = ?6, category_id = ?7, description = ?8,
                    frequency = ?9, day_of_month = ?10, day_of_week = ?11,
                    starts_on = ?12, ends_on = ?13
              WHERE id = ?1",
            params![
                id,
                input.name,
                input.type_.as_sql(),
                input.amount,
                input.account_id,
                input.counter_account_id,
                input.category_id,
                input.description,
                input.frequency.as_sql(),
                input.day_of_month.map(i64::from),
                input.day_of_week.map(i64::from),
                input.starts_on,
                input.ends_on,
            ],
        )
        .map_err(constraint_error)?;
    if changed == 0 {
        return Err(AppError::NotFound(format!("recurring rule {id}")));
    }
    Ok(())
}

/// 停止 / 再開。行は消さない (規約 5)。
///
/// 再開 (`active = true`) は `last_generated_on` を `today` まで進める。展開の窓は
/// `(last_generated_on, today]` なので、そうしないと停止中に見送った分が再開の瞬間に
/// まとめて生成され、締めた月の予算とレポートを後から書き換えてしまう。
///
/// ただし一度も生成していないルール (`last_generated_on IS NULL`) は NULL のまま
/// にする。保存前の preview が「今すぐ N 件生成されます」と約束した starts_on から
/// の backfill は、停止→再開をはさんでも残さなければならない。
/// watermark を巻き戻さないよう、`today` より先の値も動かさない。
///
/// 停止 (`active = false`) は watermark に一切触れない。
pub fn set_active(conn: &Connection, id: i64, active: bool, today: &str) -> AppResult<()> {
    let changed = if active {
        conn.execute(
            "UPDATE recurring_rules
                SET active = 1,
                    last_generated_on = CASE
                        WHEN last_generated_on IS NULL THEN NULL
                        WHEN last_generated_on > ?2 THEN last_generated_on
                        ELSE ?2
                    END
              WHERE id = ?1",
            params![id, today],
        )?
    } else {
        conn.execute(
            "UPDATE recurring_rules SET active = 0 WHERE id = ?1",
            params![id],
        )?
    };
    if changed == 0 {
        return Err(AppError::NotFound(format!("recurring rule {id}")));
    }
    Ok(())
}

pub fn set_last_generated_on(conn: &Connection, id: i64, date: &str) -> AppResult<()> {
    let changed = conn.execute(
        "UPDATE recurring_rules SET last_generated_on = ?2 WHERE id = ?1",
        params![id, date],
    )?;
    if changed == 0 {
        return Err(AppError::NotFound(format!("recurring rule {id}")));
    }
    Ok(())
}

pub fn find_by_id(conn: &Connection, id: i64) -> AppResult<RecurringRule> {
    let sql = format!("SELECT {COLUMNS} FROM recurring_rules WHERE id = ?1");
    conn.query_row(&sql, params![id], row_to_rule)
        .optional()?
        .ok_or_else(|| AppError::NotFound(format!("recurring rule {id}")))
}

pub fn list(conn: &Connection, include_inactive: bool) -> AppResult<Vec<RecurringRule>> {
    let filter = if include_inactive { "" } else { "WHERE active = 1" };
    let sql = format!("SELECT {COLUMNS} FROM recurring_rules {filter} ORDER BY id");
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], row_to_rule)?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

/// `dates` ぶんの取引を `rule` の内容で INSERT する。呼び出し側がトランザクションを張る。
pub fn insert_generated(
    conn: &Connection,
    rule: &RecurringRule,
    dates: &[NaiveDate],
    now: &str,
) -> AppResult<usize> {
    let mut stmt = conn.prepare(
        "INSERT INTO transactions(occurred_on, type, amount, account_id, counter_account_id,
                                  category_id, description, recurring_id, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9)",
    )?;
    for date in dates {
        stmt.execute(params![
            date.format("%Y-%m-%d").to_string(),
            rule.type_.as_sql(),
            rule.amount,
            rule.account_id,
            rule.counter_account_id,
            rule.category_id,
            rule.description,
            rule.id,
            now,
        ])
        .map_err(constraint_error)?;
    }
    Ok(dates.len())
}
