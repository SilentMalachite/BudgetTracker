use rusqlite::{params, Connection};
use serde::Serialize;

use crate::domain::report::{CategoryMonthAmount, MonthlyBucket};
/// `CategoryAggregate` の正本は `domain::report`。ここから使う呼び出し元
/// (`response_fixtures.rs` など) を壊さないよう再エクスポートする。
pub use crate::domain::report::CategoryAggregate;
use crate::error::{AppError, AppResult};

#[derive(Debug, Serialize)]
pub struct MonthlySummary {
    pub income: i64,
    pub expense: i64,
    pub net: i64,
    pub by_category: Vec<CategoryAggregate>,
}

pub fn monthly_summary(conn: &Connection, year: i32, month: u32) -> AppResult<MonthlySummary> {
    let from = chrono::NaiveDate::from_ymd_opt(year, month, 1)
        .ok_or_else(|| AppError::InvalidArgument(format!("month out of range: {month}")))?;
    let next_month = if month == 12 {
        chrono::NaiveDate::from_ymd_opt(year + 1, 1, 1)
    } else {
        chrono::NaiveDate::from_ymd_opt(year, month + 1, 1)
    }
    .ok_or_else(|| AppError::InvalidArgument(format!("month out of range: {month}")))?;
    let to = next_month
        .pred_opt()
        .ok_or_else(|| AppError::InvalidArgument(format!("month out of range: {month}")))?;

    let (income, expense): (i64, i64) = conn.query_row(
        "SELECT
            COALESCE(SUM(CASE WHEN type = 'income' THEN amount END), 0),
            COALESCE(SUM(CASE WHEN type = 'expense' THEN amount END), 0)
           FROM transactions
          WHERE occurred_on BETWEEN ?1 AND ?2
            AND type IN ('income','expense')",
        params![
            from.format("%Y-%m-%d").to_string(),
            to.format("%Y-%m-%d").to_string()
        ],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;

    let mut stmt = conn.prepare(
        "SELECT c.id, c.name, c.type, COALESCE(SUM(t.amount), 0) AS amount
           FROM categories c
           JOIN transactions t ON t.category_id = c.id
          WHERE t.occurred_on BETWEEN ?1 AND ?2
            AND t.type IN ('income','expense')
          GROUP BY c.id, c.name, c.type
          ORDER BY amount DESC, c.id ASC",
    )?;
    let by_category = stmt
        .query_map(
            params![
                from.format("%Y-%m-%d").to_string(),
                to.format("%Y-%m-%d").to_string()
            ],
            |row| {
                Ok(CategoryAggregate {
                    category_id: row.get(0)?,
                    name: row.get(1)?,
                    type_: row.get(2)?,
                    amount: row.get(3)?,
                })
            },
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    Ok(MonthlySummary {
        income,
        expense,
        net: income - expense,
        by_category,
    })
}

pub fn monthly_buckets_since(
    conn: &Connection,
    from_year_month: &str,
) -> AppResult<Vec<MonthlyBucket>> {
    let from = format!("{from_year_month}-01");
    let mut stmt = conn.prepare(
        "SELECT
            strftime('%Y-%m', occurred_on) AS year_month,
            COALESCE(SUM(CASE WHEN type = 'income' THEN amount END), 0) AS income,
            COALESCE(SUM(CASE WHEN type = 'expense' THEN amount END), 0) AS expense
           FROM transactions
          WHERE occurred_on >= ?1
            AND type IN ('income','expense')
          GROUP BY year_month
          ORDER BY year_month ASC",
    )?;
    let buckets = stmt
        .query_map(params![from], |row| {
            Ok(MonthlyBucket {
                year_month: row.get(0)?,
                income: row.get(1)?,
                expense: row.get(2)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    Ok(buckets)
}

/// `occurred_on` は `YYYY-MM-DD` 固定長なので、`YYYY-MM` に `-01` / `-31` を
/// 足した文字列比較で月境界を挟める。`idx_tx_occurred_on` がそのまま効く。
fn month_bounds(from_year_month: &str, to_year_month: &str) -> (String, String) {
    (format!("{from_year_month}-01"), format!("{to_year_month}-31"))
}

pub fn monthly_buckets_between(
    conn: &Connection,
    from_year_month: &str,
    to_year_month: &str,
) -> AppResult<Vec<MonthlyBucket>> {
    let (from, to) = month_bounds(from_year_month, to_year_month);
    let mut stmt = conn.prepare(
        "SELECT
            strftime('%Y-%m', occurred_on) AS year_month,
            COALESCE(SUM(CASE WHEN type = 'income' THEN amount END), 0) AS income,
            COALESCE(SUM(CASE WHEN type = 'expense' THEN amount END), 0) AS expense
           FROM transactions
          WHERE occurred_on BETWEEN ?1 AND ?2
            AND type IN ('income','expense')
          GROUP BY year_month
          ORDER BY year_month ASC",
    )?;
    let buckets = stmt
        .query_map(params![from, to], |row| {
            Ok(MonthlyBucket {
                year_month: row.get(0)?,
                income: row.get(1)?,
                expense: row.get(2)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    Ok(buckets)
}

/// 期間内のカテゴリ別合計。並びは `monthly_summary` の `by_category` と同じ
/// （金額の降順、同額なら id の昇順）。フロントはこの並びの先頭を取るだけでよい。
pub fn category_totals_between(
    conn: &Connection,
    from_year_month: &str,
    to_year_month: &str,
) -> AppResult<Vec<CategoryAggregate>> {
    let (from, to) = month_bounds(from_year_month, to_year_month);
    let mut stmt = conn.prepare(
        "SELECT c.id, c.name, c.type, COALESCE(SUM(t.amount), 0) AS amount
           FROM categories c
           JOIN transactions t ON t.category_id = c.id
          WHERE t.occurred_on BETWEEN ?1 AND ?2
            AND t.type IN ('income','expense')
          GROUP BY c.id, c.name, c.type
          ORDER BY amount DESC, c.id ASC",
    )?;
    let rows = stmt
        .query_map(params![from, to], |row| {
            Ok(CategoryAggregate {
                category_id: row.get(0)?,
                name: row.get(1)?,
                type_: row.get(2)?,
                amount: row.get(3)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    Ok(rows)
}

pub fn category_month_amounts(
    conn: &Connection,
    from_year_month: &str,
    to_year_month: &str,
) -> AppResult<Vec<CategoryMonthAmount>> {
    let (from, to) = month_bounds(from_year_month, to_year_month);
    let mut stmt = conn.prepare(
        "SELECT strftime('%Y-%m', t.occurred_on) AS year_month,
                c.id, c.name, c.type, COALESCE(SUM(t.amount), 0) AS amount
           FROM categories c
           JOIN transactions t ON t.category_id = c.id
          WHERE t.occurred_on BETWEEN ?1 AND ?2
            AND t.type IN ('income','expense')
          GROUP BY year_month, c.id, c.name, c.type
          ORDER BY year_month ASC, c.id ASC",
    )?;
    let rows = stmt
        .query_map(params![from, to], |row| {
            Ok(CategoryMonthAmount {
                year_month: row.get(0)?,
                category_id: row.get(1)?,
                name: row.get(2)?,
                type_: row.get(3)?,
                amount: row.get(4)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    Ok(rows)
}

/// 純資産を動かす1取引1行。`archived_at IS NULL` の口座だけを見る。
///
/// [`crate::infra::repo::balance_repo::LIST_BALANCES_SQL`] と同じ4方向
/// （income +、expense −、transfer 出 −、transfer 入 +）を数える。全口座を
/// 合算するなら振替は勝手に相殺されるが、アーカイブ口座を外すとその相殺が
/// 崩れるため、両脚を明示的に数える必要がある。`{filter}` は
/// `t.occurred_on` に対する日付条件に差し替えて使う。
const NET_WORTH_DELTA_ROWS: &str = "\
SELECT strftime('%Y-%m', t.occurred_on) AS year_month,
       CASE t.type WHEN 'income'   THEN  t.amount
                   WHEN 'expense'  THEN -t.amount
                   WHEN 'transfer' THEN -t.amount
       END AS delta
  FROM transactions t
  JOIN accounts a ON a.id = t.account_id
 WHERE a.archived_at IS NULL AND {filter}
 UNION ALL
SELECT strftime('%Y-%m', t.occurred_on) AS year_month,
       t.amount AS delta
  FROM transactions t
  JOIN accounts a ON a.id = t.counter_account_id
 WHERE t.type = 'transfer' AND a.archived_at IS NULL AND {filter}";

/// 月ごとの純資産の増減。取引の無い月は行そのものが返らない
/// （呼び出し側が [`crate::domain::report::align_to_months`] で 0 埋めする）。
pub fn monthly_net_worth_delta(
    conn: &Connection,
    from_year_month: &str,
    to_year_month: &str,
) -> AppResult<Vec<(String, i64)>> {
    let (from, to) = month_bounds(from_year_month, to_year_month);
    let rows = NET_WORTH_DELTA_ROWS.replace("{filter}", "t.occurred_on BETWEEN ?1 AND ?2");
    let sql = format!(
        "SELECT year_month, COALESCE(SUM(delta), 0) FROM ({rows})
          GROUP BY year_month ORDER BY year_month ASC"
    );
    let mut stmt = conn.prepare(&sql)?;
    let deltas = stmt
        .query_map(params![from, to], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    Ok(deltas)
}

/// `from_year_month` の初日より前の時点での純資産。
pub fn opening_net_worth(conn: &Connection, from_year_month: &str) -> AppResult<i64> {
    let from = format!("{from_year_month}-01");
    let rows = NET_WORTH_DELTA_ROWS.replace("{filter}", "t.occurred_on < ?1");
    let sql = format!(
        "SELECT (SELECT COALESCE(SUM(initial_balance), 0)
                   FROM accounts WHERE archived_at IS NULL)
              + COALESCE((SELECT SUM(delta) FROM ({rows})), 0)"
    );
    let opening: i64 = conn.query_row(&sql, params![from], |row| row.get(0))?;

    Ok(opening)
}
