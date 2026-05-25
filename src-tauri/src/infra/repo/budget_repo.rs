use chrono::NaiveDate;
use rusqlite::{params, Connection, OptionalExtension};

use crate::domain::budget::{
    self, Budget, BudgetStatus, BudgetStatusInput, ValidatedSetBudgetInput, YearMonth,
};
use crate::error::{AppError, AppResult};

fn row_to_budget(row: &rusqlite::Row<'_>) -> rusqlite::Result<Budget> {
    Ok(Budget {
        id: row.get("id")?,
        category_id: row.get("category_id")?,
        period: row.get("period")?,
        amount: row.get("amount")?,
        starts_on: row.get("starts_on")?,
        ends_on: row.get("ends_on")?,
        alert_threshold: row.get("alert_threshold")?,
    })
}

pub fn set_monthly_budget(
    conn: &Connection,
    input: &ValidatedSetBudgetInput,
) -> AppResult<Budget> {
    let starts_on = format!("{}-01", input.year_month.key());
    conn.execute(
        "INSERT INTO budgets(category_id, period, amount, starts_on, ends_on, alert_threshold)
         VALUES (?1, 'monthly', ?2, ?3, NULL, ?4)
         ON CONFLICT(category_id, starts_on) DO UPDATE SET
           period = 'monthly',
           amount = excluded.amount,
           ends_on = NULL,
           alert_threshold = excluded.alert_threshold",
        params![
            input.category_id,
            input.amount,
            starts_on,
            input.alert_threshold
        ],
    )
    .map_err(|e| match e {
        rusqlite::Error::SqliteFailure(err, _)
            if err.code == rusqlite::ErrorCode::ConstraintViolation =>
        {
            AppError::InvalidArgument("budget violates database constraints".into())
        }
        other => AppError::Db(other),
    })?;

    find_by_category_starts_on(conn, input.category_id, &starts_on)
}

pub fn find_by_category_starts_on(
    conn: &Connection,
    category_id: i64,
    starts_on: &str,
) -> AppResult<Budget> {
    conn.query_row(
        "SELECT id, category_id, period, amount, starts_on, ends_on, alert_threshold
           FROM budgets
          WHERE category_id = ?1 AND starts_on = ?2",
        params![category_id, starts_on],
        row_to_budget,
    )
    .optional()?
    .ok_or_else(|| AppError::NotFound(format!("budget category={category_id} starts_on={starts_on}")))
}

pub fn list_statuses(
    conn: &Connection,
    year_month: YearMonth,
    today: NaiveDate,
) -> AppResult<Vec<BudgetStatus>> {
    let bounds = budget::month_bounds(year_month)?;
    let starts_on = bounds.starts_on.format("%Y-%m-%d").to_string();
    let next_starts_on = bounds.next_starts_on.format("%Y-%m-%d").to_string();
    let mut stmt = conn.prepare(
        "SELECT
            c.id AS category_id,
            c.name AS category_name,
            c.color AS category_color,
            c.icon AS category_icon,
            b.id AS budget_id,
            COALESCE(b.amount, 0) AS budgeted,
            COALESCE(b.alert_threshold, 80) AS alert_threshold,
            COALESCE(SUM(t.amount), 0) AS spent
           FROM categories c
           LEFT JOIN budgets b
             ON b.category_id = c.id
            AND b.period = 'monthly'
            AND b.starts_on = ?1
           LEFT JOIN transactions t
             ON t.category_id = c.id
            AND t.type = 'expense'
            AND t.occurred_on >= ?1
            AND t.occurred_on < ?2
          WHERE c.type = 'expense'
            AND c.archived_at IS NULL
          GROUP BY
            c.id, c.name, c.color, c.icon,
            b.id, b.amount, b.alert_threshold
          ORDER BY c.display_order ASC, c.id ASC",
    )?;
    let inputs = stmt
        .query_map(params![starts_on, next_starts_on], |row| {
            Ok(BudgetStatusInput {
                category_id: row.get("category_id")?,
                category_name: row.get("category_name")?,
                category_color: row.get("category_color")?,
                category_icon: row.get("category_icon")?,
                budget_id: row.get("budget_id")?,
                budgeted: row.get("budgeted")?,
                spent: row.get("spent")?,
                alert_threshold: row.get("alert_threshold")?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    inputs
        .into_iter()
        .map(|input| budget::evaluate_status(input, year_month, today))
        .collect()
}
