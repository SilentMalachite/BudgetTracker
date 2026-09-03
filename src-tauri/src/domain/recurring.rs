//! 定期取引ルールの日付列挙。
//!
//! ここは純粋関数だけを置く。時計 (`Local::now()`) も DB も触らない。
//! 「いつまで生成するか」は呼び出し側が `through` として渡す。
//!
//! 窓は左開右閉 `(after, through]`。`after` は `recurring_rules.last_generated_on`
//! で、これにより
//! `occurrences_between(s, a, c) == occurrences_between(s, a, b) ++ occurrences_between(s, b, c)`
//! が成り立つ。アプリを連続起動しても取引が二重生成されないのはこの性質のため。

use chrono::{Datelike, Days, NaiveDate};
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Frequency {
    Monthly,
    Weekly,
    Yearly,
}

impl Frequency {
    pub fn as_sql(self) -> &'static str {
        match self {
            Frequency::Monthly => "monthly",
            Frequency::Weekly => "weekly",
            Frequency::Yearly => "yearly",
        }
    }

    pub fn parse(raw: &str) -> AppResult<Self> {
        match raw {
            "monthly" => Ok(Self::Monthly),
            "weekly" => Ok(Self::Weekly),
            "yearly" => Ok(Self::Yearly),
            other => Err(AppError::InvalidArgument(format!(
                "frequency must be monthly|weekly|yearly, got '{other}'"
            ))),
        }
    }
}

/// 日付列挙に必要な項目だけを抜き出したもの。金額や口座は含めない。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Schedule {
    pub frequency: Frequency,
    pub day_of_month: Option<u32>,
    pub day_of_week: Option<u32>,
    pub starts_on: NaiveDate,
    pub ends_on: Option<NaiveDate>,
}

/// `recurring_rules` の 1 行。Tauri のレスポンスとしてそのまま返す。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecurringRule {
    pub id: i64,
    pub name: String,
    #[serde(rename = "type")]
    pub type_: crate::domain::ledger::TxType,
    pub amount: i64,
    pub account_id: i64,
    pub counter_account_id: Option<i64>,
    pub category_id: Option<i64>,
    pub description: String,
    pub frequency: Frequency,
    pub day_of_month: Option<u32>,
    pub day_of_week: Option<u32>,
    pub starts_on: String,
    pub ends_on: Option<String>,
    pub last_generated_on: Option<String>,
    pub active: bool,
}

impl RecurringRule {
    /// 保存済みの ISO 日付を解釈して日付列挙用の `Schedule` にする。
    pub fn schedule(&self) -> AppResult<Schedule> {
        let starts_on = crate::domain::date::parse_iso_date("starts_on", &self.starts_on)?;
        let ends_on = match &self.ends_on {
            Some(raw) => Some(crate::domain::date::parse_iso_date("ends_on", raw)?),
            None => None,
        };
        Ok(Schedule {
            frequency: self.frequency,
            day_of_month: self.day_of_month,
            day_of_week: self.day_of_week,
            starts_on,
            ends_on,
        })
    }

    /// `last_generated_on` を窓の左端 (排他) として解釈する。未生成なら `None`。
    pub fn generated_through(&self) -> AppResult<Option<NaiveDate>> {
        match &self.last_generated_on {
            Some(raw) => Ok(Some(crate::domain::date::parse_iso_date(
                "last_generated_on",
                raw,
            )?)),
            None => Ok(None),
        }
    }
}

const MAX_NAME_LEN: usize = 100;
const MAX_DESCRIPTION_LEN: usize = 200;

#[derive(Debug, Clone, Copy)]
pub struct RawRuleInput<'a> {
    pub name: &'a str,
    pub type_: &'a str,
    pub amount: i64,
    pub account_id: i64,
    pub counter_account_id: Option<i64>,
    pub category_id: Option<i64>,
    pub description: &'a str,
    pub frequency: &'a str,
    pub day_of_month: Option<u32>,
    pub day_of_week: Option<u32>,
    pub starts_on: &'a str,
    pub ends_on: Option<&'a str>,
}

