use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct YearMonth {
    pub year: i32,
    pub month: u32,
}

impl YearMonth {
    pub fn key(self) -> String {
        format!("{:04}-{:02}", self.year, self.month)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MonthBounds {
    pub starts_on: NaiveDate,
    pub next_starts_on: NaiveDate,
    pub days_in_month: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RawSetBudgetInput<'a> {
    pub category_id: i64,
    pub year_month: &'a str,
    pub amount: i64,
    pub alert_threshold: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ValidatedSetBudgetInput {
    pub category_id: i64,
    pub year_month: YearMonth,
    pub amount: i64,
    pub alert_threshold: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Budget {
    pub id: i64,
    pub category_id: i64,
    pub period: String,
    pub amount: i64,
    pub starts_on: String,
    pub ends_on: Option<String>,
    pub alert_threshold: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BudgetStatusInput {
    pub category_id: i64,
    pub category_name: String,
    pub category_color: Option<String>,
    pub category_icon: Option<String>,
    pub budget_id: Option<i64>,
    pub budgeted: i64,
    pub spent: i64,
    pub alert_threshold: i64,
}

#[cfg(test)]
impl BudgetStatusInput {
    fn minimal(category_id: i64, budgeted: i64, spent: i64) -> Self {
        Self {
            category_id,
            category_name: "食費".into(),
            category_color: None,
            category_icon: None,
            budget_id: Some(1),
            budgeted,
            spent,
            alert_threshold: 80,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BudgetStatus {
    pub category_id: i64,
    pub category_name: String,
    pub category_color: Option<String>,
    pub category_icon: Option<String>,
    pub budget_id: Option<i64>,
    pub budgeted: i64,
    pub spent: i64,
    pub percent: i64,
    pub progress_percent: i64,
    pub days_left: i64,
    pub projected: i64,
    pub alert_threshold: i64,
    pub threshold_reached: bool,
    pub projected_over_budget: bool,
}

pub fn parse_year_month(raw: &str) -> AppResult<YearMonth> {
    let Some((year_raw, month_raw)) = raw.split_once('-') else {
        return Err(AppError::InvalidArgument(format!(
            "year_month must be YYYY-MM, got '{raw}'"
        )));
    };
    if year_raw.len() != 4 || month_raw.len() != 2 {
        return Err(AppError::InvalidArgument(format!(
            "year_month must be YYYY-MM, got '{raw}'"
        )));
    }

    let year = year_raw
        .parse::<i32>()
        .map_err(|_| AppError::InvalidArgument(format!("bad year in '{raw}'")))?;
    let month = month_raw
        .parse::<u32>()
        .map_err(|_| AppError::InvalidArgument(format!("bad month in '{raw}'")))?;
    if !(1..=12).contains(&month) {
        return Err(AppError::InvalidArgument(format!(
            "month out of range: {month}"
        )));
    }
    Ok(YearMonth { year, month })
}

pub fn month_bounds(year_month: YearMonth) -> AppResult<MonthBounds> {
    let starts_on = NaiveDate::from_ymd_opt(year_month.year, year_month.month, 1).ok_or_else(|| {
        AppError::InvalidArgument(format!("invalid year_month '{}'", year_month.key()))
    })?;
    let (next_year, next_month) = if year_month.month == 12 {
        (year_month.year + 1, 1)
    } else {
        (year_month.year, year_month.month + 1)
    };
    let next_starts_on = NaiveDate::from_ymd_opt(next_year, next_month, 1).ok_or_else(|| {
        AppError::InvalidArgument(format!("invalid next month for '{}'", year_month.key()))
    })?;
    Ok(MonthBounds {
        starts_on,
        next_starts_on,
        days_in_month: (next_starts_on - starts_on).num_days(),
    })
}

pub fn validate_set_budget_input(
    raw: &RawSetBudgetInput<'_>,
) -> AppResult<ValidatedSetBudgetInput> {
    if raw.category_id <= 0 {
        return Err(AppError::InvalidArgument(format!(
            "category_id must be positive, got {}",
            raw.category_id
        )));
    }
    if raw.amount < 0 {
        return Err(AppError::InvalidArgument(format!(
            "amount must be zero or positive, got {}",
            raw.amount
        )));
    }
    if !(0..=200).contains(&raw.alert_threshold) {
        return Err(AppError::InvalidArgument(format!(
            "alert_threshold must be 0..=200, got {}",
            raw.alert_threshold
        )));
    }
    Ok(ValidatedSetBudgetInput {
        category_id: raw.category_id,
        year_month: parse_year_month(raw.year_month)?,
        amount: raw.amount,
        alert_threshold: raw.alert_threshold,
    })
}

pub fn evaluate_status(
    input: BudgetStatusInput,
    year_month: YearMonth,
    today: NaiveDate,
) -> AppResult<BudgetStatus> {
    let bounds = month_bounds(year_month)?;
    let percent = if input.budgeted > 0 {
        input.spent.saturating_mul(100).saturating_div(input.budgeted)
    } else if input.spent > 0 {
        200
    } else {
        0
    };
    let progress_percent = percent.clamp(0, 100);
    let selected_key = (year_month.year, year_month.month);
    let today_key = (today.year(), today.month());
    let days_left = if selected_key < today_key {
        0
    } else if selected_key > today_key {
        bounds.days_in_month
    } else {
        i64::from(bounds.next_starts_on.day()) + bounds.days_in_month - i64::from(today.day())
    };
    let projected = if selected_key < today_key {
        input.spent
    } else if selected_key > today_key {
        0
    } else {
        let elapsed_days = i64::from(today.day()).max(1);
        input
            .spent
            .saturating_mul(bounds.days_in_month)
            .saturating_div(elapsed_days)
    };
    Ok(BudgetStatus {
        category_id: input.category_id,
        category_name: input.category_name,
        category_color: input.category_color,
        category_icon: input.category_icon,
        budget_id: input.budget_id,
        budgeted: input.budgeted,
        spent: input.spent,
        percent,
        progress_percent,
        days_left,
        projected,
        alert_threshold: input.alert_threshold,
        threshold_reached: input.budgeted > 0 && percent >= input.alert_threshold,
        projected_over_budget: input.budgeted > 0 && projected > input.budgeted,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_year_month_accepts_canonical_key() {
        let ym = parse_year_month("2026-05").unwrap();

        assert_eq!(ym.year, 2026);
        assert_eq!(ym.month, 5);
        assert_eq!(ym.key(), "2026-05");
    }

    #[test]
    fn parse_year_month_rejects_non_canonical_input() {
        assert!(parse_year_month("2026/05").is_err());
        assert!(parse_year_month("2026-5").is_err());
        assert!(parse_year_month("2026-13").is_err());
        assert!(parse_year_month("abcd-05").is_err());
    }

    #[test]
    fn month_bounds_returns_start_next_start_and_days() {
        let bounds = month_bounds(parse_year_month("2026-02").unwrap()).unwrap();

        assert_eq!(bounds.starts_on.to_string(), "2026-02-01");
        assert_eq!(bounds.next_starts_on.to_string(), "2026-03-01");
        assert_eq!(bounds.days_in_month, 28);
    }

    #[test]
    fn month_bounds_handles_leap_year_and_december() {
        let leap = month_bounds(parse_year_month("2024-02").unwrap()).unwrap();
        let december = month_bounds(parse_year_month("2026-12").unwrap()).unwrap();

        assert_eq!(leap.days_in_month, 29);
        assert_eq!(december.starts_on.to_string(), "2026-12-01");
        assert_eq!(december.next_starts_on.to_string(), "2027-01-01");
        assert_eq!(december.days_in_month, 31);
    }

    #[test]
    fn validate_set_budget_input_accepts_valid_payload() {
        let validated = validate_set_budget_input(&RawSetBudgetInput {
            category_id: 1,
            year_month: "2026-05",
            amount: 50_000,
            alert_threshold: 80,
        })
        .unwrap();

        assert_eq!(validated.category_id, 1);
        assert_eq!(validated.year_month.key(), "2026-05");
        assert_eq!(validated.amount, 50_000);
        assert_eq!(validated.alert_threshold, 80);
    }

    #[test]
    fn validate_set_budget_input_rejects_invalid_values() {
        assert!(validate_set_budget_input(&RawSetBudgetInput {
            category_id: 0,
            year_month: "2026-05",
            amount: 1,
            alert_threshold: 80,
        })
        .is_err());
        assert!(validate_set_budget_input(&RawSetBudgetInput {
            category_id: 1,
            year_month: "2026-13",
            amount: 1,
            alert_threshold: 80,
        })
        .is_err());
        assert!(validate_set_budget_input(&RawSetBudgetInput {
            category_id: 1,
            year_month: "2026-05",
            amount: -1,
            alert_threshold: 80,
        })
        .is_err());
        assert!(validate_set_budget_input(&RawSetBudgetInput {
            category_id: 1,
            year_month: "2026-05",
            amount: 1,
            alert_threshold: 201,
        })
        .is_err());
    }

    #[test]
    fn evaluate_status_calculates_progress_and_projection_for_current_month() {
        let status = evaluate_status(
            BudgetStatusInput {
                category_id: 7,
                category_name: "食費".into(),
                category_color: Some("#FFAA00".into()),
                category_icon: Some("utensils".into()),
                budget_id: Some(3),
                budgeted: 60_000,
                spent: 30_000,
                alert_threshold: 80,
            },
            parse_year_month("2026-05").unwrap(),
            chrono::NaiveDate::from_ymd_opt(2026, 5, 15).unwrap(),
        )
        .unwrap();

        assert_eq!(status.percent, 50);
        assert_eq!(status.progress_percent, 50);
        assert_eq!(status.days_left, 17);
        assert_eq!(status.projected, 62_000);
        assert!(!status.threshold_reached);
        assert!(status.projected_over_budget);
    }

    #[test]
    fn evaluate_status_handles_zero_budget() {
        let no_spend = evaluate_status(
            BudgetStatusInput::minimal(1, 0, 0),
            parse_year_month("2026-05").unwrap(),
            chrono::NaiveDate::from_ymd_opt(2026, 5, 15).unwrap(),
        )
        .unwrap();
        let with_spend = evaluate_status(
            BudgetStatusInput::minimal(1, 0, 1),
            parse_year_month("2026-05").unwrap(),
            chrono::NaiveDate::from_ymd_opt(2026, 5, 15).unwrap(),
        )
        .unwrap();

        assert_eq!(no_spend.percent, 0);
        assert_eq!(with_spend.percent, 200);
        assert_eq!(with_spend.progress_percent, 100);
        assert!(!with_spend.threshold_reached);
        assert!(!with_spend.projected_over_budget);
    }

    #[test]
    fn evaluate_status_marks_threshold_reached() {
        let status = evaluate_status(
            BudgetStatusInput::minimal(1, 10_000, 8_000),
            parse_year_month("2026-05").unwrap(),
            chrono::NaiveDate::from_ymd_opt(2026, 5, 20).unwrap(),
        )
        .unwrap();

        assert_eq!(status.percent, 80);
        assert!(status.threshold_reached);
    }

    #[test]
    fn evaluate_status_uses_past_and_future_month_rules() {
        let past = evaluate_status(
            BudgetStatusInput::minimal(1, 30_000, 12_000),
            parse_year_month("2026-04").unwrap(),
            chrono::NaiveDate::from_ymd_opt(2026, 5, 15).unwrap(),
        )
        .unwrap();
        let future = evaluate_status(
            BudgetStatusInput::minimal(1, 30_000, 12_000),
            parse_year_month("2026-06").unwrap(),
            chrono::NaiveDate::from_ymd_opt(2026, 5, 15).unwrap(),
        )
        .unwrap();

        assert_eq!(past.days_left, 0);
        assert_eq!(past.projected, 12_000);
        assert_eq!(future.days_left, 30);
        assert_eq!(future.projected, 0);
    }

    mod prop_tests {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            #[test]
            fn evaluate_status_does_not_panic_or_overflow_progress(
                budgeted in 0_i64..=i64::MAX,
                spent in 0_i64..=i64::MAX,
                threshold in 0_i64..=200,
            ) {
                let status = evaluate_status(
                    BudgetStatusInput {
                        alert_threshold: threshold,
                        ..BudgetStatusInput::minimal(1, budgeted, spent)
                    },
                    parse_year_month("2026-05").unwrap(),
                    chrono::NaiveDate::from_ymd_opt(2026, 5, 15).unwrap(),
                ).unwrap();

                prop_assert!(status.progress_percent >= 0);
                prop_assert!(status.progress_percent <= 100);
            }
        }
    }
}
