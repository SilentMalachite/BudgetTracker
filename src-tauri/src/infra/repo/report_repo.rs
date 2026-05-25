use rusqlite::{params, Connection};
use serde::Serialize;

use crate::domain::report::MonthlyBucket;
use crate::error::{AppError, AppResult};

#[derive(Debug, Serialize)]
pub struct CategoryAggregate {
    pub category_id: i64,
    pub name: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub amount: i64,
}

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