#[derive(Debug, Clone)]
pub struct ValidatedRule {
    pub name: String,
    pub type_: crate::domain::ledger::TxType,
    pub amount: i64,
    pub account_id: i64,
    pub counter_account_id: Option<i64>,
    pub category_id: Option<i64>,
    pub description: String,
    pub frequency: Frequency,
    pub day_of_month: Option<u32>,
    pub day_of_week: Option<u32>,
    pub starts_on: NaiveDate,
    pub ends_on: Option<NaiveDate>,
}

impl ValidatedRule {
    pub fn schedule(&self) -> Schedule {
        Schedule {
            frequency: self.frequency,
            day_of_month: self.day_of_month,
            day_of_week: self.day_of_week,
            starts_on: self.starts_on,
            ends_on: self.ends_on,
        }
    }

    pub fn starts_on_key(&self) -> String {
        self.starts_on.format("%Y-%m-%d").to_string()
    }

    pub fn ends_on_key(&self) -> Option<String> {
        self.ends_on.map(|d| d.format("%Y-%m-%d").to_string())
    }
}

/// ルール入力の形を検証する。口座やカテゴリが実在するか / アーカイブ済みかは
/// DB を見ないと分からないので、ここではなくコマンド層で確かめる。
pub fn validate_rule_input(raw: &RawRuleInput<'_>) -> AppResult<ValidatedRule> {
    let name = raw.name.trim();
    if name.is_empty() {
        return Err(AppError::InvalidArgument("name must not be blank".into()));
    }
    if name.chars().count() > MAX_NAME_LEN {
        return Err(AppError::InvalidArgument(format!(
            "name must be {MAX_NAME_LEN} chars or fewer"
        )));
    }
    if raw.description.chars().count() > MAX_DESCRIPTION_LEN {
        return Err(AppError::InvalidArgument(format!(
            "description must be {MAX_DESCRIPTION_LEN} chars or fewer"
        )));
    }
    if raw.amount <= 0 {
        return Err(AppError::InvalidArgument(format!(
            "amount must be positive, got {}",
            raw.amount
        )));
    }

    let type_ = crate::domain::ledger::TxType::parse(raw.type_)?;
    match type_ {
        crate::domain::ledger::TxType::Transfer => {
            let counter = raw.counter_account_id.ok_or_else(|| {
                AppError::InvalidArgument("transfer rules require a counter_account_id".into())
            })?;
            if counter == raw.account_id {
                return Err(AppError::InvalidArgument(
                    "transfer rules require two different accounts".into(),
                ));
            }
            if raw.category_id.is_some() {
                return Err(AppError::InvalidArgument(
                    "transfer rules must not carry a category_id".into(),
                ));
            }
        }
        _ => {
            if raw.category_id.is_none() {
                return Err(AppError::InvalidArgument(
                    "income/expense rules require a category_id".into(),
                ));
            }
            if raw.counter_account_id.is_some() {
                return Err(AppError::InvalidArgument(
                    "income/expense rules must not carry a counter_account_id".into(),
                ));
            }
        }
    }

    let frequency = Frequency::parse(raw.frequency)?;
    match frequency {
        Frequency::Monthly | Frequency::Yearly => {
            let day = raw.day_of_month.ok_or_else(|| {
                AppError::InvalidArgument(format!(
                    "{} rules require a day_of_month",
                    frequency.as_sql()
                ))
            })?;
            if !(1..=31).contains(&day) {
                return Err(AppError::InvalidArgument(format!(
                    "day_of_month must be 1..=31, got {day}"
                )));
            }
            if raw.day_of_week.is_some() {
                return Err(AppError::InvalidArgument(format!(
                    "{} rules must not carry a day_of_week",
                    frequency.as_sql()
                )));
            }
        }
        Frequency::Weekly => {
            let day = raw.day_of_week.ok_or_else(|| {
                AppError::InvalidArgument("weekly rules require a day_of_week".into())
            })?;
            if day > 6 {
                return Err(AppError::InvalidArgument(format!(
                    "day_of_week must be 0..=6 (0 = Sunday), got {day}"
                )));
            }
            if raw.day_of_month.is_some() {
                return Err(AppError::InvalidArgument(
                    "weekly rules must not carry a day_of_month".into(),
                ));
            }
        }
    }

    let starts_on = crate::domain::date::parse_iso_date("starts_on", raw.starts_on)?;
    let ends_on = match raw.ends_on {
        Some(value) => Some(crate::domain::date::parse_iso_date("ends_on", value)?),
        None => None,
    };
    if let Some(end) = ends_on {
        if end < starts_on {
            return Err(AppError::InvalidArgument(
                "ends_on must not precede starts_on".into(),
            ));
        }
    }

    Ok(ValidatedRule {
        name: name.to_string(),
        type_,
        amount: raw.amount,
        account_id: raw.account_id,
        counter_account_id: raw.counter_account_id,
        category_id: raw.category_id,
        description: raw.description.to_string(),
        frequency,
        day_of_month: raw.day_of_month,
        day_of_week: raw.day_of_week,
        starts_on,
        ends_on,
    })
}

