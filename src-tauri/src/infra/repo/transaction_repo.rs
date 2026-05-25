use rusqlite::{params, Connection, OptionalExtension, ToSql};

use crate::domain::ledger::{Transaction, TxType};
use crate::error::{AppError, AppResult};

#[derive(Debug, Default, Clone)]
pub struct ListFilter {
    pub from: Option<String>,
    pub to: Option<String>,
    pub type_: Option<TxType>,
    pub category_id: Option<i64>,
    pub account_id: Option<i64>,
    pub search: Option<String>,
}

fn row_to_tx(row: &rusqlite::Row<'_>) -> rusqlite::Result<Transaction> {
    let type_raw: String = row.get("type")?;
    let type_ = match type_raw.as_str() {
        "income" => TxType::Income,
        "expense" => TxType::Expense,
        "transfer" => TxType::Transfer,
        other => {
            return Err(rusqlite::Error::FromSqlConversionFailure(
                0,
                rusqlite::types::Type::Text,
                format!("unknown tx type '{other}'").into(),
            ));
        }
    };
    Ok(Transaction {
        id: row.get("id")?,
        occurred_on: row.get("occurred_on")?,
        type_,
        amount: row.get("amount")?,
        account_id: row.get("account_id")?,
        counter_account_id: row.get("counter_account_id")?,
        category_id: row.get("category_id")?,
        description: row.get("description")?,
        recurring_id: row.get("recurring_id")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

fn build_where(filter: &ListFilter) -> (String, Vec<Box<dyn ToSql>>) {
    let mut clauses: Vec<String> = vec!["type IN ('income','expense')".into()];
    let mut binds: Vec<Box<dyn ToSql>> = Vec::new();
    if let Some(from) = &filter.from {
        clauses.push("occurred_on >= ?".into());
        binds.push(Box::new(from.clone()));
    }
    if let Some(to) = &filter.to {
        clauses.push("occurred_on <= ?".into());
        binds.push(Box::new(to.clone()));
    }
    if let Some(t) = filter.type_ {
        clauses.push("type = ?".into());
        binds.push(Box::new(t.as_sql().to_string()));
    }
    if let Some(cat) = filter.category_id {
        clauses.push("category_id = ?".into());
        binds.push(Box::new(cat));
    }
    if let Some(acc) = filter.account_id {
        clauses.push("account_id = ?".into());
        binds.push(Box::new(acc));
    }
    if let Some(q) = &filter.search {
        if !q.is_empty() {
            clauses.push("description LIKE ?".into());
            binds.push(Box::new(format!("%{q}%")));
        }
    }
    (format!(" WHERE {}", clauses.join(" AND ")), binds)
}

pub fn list(
    conn: &Connection,
    filter: &ListFilter,
    page: u32,
    page_size: u32,
) -> AppResult<(Vec<Transaction>, u32)> {
    let (where_sql, binds) = build_where(filter);

    let total: u32 = {
        let count_sql = format!("SELECT COUNT(*) FROM transactions{where_sql}");
        let mut stmt = conn.prepare(&count_sql)?;
        let params_refs: Vec<&dyn ToSql> = binds.iter().map(|b| b.as_ref()).collect();
        stmt.query_row(params_refs.as_slice(), |r| r.get::<_, i64>(0))? as u32
    };

    let limit = page_size.max(1);
    let offset = page.saturating_mul(limit);
    let list_sql = format!(
        "SELECT id, occurred_on, type, amount, account_id, counter_account_id,
                category_id, description, recurring_id, created_at, updated_at
           FROM transactions{where_sql}
          ORDER BY occurred_on DESC, id DESC
          LIMIT {limit} OFFSET {offset}",
    );
    let mut stmt = conn.prepare(&list_sql)?;
    let params_refs: Vec<&dyn ToSql> = binds.iter().map(|b| b.as_ref()).collect();
    let items: Vec<Transaction> = stmt
        .query_map(params_refs.as_slice(), row_to_tx)?
        .collect::<rusqlite::Result<_>>()?;
    Ok((items, total))
}

pub fn find_by_id(conn: &Connection, id: i64) -> AppResult<Transaction> {
    conn.query_row(
        "SELECT id, occurred_on, type, amount, account_id, counter_account_id,
                category_id, description, recurring_id, created_at, updated_at
           FROM transactions WHERE id = ?1",
        params![id],
        row_to_tx,
    )
    .optional()?
    .ok_or_else(|| AppError::NotFound(format!("transaction {id}")))
}

pub struct InsertInput<'a> {
    pub occurred_on: &'a str,
    pub type_: TxType,
    pub amount: i64,
    pub account_id: i64,
    pub category_id: i64,
    pub description: &'a str,
    pub now: &'a str,
}

pub fn insert(conn: &Connection, input: &InsertInput<'_>) -> AppResult<i64> {
    conn.execute(
        "INSERT INTO transactions(occurred_on, type, amount, account_id, category_id,
                                  description, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)",
        params![
            input.occurred_on,
            input.type_.as_sql(),
            input.amount,
            input.account_id,
            input.category_id,
            input.description,
            input.now,
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

pub struct UpdateInput<'a> {
    pub occurred_on: &'a str,
    pub type_: TxType,
    pub amount: i64,
    pub account_id: i64,
    pub category_id: i64,
    pub description: &'a str,
    pub now: &'a str,
}

pub fn update(conn: &Connection, id: i64, input: &UpdateInput<'_>) -> AppResult<()> {
    let n = conn.execute(
        "UPDATE transactions
            SET occurred_on = ?1, type = ?2, amount = ?3, account_id = ?4,
                counter_account_id = NULL, category_id = ?5, description = ?6, updated_at = ?7
          WHERE id = ?8",
        params![
            input.occurred_on,
            input.type_.as_sql(),
            input.amount,
            input.account_id,
            input.category_id,
            input.description,
            input.now,
            id,
        ],
    )?;
    if n == 0 {
        return Err(AppError::NotFound(format!("transaction {id}")));
    }
    Ok(())
}

pub fn delete(conn: &Connection, id: i64) -> AppResult<()> {
    let n = conn.execute("DELETE FROM transactions WHERE id = ?1", params![id])?;
    if n == 0 {
        return Err(AppError::NotFound(format!("transaction {id}")));
    }
    Ok(())
}
