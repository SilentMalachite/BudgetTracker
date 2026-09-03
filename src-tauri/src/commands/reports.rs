use rusqlite::Connection;
use serde::Serialize;
use tauri::State;

use crate::commands::meta::AppState;
use crate::domain::report::{
    self, CategoryAggregate, Delta, MonthlyBucket, PeriodTotals,
};
use crate::domain::year_month::year_month_from_date;
use crate::domain::YearMonth;
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
    let end = year_month_from_date(today);
    let start = end.step_back(months - 1);
    let raw = state.with_conn(|conn| report_repo::monthly_buckets_since(conn, &start.key()))?;
    report::fill_monthly_series(&raw, end, months)
}

/// 月次タブの Top5 は 5 件固定。
pub const TOP_CATEGORY_COUNT: usize = 5;

#[derive(Debug, Serialize)]
pub struct MonthlyReport {
    pub current: PeriodTotals,
    pub prev_month: PeriodTotals,
    pub prev_year: PeriodTotals,
    pub mom: Delta,
    pub yoy: Delta,
    pub top_expense: Vec<CategoryAggregate>,
    pub top_income: Vec<CategoryAggregate>,
}

fn totals_of(summary: &MonthlySummary) -> PeriodTotals {
    PeriodTotals::new(summary.income, summary.expense)
}

/// 月次タブの比較値と Top5。棒グラフの系列は既存 `monthly_series` が持つので
/// ここでは返さない（系列を二重に持たないため）。
pub fn build_monthly_report(conn: &Connection, year: i32, month: u32) -> AppResult<MonthlyReport> {
    let this = YearMonth { year, month };
    let last_month = this.step_back(1);
    let last_year = this.step_back(12);

    let current_summary = report_repo::monthly_summary(conn, year, month)?;
    let prev_month_summary =
        report_repo::monthly_summary(conn, last_month.year, last_month.month)?;
    let prev_year_summary = report_repo::monthly_summary(conn, last_year.year, last_year.month)?;

    let current = totals_of(&current_summary);
    let prev_month = totals_of(&prev_month_summary);
    let prev_year = totals_of(&prev_year_summary);

    Ok(MonthlyReport {
        current,
        prev_month,
        prev_year,
        mom: report::compare(current, prev_month),
        yoy: report::compare(current, prev_year),
        top_expense: report::top_n(&current_summary.by_category, "expense", TOP_CATEGORY_COUNT),
        top_income: report::top_n(&current_summary.by_category, "income", TOP_CATEGORY_COUNT),
    })
}

#[derive(Debug, Serialize)]
pub struct YearlyReport {
    pub year: i32,
    pub months: Vec<MonthlyBucket>,
    pub total_income: i64,
    pub total_expense: i64,
    pub net: i64,
    pub avg_income: i64,
    pub avg_expense: i64,
    pub max_expense_month: Option<String>,
}

/// 1月から12月までを必ず 12 件返す。平均は常に 12 で割る。
pub fn build_yearly_report(conn: &Connection, year: i32) -> AppResult<YearlyReport> {
    let raw = report_repo::monthly_buckets_between(
        conn,
        &YearMonth { year, month: 1 }.key(),
        &YearMonth { year, month: 12 }.key(),
    )?;
    let months = report::fill_monthly_series(&raw, YearMonth { year, month: 12 }, 12)?;
    let stats = report::yearly_stats(&months);

    Ok(YearlyReport {
        year,
        months,
        total_income: stats.total_income,
        total_expense: stats.total_expense,
        net: stats.net,
        avg_income: stats.avg_income,
        avg_expense: stats.avg_expense,
        max_expense_month: stats.max_expense_month,
    })
}

/// `report_yearly` / レンジの両方が使う年の妥当性チェック。
/// `YearMonth::key()` が `YYYY` の 4 桁を前提にしているため範囲を絞る。
fn validate_year(year: i32) -> AppResult<()> {
    if !(1000..=9999).contains(&year) {
        return Err(AppError::InvalidArgument(format!(
            "year out of range: {year}"
        )));
    }
    Ok(())
}

#[tauri::command]
pub fn report_monthly(
    state: State<'_, AppState>,
    year: i32,
    month: u32,
) -> AppResult<MonthlyReport> {
    validate_year(year)?;
    if !(1..=12).contains(&month) {
        return Err(AppError::InvalidArgument(format!(
            "month out of range: {month}"
        )));
    }
    state.with_conn(|conn| build_monthly_report(conn, year, month))
}

#[tauri::command]
pub fn report_yearly(state: State<'_, AppState>, year: i32) -> AppResult<YearlyReport> {
    validate_year(year)?;
    state.with_conn(|conn| build_yearly_report(conn, year))
}