/// `year`/`month` の末日。
fn last_day_of_month(year: i32, month: u32) -> Option<u32> {
    let (next_year, next_month) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    NaiveDate::from_ymd_opt(next_year, next_month, 1)?
        .pred_opt()
        .map(|d| d.day())
}

/// その月に `day` が存在しなければ末日に寄せる。31 -> 2 月は 28/29、4 月は 30。
/// スキップも翌月繰り越しもしない (spec §5.4)。
fn clamp_day(year: i32, month: u32, day: u32) -> Option<NaiveDate> {
    let last = last_day_of_month(year, month)?;
    NaiveDate::from_ymd_opt(year, month, day.clamp(1, last))
}

/// 窓 `(after, through]` の下端 (この日を含む)。
fn window_start(schedule: &Schedule, after: Option<NaiveDate>) -> Option<NaiveDate> {
    match after {
        Some(a) => Some(schedule.starts_on.max(a.checked_add_days(Days::new(1))?)),
        None => Some(schedule.starts_on),
    }
}

/// `(after, through]` に発生する日付を昇順で返す。
/// `after` が `None` なら `starts_on` から遡って全件返す。
pub fn occurrences_between(
    schedule: &Schedule,
    after: Option<NaiveDate>,
    through: NaiveDate,
) -> Vec<NaiveDate> {
    let mut out = Vec::new();
    let Some(from) = window_start(schedule, after) else {
        return out;
    };
    let last = match schedule.ends_on {
        Some(end) => through.min(end),
        None => through,
    };
    if from > last {
        return out;
    }

    match schedule.frequency {
        Frequency::Weekly => {
            let target = schedule
                .day_of_week
                .unwrap_or_else(|| schedule.starts_on.weekday().num_days_from_sunday())
                % 7;
            let current = from.weekday().num_days_from_sunday();
            let offset = u64::from((target + 7 - current) % 7);
            let Some(mut date) = from.checked_add_days(Days::new(offset)) else {
                return out;
            };
            while date <= last {
                out.push(date);
                match date.checked_add_days(Days::new(7)) {
                    Some(next) => date = next,
                    None => break,
                }
            }
        }
        Frequency::Monthly => {
            let day = schedule
                .day_of_month
                .unwrap_or_else(|| schedule.starts_on.day());
            let (mut year, mut month) = (from.year(), from.month());
            while (year, month) <= (last.year(), last.month()) {
                if let Some(date) = clamp_day(year, month, day) {
                    if date >= from && date <= last {
                        out.push(date);
                    }
                }
                if month == 12 {
                    year += 1;
                    month = 1;
                } else {
                    month += 1;
                }
            }
        }
        Frequency::Yearly => {
            let day = schedule
                .day_of_month
                .unwrap_or_else(|| schedule.starts_on.day());
            let month = schedule.starts_on.month();
            for year in from.year()..=last.year() {
                if let Some(date) = clamp_day(year, month, day) {
                    if date >= from && date <= last {
                        out.push(date);
                    }
                }
            }
        }
    }
    out
}

