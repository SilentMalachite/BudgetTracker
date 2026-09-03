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
