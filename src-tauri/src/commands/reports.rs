use rusqlite::Connection;
use serde::Serialize;
use tauri::State;

use crate::commands::meta::AppState;
use crate::domain::report::{
    self, CategoryAggregate, CategorySeries, Delta, MonthlyBucket, PeriodTotals,
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
    validate_month(month)?;
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

/// `monthly_summary` / `report_monthly` が共有する月の妥当性チェック。
fn validate_month(month: u32) -> AppResult<()> {
    if !(1..=12).contains(&month) {
        return Err(AppError::InvalidArgument(format!(
            "month out of range: {month}"
        )));
    }
    Ok(())
}

/// レンジの上限。既存 `monthly_series` の上限に揃える。
pub const MAX_RANGE_MONTHS: u32 = 60;
/// 移動平均の窓（spec §5.6）。
pub const MOVING_AVERAGE_WINDOW: usize = 3;

/// `"YYYY-MM"` の閉区間を月キーの並びに開く。書式・順序・長さをここで弾く。
pub fn range_months(from_year_month: &str, to_year_month: &str) -> AppResult<Vec<String>> {
    let start = YearMonth::parse_key(from_year_month)?;
    let end = YearMonth::parse_key(to_year_month)?;
    let span = end.months_since(start) + 1;
    if span < 1 {
        return Err(AppError::InvalidArgument(format!(
            "range must not run backwards: {from_year_month}..{to_year_month}"
        )));
    }
    if span > i64::from(MAX_RANGE_MONTHS) {
        return Err(AppError::InvalidArgument(format!(
            "range must be at most {MAX_RANGE_MONTHS} months, got {span}"
        )));
    }

    let span = span as u32;
    Ok((0..span)
        .map(|offset| end.step_back(span - 1 - offset).key())
        .collect())
}

#[tauri::command]
pub fn report_monthly(
    state: State<'_, AppState>,
    year: i32,
    month: u32,
) -> AppResult<MonthlyReport> {
    validate_year(year)?;
    validate_month(month)?;
    state.with_conn(|conn| build_monthly_report(conn, year, month))
}

#[tauri::command]
pub fn report_yearly(state: State<'_, AppState>, year: i32) -> AppResult<YearlyReport> {
    validate_year(year)?;
    state.with_conn(|conn| build_yearly_report(conn, year))
}

#[derive(Debug, Serialize)]
pub struct CategoryReport {
    /// 軸ラベル。`series[*].points` はこの並びと同じ長さ。
    pub months: Vec<String>,
    pub income: Vec<CategoryAggregate>,
    pub expense: Vec<CategoryAggregate>,
    pub series: Vec<CategorySeries>,
}

/// 期間内に取引のある全カテゴリを一度に返す。UI はクリックで表示を絞るだけで、
/// 切り替えのたびに呼び直さない（spec §5.6）。
pub fn build_category_report(
    conn: &Connection,
    from_year_month: &str,
    to_year_month: &str,
) -> AppResult<CategoryReport> {
    let months = range_months(from_year_month, to_year_month)?;
    let totals = report_repo::category_totals_between(conn, from_year_month, to_year_month)?;
    let rows = report_repo::category_month_amounts(conn, from_year_month, to_year_month)?;

    Ok(CategoryReport {
        income: totals
            .iter()
            .filter(|a| a.type_ == "income")
            .cloned()
            .collect(),
        expense: totals
            .iter()
            .filter(|a| a.type_ == "expense")
            .cloned()
            .collect(),
        series: report::pivot_category_series(&rows, &months),
        months,
    })
}

#[derive(Debug, Serialize)]
pub struct NetWorthPoint {
    pub year_month: String,
    /// 月末時点の純資産（振替の両脚を含む、非アーカイブ口座のみ）。
    pub net_worth: i64,
    /// その月の収支（振替は除外、規約3）。
    pub net: i64,
    /// `net` の3ヶ月移動平均。窓が埋まらない先頭2点は `None`。
    pub net_moving_avg: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct NetWorthReport {
    pub points: Vec<NetWorthPoint>,
}

pub fn build_net_worth_report(
    conn: &Connection,
    from_year_month: &str,
    to_year_month: &str,
) -> AppResult<NetWorthReport> {
    let months = range_months(from_year_month, to_year_month)?;
    let opening = report_repo::opening_net_worth(conn, from_year_month)?;
    let deltas = report_repo::monthly_net_worth_delta(conn, from_year_month, to_year_month)?;
    let net_worth = report::accumulate_net_worth(opening, &report::align_to_months(&deltas, &months));

    let buckets = report_repo::monthly_buckets_between(conn, from_year_month, to_year_month)?;
    let nets: Vec<i64> = report::align_to_months(
        &buckets
            .iter()
            .map(|b| (b.year_month.clone(), b.income.saturating_sub(b.expense)))
            .collect::<Vec<_>>(),
        &months,
    );
    let averages = report::moving_average(&nets, MOVING_AVERAGE_WINDOW);

    let points = months
        .into_iter()
        .enumerate()
        .map(|(i, year_month)| NetWorthPoint {
            year_month,
            net_worth: net_worth[i],
            net: nets[i],
            net_moving_avg: averages[i],
        })
        .collect();

    Ok(NetWorthReport { points })
}

#[tauri::command]
pub fn report_by_category(
    state: State<'_, AppState>,
    from_year_month: String,
    to_year_month: String,
) -> AppResult<CategoryReport> {
    // 書式と長さは range_months が弾く。DB を開く前に検証しておく。
    range_months(&from_year_month, &to_year_month)?;
    state.with_conn(|conn| build_category_report(conn, &from_year_month, &to_year_month))
}

#[tauri::command]
pub fn report_net_worth_series(
    state: State<'_, AppState>,
    from_year_month: String,
    to_year_month: String,
) -> AppResult<NetWorthReport> {
    range_months(&from_year_month, &to_year_month)?;
    state.with_conn(|conn| build_net_worth_report(conn, &from_year_month, &to_year_month))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_year_rejects_out_of_range() {
        let err = validate_year(999).unwrap_err();
        assert!(matches!(err, AppError::InvalidArgument(_)));

        let err = validate_year(10_000).unwrap_err();
        assert!(matches!(err, AppError::InvalidArgument(_)));
    }

    #[test]
    fn validate_year_accepts_in_range() {
        assert!(validate_year(2026).is_ok());
    }

    #[test]
    fn validate_month_rejects_out_of_range() {
        let err = validate_month(0).unwrap_err();
        assert!(matches!(err, AppError::InvalidArgument(_)));

        let err = validate_month(13).unwrap_err();
        assert!(matches!(err, AppError::InvalidArgument(_)));
    }

    #[test]
    fn validate_month_accepts_in_range() {
        assert!(validate_month(1).is_ok());
        assert!(validate_month(12).is_ok());
    }
}