/// `after` より後の最初の発生日。生成は行わない (UI の「次回予定」表示専用)。
/// `ends_on` を過ぎていれば `None`。
pub fn next_occurrence(schedule: &Schedule, after: NaiveDate) -> Option<NaiveDate> {
    // ends_on が無いルールでも 1 回で必ず当たる幅を取る。
    let horizon = match schedule.frequency {
        Frequency::Weekly => 14,
        Frequency::Monthly => 70,
        Frequency::Yearly => 800,
    };
    let through = schedule
        .starts_on
        .max(after)
        .checked_add_days(Days::new(horizon))?;
    occurrences_between(schedule, Some(after), through)
        .into_iter()
        .next()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    fn monthly(day: u32, starts_on: NaiveDate, ends_on: Option<NaiveDate>) -> Schedule {
        Schedule {
            frequency: Frequency::Monthly,
            day_of_month: Some(day),
            day_of_week: None,
            starts_on,
            ends_on,
        }
    }

    #[test]
    fn monthly_day_31_clamps_to_the_last_day_of_short_months() {
        let s = monthly(31, d(2026, 1, 1), None);
        assert_eq!(
            occurrences_between(&s, None, d(2026, 4, 30)),
            vec![d(2026, 1, 31), d(2026, 2, 28), d(2026, 3, 31), d(2026, 4, 30)]
        );
    }

    #[test]
    fn monthly_day_31_hits_february_29_in_a_leap_year() {
        let s = monthly(31, d(2024, 2, 1), None);
        assert_eq!(occurrences_between(&s, None, d(2024, 2, 29)), vec![d(2024, 2, 29)]);
    }

    #[test]
    fn yearly_takes_its_month_from_starts_on_and_clamps_leap_day() {
        let s = Schedule {
            frequency: Frequency::Yearly,
            day_of_month: Some(29),
            day_of_week: None,
            starts_on: d(2024, 2, 29),
            ends_on: None,
        };
        assert_eq!(
            occurrences_between(&s, None, d(2026, 12, 31)),
            vec![d(2024, 2, 29), d(2025, 2, 28), d(2026, 2, 28)]
        );
    }

    #[test]
    fn weekly_starts_at_the_first_matching_weekday_on_or_after_the_window() {
        // day_of_week = 1 (月曜)。2026-01-01 は木曜なので最初の月曜は 1/5。
        let s = Schedule {
            frequency: Frequency::Weekly,
            day_of_month: None,
            day_of_week: Some(1),
            starts_on: d(2026, 1, 1),
            ends_on: None,
        };
        assert_eq!(
            occurrences_between(&s, None, d(2026, 1, 26)),
            vec![d(2026, 1, 5), d(2026, 1, 12), d(2026, 1, 19), d(2026, 1, 26)]
        );
    }

    #[test]
    fn the_window_is_open_on_the_left_and_closed_on_the_right() {
        let s = monthly(10, d(2026, 1, 1), None);
        // 2/10 は after と同日なので出ない。3/10 は through と同日なので出る。
        assert_eq!(
            occurrences_between(&s, Some(d(2026, 2, 10)), d(2026, 3, 10)),
            vec![d(2026, 3, 10)]
        );
    }

    #[test]
    fn ends_on_cuts_the_series_off() {
        let s = monthly(10, d(2026, 1, 1), Some(d(2026, 2, 15)));
        assert_eq!(
            occurrences_between(&s, None, d(2026, 12, 31)),
            vec![d(2026, 1, 10), d(2026, 2, 10)]
        );
    }

    #[test]
    fn nothing_is_generated_before_starts_on() {
        let s = monthly(10, d(2026, 3, 1), None);
        assert_eq!(
            occurrences_between(&s, None, d(2026, 3, 31)),
            vec![d(2026, 3, 10)]
        );
    }

    #[test]
    fn next_occurrence_looks_past_the_end_of_the_window() {
        let s = monthly(31, d(2026, 1, 1), None);
        assert_eq!(next_occurrence(&s, d(2026, 1, 31)), Some(d(2026, 2, 28)));
    }

    #[test]
    fn next_occurrence_is_none_after_ends_on() {
        let s = monthly(10, d(2026, 1, 1), Some(d(2026, 2, 15)));
        assert_eq!(next_occurrence(&s, d(2026, 2, 10)), None);
    }

    #[test]
    fn frequency_parses_and_rejects_unknown_values() {
        assert_eq!(Frequency::parse("monthly").unwrap(), Frequency::Monthly);
        assert_eq!(Frequency::parse("weekly").unwrap(), Frequency::Weekly);
        assert_eq!(Frequency::parse("yearly").unwrap(), Frequency::Yearly);
        assert!(Frequency::parse("daily").is_err());
    }

    fn raw_expense() -> RawRuleInput<'static> {
        RawRuleInput {
            name: "家賃",
            type_: "expense",
            amount: 85_000,
            account_id: 1,
            counter_account_id: None,
            category_id: Some(2),
            description: "",
            frequency: "monthly",
            day_of_month: Some(27),
            day_of_week: None,
            starts_on: "2026-01-27",
            ends_on: None,
        }
    }

    #[test]
    fn a_well_formed_monthly_expense_validates() {
        let v = validate_rule_input(&raw_expense()).unwrap();
        assert_eq!(v.type_, crate::domain::ledger::TxType::Expense);
        assert_eq!(v.frequency, Frequency::Monthly);
        assert_eq!(v.starts_on, d(2026, 1, 27));
        assert_eq!(v.starts_on_key(), "2026-01-27");
        assert_eq!(v.ends_on_key(), None);
    }

    #[test]
    fn name_must_not_be_blank() {
        let raw = RawRuleInput { name: "   ", ..raw_expense() };
        assert!(validate_rule_input(&raw).is_err());
    }

    #[test]
    fn amount_must_be_positive() {
        for amount in [0, -1] {
            let raw = RawRuleInput { amount, ..raw_expense() };
            assert!(validate_rule_input(&raw).is_err(), "amount {amount}");
        }
    }

    #[test]
    fn monthly_requires_day_of_month_in_range_and_no_weekday() {
        let missing = RawRuleInput { day_of_month: None, ..raw_expense() };
        assert!(validate_rule_input(&missing).is_err());

        let out_of_range = RawRuleInput { day_of_month: Some(32), ..raw_expense() };
        assert!(validate_rule_input(&out_of_range).is_err());

        let stray_weekday = RawRuleInput { day_of_week: Some(3), ..raw_expense() };
        assert!(validate_rule_input(&stray_weekday).is_err());
    }

    #[test]
    fn weekly_requires_day_of_week_in_range_and_no_day_of_month() {
        let base = RawRuleInput { frequency: "weekly", day_of_month: None, ..raw_expense() };

        assert!(validate_rule_input(&RawRuleInput { day_of_week: None, ..base }).is_err());
        assert!(validate_rule_input(&RawRuleInput { day_of_week: Some(7), ..base }).is_err());
        assert!(validate_rule_input(&RawRuleInput {
            day_of_week: Some(1),
            day_of_month: Some(5),
            ..base
        })
        .is_err());
        assert!(validate_rule_input(&RawRuleInput { day_of_week: Some(1), ..base }).is_ok());
    }

    #[test]
    fn income_and_expense_require_a_category_and_reject_a_counter_account() {
        let no_category = RawRuleInput { category_id: None, ..raw_expense() };
        assert!(validate_rule_input(&no_category).is_err());

        let with_counter = RawRuleInput { counter_account_id: Some(3), ..raw_expense() };
        assert!(validate_rule_input(&with_counter).is_err());
    }

    #[test]
    fn transfer_requires_a_distinct_counter_account_and_no_category() {
        let base = RawRuleInput { type_: "transfer", category_id: None, ..raw_expense() };

        assert!(validate_rule_input(&RawRuleInput { counter_account_id: None, ..base }).is_err());
        assert!(validate_rule_input(&RawRuleInput { counter_account_id: Some(1), ..base }).is_err());
        assert!(validate_rule_input(&RawRuleInput {
            counter_account_id: Some(3),
            category_id: Some(2),
            ..base
        })
        .is_err());
        assert!(validate_rule_input(&RawRuleInput { counter_account_id: Some(3), ..base }).is_ok());
    }

    #[test]
    fn ends_on_must_not_precede_starts_on() {
        let raw = RawRuleInput { ends_on: Some("2026-01-26"), ..raw_expense() };
        assert!(validate_rule_input(&raw).is_err());

        let same_day = RawRuleInput { ends_on: Some("2026-01-27"), ..raw_expense() };
        assert!(validate_rule_input(&same_day).is_ok());
    }

    #[test]
    fn dates_must_be_canonical_iso() {
        let raw = RawRuleInput { starts_on: "2026-1-27", ..raw_expense() };
        assert!(validate_rule_input(&raw).is_err());
    }
}

