use tauri::State;

use crate::commands::meta::AppState;
use crate::domain::report::{self, MonthlyBucket};
use crate::error::{AppError, AppResult};
use crate::infra::repo::report_repo::{self, MonthlySummary};

#[tauri::command]
pub fn monthly_summary(
    state: State<'_, AppState>,
    year: i32,
    month: u32,
) -> AppResult<MonthlySummary> {
    if !(1..=12).contains(&month) {
        return Err(AppError::InvalidArgument(format!(
            "month out of range: {month}"
        )));
    }
    state.with_conn(|conn| report_repo::monthly_summary(conn, year, month))
}

#[tauri::command]
pub fn monthly_series(state: State<'_, AppState>, months: u32) -> AppResult<Vec<MonthlyBucket>> {
    if months == 0 || months > 60 {
        return Err(AppError::InvalidArgument(format!(
            "months must be between 1 and 60, got {months}"
        )));
    }

    let today = chrono::Local::now().date_naive();
    let end = report::year_month_from_date(today);
    let start = end.step_back(months - 1);
    let raw = state.with_conn(|conn| report_repo::monthly_buckets_since(conn, &start.key()))?;
    report::fill_monthly_series(&raw, end, months)
}
