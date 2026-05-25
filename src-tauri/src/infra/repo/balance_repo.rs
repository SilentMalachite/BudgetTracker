use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::domain::account::AccountKind;
use crate::error::AppResult;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountBalance {
    pub account_id: i64,
    pub name: String,
    pub kind: AccountKind,
    pub initial_balance: i64,
    pub balance: i64,
    pub archived_at: Option<String>,
    pub display_order: i64,
}

/// One row per account (including archived). Caller filters as needed.
///
/// SQL:
/// - LEFT JOIN matches a transaction on either `account_id = a.id` or `counter_account_id = a.id`.
/// - The CASE expression buckets the matched amount with the correct sign.
/// - Self-transfer is impossible (validator rejects it; SQL would not double-count anyway because
///   a row can match either side, not both, for a given `a.id` — V001 CHECK forbids the same id
///   in both columns indirectly: source != destination is a validator-level invariant).
pub fn list_balances(conn: &Connection) -> AppResult<Vec<AccountBalance>> {
    let mut stmt = conn.prepare(
        "SELECT a.id,
                a.name,
                a.kind,
                a.initial_balance,
                a.archived_at,
                a.display_order,
                a.initial_balance + COALESCE(SUM(
                    CASE
                        WHEN t.account_id = a.id AND t.type = 'income'   THEN  t.amount
                        WHEN t.account_id = a.id AND t.type = 'expense'  THEN -t.amount
                        WHEN t.account_id = a.id AND t.type = 'transfer' THEN -t.amount
                        WHEN t.counter_account_id = a.id AND t.type = 'transfer' THEN t.amount
                        ELSE 0
                    END
                ), 0) AS balance
           FROM accounts a
           LEFT JOIN transactions t
             ON t.account_id = a.id OR t.counter_account_id = a.id
          GROUP BY a.id
          ORDER BY a.display_order ASC, a.id ASC",
    )?;
    let rows = stmt
        .query_map([], |row| {
            let kind_raw: String = row.get(2)?;
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
            Ok(AccountBalance {
                account_id: row.get(0)?,
                name: row.get(1)?,
                kind,
                initial_balance: row.get(3)?,
                archived_at: row.get(4)?,
                display_order: row.get(5)?,
                balance: row.get(6)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}