#[cfg(test)]
mod prop_tests {
    use super::*;
    use proptest::prelude::*;

    const EPOCH: (i32, u32, u32) = (2020, 1, 1);

    fn epoch() -> NaiveDate {
        NaiveDate::from_ymd_opt(EPOCH.0, EPOCH.1, EPOCH.2).unwrap()
    }

    fn plus(base: NaiveDate, days: u64) -> NaiveDate {
        base.checked_add_days(Days::new(days)).unwrap()
    }

    prop_compose! {
        fn arb_schedule()(
            freq_index in 0usize..3,
            day_of_month in 1u32..=31,
            day_of_week in 0u32..7,
            start_offset in 0u64..2000,
            span in prop::option::of(0u64..2000),
        ) -> Schedule {
            let starts_on = plus(epoch(), start_offset);
            Schedule {
                frequency: [Frequency::Monthly, Frequency::Weekly, Frequency::Yearly][freq_index],
                day_of_month: Some(day_of_month),
                day_of_week: Some(day_of_week),
                starts_on,
                ends_on: span.map(|s| plus(starts_on, s)),
            }
        }
    }

    proptest! {
        /// 「アプリを連続起動しても二重生成されない」の本体。
        /// 窓を途中で切っても、通しで取っても、同じ日付列になる。
        #[test]
        fn splitting_the_window_yields_the_same_dates(
            schedule in arb_schedule(),
            a_offset in 0u64..3000,
            b_gap in 0u64..1500,
            c_gap in 0u64..1500,
        ) {
            let a = plus(epoch(), a_offset);
            let b = plus(a, b_gap);
            let c = plus(b, c_gap);

            let whole = occurrences_between(&schedule, Some(a), c);
            let mut split = occurrences_between(&schedule, Some(a), b);
            split.extend(occurrences_between(&schedule, Some(b), c));

            prop_assert_eq!(whole, split);
        }

        /// 同じ日に 2 回展開しても 2 回目は何も出ない。
        #[test]
        fn an_empty_window_yields_nothing(schedule in arb_schedule(), offset in 0u64..3000) {
            let day = plus(epoch(), offset);
            prop_assert!(occurrences_between(&schedule, Some(day), day).is_empty());
        }

        #[test]
        fn every_date_is_inside_the_window_and_strictly_increasing(
            schedule in arb_schedule(),
            a_offset in 0u64..3000,
            gap in 0u64..2000,
        ) {
            let after = plus(epoch(), a_offset);
            let through = plus(after, gap);
            let dates = occurrences_between(&schedule, Some(after), through);

            for pair in dates.windows(2) {
                prop_assert!(pair[0] < pair[1]);
            }
            for date in &dates {
                prop_assert!(*date > after);
                prop_assert!(*date <= through);
                prop_assert!(*date >= schedule.starts_on);
                if let Some(end) = schedule.ends_on {
                    prop_assert!(*date <= end);
                }
            }
        }

        /// next_occurrence は「窓を十分広く取ったときの先頭」と一致する。
        #[test]
        fn next_occurrence_agrees_with_a_wide_window(
            schedule in arb_schedule(),
            a_offset in 0u64..3000,
        ) {
            let after = plus(epoch(), a_offset);
            let wide = plus(schedule.starts_on.max(after), 1200);
            let expected = occurrences_between(&schedule, Some(after), wide)
                .into_iter()
                .next();
            prop_assert_eq!(next_occurrence(&schedule, after), expected);
        }
    }
}
