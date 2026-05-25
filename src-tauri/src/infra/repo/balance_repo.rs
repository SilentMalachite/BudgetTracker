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

/// SQL used by [`list_balances`]. Exposed so integration tests can run
/// `EXPLAIN QUERY PLAN` against the exact same statement.
///
/// Computes each account's current balance using two index-friendly
/// subqueries combined with `UNION ALL`. The first leg uses
/// `idx_tx_account`; the second uses `idx_tx_counter_account` (V002).
/// Self-transfer is impossible because `validate_transfer_input` rejects
/// `account_id == counter_account_id`.
pub const LIST_BALANCES_SQL: &str = "\
SELECT a.id,
       a.name,
       a.kind,
       a.initial_balance,
       a.archived_at,
       a.display_order,
       a.initial_balance + COALESCE((
           SELECT SUM(delta) FROM (
               SELECT CASE type
                          WHEN 'income'   THEN  amount
                          WHEN 'expense'  THEN -amount
                          WHEN 'transfer' THEN -amount
                      END AS delta
                 FROM transactions
                WHERE account_id = a.id
               UNION ALL
               SELECT amount AS delta
                 FROM transactions
                WHERE counter_account_id = a.id AND type = 'transfer'
           )
       ), 0) AS balance
  FROM accounts a
 ORDER BY a.display_order ASC, a.id ASC";

/// One row per account (including archived). Caller filters as needed.
///
/// Computes each account's current balance using two index-friendly
/// subqueries combined with `UNION ALL`. The first leg uses
/// `idx_tx_account`; the second uses `idx_tx_counter_account` (V002).
/// Self-transfer is impossible because `validate_transfer_input` rejects
/// `account_id == counter_account_id`.
pub fn list_balances(conn: &Connection) -> AppResult<Vec<AccountBalance>> {
    let mut stmt = conn.prepare(LIST_BALANCES_SQL)?;
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
