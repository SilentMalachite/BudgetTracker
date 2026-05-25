use rusqlite::{params, Connection, OptionalExtension};

use crate::domain::account::{Account, AccountKind};
use crate::error::{AppError, AppResult};

fn row_to_account(row: &rusqlite::Row<'_>) -> rusqlite::Result<Account> {
    let kind_raw: String = row.get("kind")?;
    let kind = match kind_raw.as_str() {
        "cash" => AccountKind::Cash,
        "bank" => AccountKind::Bank,
        "credit_card" => AccountKind::CreditCard,
        "e_money" => AccountKind::EMoney,
        "investment" => AccountKind::Investment,
        other => {
            return Err(rusqlite::Error::FromSqlConversionFailure(
                0,
                rusqlite::types::Type::Text,
                format!("unknown account kind '{other}'").into(),
            ));
        }
    };
    Ok(Account {
        id: row.get("id")?,
        name: row.get("name")?,
        kind,
        currency: row.get("currency")?,
        initial_balance: row.get("initial_balance")?,
        display_order: row.get("display_order")?,
        note: row.get("note")?,
        archived_at: row.get("archived_at")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

pub fn list(conn: &Connection, include_archived: bool) -> AppResult<Vec<Account>> {
    let sql = if include_archived {
        "SELECT id, name, kind, currency, initial_balance, display_order, note,
                archived_at, created_at, updated_at
           FROM accounts
          ORDER BY display_order ASC, id ASC"
    } else {
        "SELECT id, name, kind, currency, initial_balance, display_order, note,
                archived_at, created_at, updated_at
           FROM accounts
          WHERE archived_at IS NULL
          ORDER BY display_order ASC, id ASC"
    };
    let mut stmt = conn.prepare(sql)?;
    let items = stmt
        .query_map([], row_to_account)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(items)
}

pub fn find_by_id(conn: &Connection, id: i64) -> AppResult<Account> {
    conn.query_row(
        "SELECT id, name, kind, currency, initial_balance, display_order, note,
                archived_at, created_at, updated_at
           FROM accounts WHERE id = ?1",
        params![id],
        row_to_account,
    )
    .optional()?
    .ok_or_else(|| AppError::NotFound(format!("account {id}")))
}

pub fn next_display_order(conn: &Connection) -> AppResult<i64> {
    let max: Option<i64> =
        conn.query_row("SELECT MAX(display_order) FROM accounts", [], |r| r.get(0))?;
    Ok(max.unwrap_or(-1) + 1)
}

pub struct InsertInput<'a> {
    pub name: &'a str,
    pub kind: AccountKind,
    pub currency: &'a str,
    pub initial_balance: i64,
    pub display_order: i64,
    pub note: &'a str,
    pub now: &'a str,
}

pub fn insert(conn: &Connection, input: &InsertInput<'_>) -> AppResult<i64> {
    conn.execute(
        "INSERT INTO accounts(name, kind, currency, initial_balance, display_order, note,
                              created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)",
        params![
            input.name,
            input.kind.as_sql(),
            input.currency,
            input.initial_balance,
            input.display_order,
            input.note,
            input.now,
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

#[derive(Debug, Default)]
pub struct UpdatePatch<'a> {
    pub name: Option<&'a str>,
    pub kind: Option<AccountKind>,
    pub initial_balance: Option<i64>,
    pub note: Option<&'a str>,
    pub display_order: Option<i64>,
}

pub fn update(conn: &Connection, id: i64, patch: &UpdatePatch<'_>, now: &str) -> AppResult<()> {
    if let Some(name) = patch.name {
        conn.execute(
            "UPDATE accounts SET name = ?1, updated_at = ?2 WHERE id = ?3",
            params![name, now, id],
        )?;
    }
    if let Some(kind) = patch.kind {
        conn.execute(
            "UPDATE accounts SET kind = ?1, updated_at = ?2 WHERE id = ?3",
            params![kind.as_sql(), now, id],
        )?;
    }
    if let Some(balance) = patch.initial_balance {
        conn.execute(
            "UPDATE accounts SET initial_balance = ?1, updated_at = ?2 WHERE id = ?3",
            params![balance, now, id],
        )?;
    }
    if let Some(note) = patch.note {
        conn.execute(
            "UPDATE accounts SET note = ?1, updated_at = ?2 WHERE id = ?3",
            params![note, now, id],
        )?;
    }
    if let Some(order) = patch.display_order {
        conn.execute(
            "UPDATE accounts SET display_order = ?1, updated_at = ?2 WHERE id = ?3",
            params![order, now, id],
        )?;
    }
    Ok(())
}

pub fn set_archived(
    conn: &Connection,
    id: i64,
    archived_at: Option<&str>,
    now: &str,
) -> AppResult<()> {
    let n = conn.execute(
        "UPDATE accounts SET archived_at = ?1, updated_at = ?2 WHERE id = ?3",
        params![archived_at, now, id],
    )?;
    if n == 0 {
        return Err(AppError::NotFound(format!("account {id}")));
    }
    Ok(())
}
