use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::domain::YearMonth;
use crate::error::AppResult;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MonthlyBucket {
    pub year_month: String,
    pub income: i64,
    pub expense: i64,
}

/// 1カテゴリの期間合計。`type` は `"income"` / `"expense"`。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CategoryAggregate {
    pub category_id: i64,
    pub name: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub amount: i64,
}

pub fn fill_monthly_series(
    buckets: &[MonthlyBucket],
    end: YearMonth,
    months: u32,
) -> AppResult<Vec<MonthlyBucket>> {
    if months == 0 {
        return Ok(Vec::new());
    }

    let mut by_key: HashMap<&str, &MonthlyBucket> = buckets
        .iter()
        .map(|bucket| (bucket.year_month.as_str(), bucket))
        .collect();
    let mut series = Vec::with_capacity(months as usize);

    for offset in (0..months).rev() {
        let key = end.step_back(offset).key();
        let bucket = by_key
            .remove(key.as_str())
            .cloned()
            .unwrap_or(MonthlyBucket {
                year_month: key,
                income: 0,
                expense: 0,
            });
        series.push(bucket);
    }

    Ok(series)
}

/// 期間の収入 / 支出 / 差額。`net` は常に `income - expense`。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct PeriodTotals {
    pub income: i64,
    pub expense: i64,
    pub net: i64,
}

impl PeriodTotals {
    pub fn new(income: i64, expense: i64) -> Self {
        Self {
            income,
            expense,
            net: income.saturating_sub(expense),
        }
    }
}

/// 2期間の差分。`expense_percent` は比較対象の支出が 0 のとき `None`
/// （0除算をフロントに出さないため）。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct Delta {
    pub income_diff: i64,
    pub expense_diff: i64,
    pub net_diff: i64,
    pub expense_percent: Option<i64>,
}

pub fn compare(current: PeriodTotals, previous: PeriodTotals) -> Delta {
    let expense_diff = current.expense.saturating_sub(previous.expense);
    Delta {
        income_diff: current.income.saturating_sub(previous.income),
        expense_diff,
        net_diff: current.net.saturating_sub(previous.net),
        // 整数除算は 0 方向に切り捨てられる (規約: 端数は 0 方向)。
        expense_percent: (previous.expense != 0)
            .then(|| expense_diff.saturating_mul(100) / previous.expense),
    }
}

/// 年間サマリー。平均はスライス長で割る（`report_yearly` は常に 12 件渡す）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct YearlyStats {
    pub total_income: i64,
    pub total_expense: i64,
    pub net: i64,
    pub avg_income: i64,
    pub avg_expense: i64,
    pub max_expense_month: Option<String>,
}

pub fn yearly_stats(months: &[MonthlyBucket]) -> YearlyStats {
    let total_income = months
        .iter()
        .fold(0i64, |sum, b| sum.saturating_add(b.income));
    let total_expense = months
        .iter()
        .fold(0i64, |sum, b| sum.saturating_add(b.expense));
    let divisor = months.len().max(1) as i64;

    // 同額で並んだら早い月を採るため、厳密な `>` でしか更新しない。
    let mut max: Option<&MonthlyBucket> = None;
    for bucket in months {
        if bucket.expense <= 0 {
            continue;
        }
        if max.map(|current| bucket.expense > current.expense).unwrap_or(true) {
            max = Some(bucket);
        }
    }

    YearlyStats {
        total_income,
        total_expense,
        net: total_income.saturating_sub(total_expense),
        avg_income: total_income / divisor,
        avg_expense: total_expense / divisor,
        max_expense_month: max.map(|b| b.year_month.clone()),
    }
}

/// 後方 `window` 点の単純移動平均。窓が埋まらない先頭は `None`。
pub fn moving_average(values: &[i64], window: usize) -> Vec<Option<i64>> {
    if window == 0 {
        return vec![None; values.len()];
    }
    let divisor = window as i64;
    (0..values.len())
        .map(|i| {
            if i + 1 < window {
                return None;
            }
            let sum = values[i + 1 - window..=i]
                .iter()
                .fold(0i64, |a, v| a.saturating_add(*v));
            Some(sum / divisor)
        })
        .collect()
}

/// 開始残高に月次増減を順に足し込んだ、各月末時点の残高列。
pub fn accumulate_net_worth(opening: i64, deltas: &[i64]) -> Vec<i64> {
    let mut running = opening;
    deltas
        .iter()
        .map(|delta| {
            running = running.saturating_add(*delta);
            running
        })
        .collect()
}

/// `(year_month, value)` を `months` の並びに揃える。歯抜けは 0、範囲外は捨てる。
pub fn align_to_months(rows: &[(String, i64)], months: &[String]) -> Vec<i64> {
    let index: HashMap<&str, usize> = months
        .iter()
        .enumerate()
        .map(|(i, ym)| (ym.as_str(), i))
        .collect();
    let mut out = vec![0i64; months.len()];
    for (year_month, value) in rows {
        if let Some(i) = index.get(year_month.as_str()) {
            out[*i] = out[*i].saturating_add(*value);
        }
    }
    out
}

/// repo が返す (月, カテゴリ, 金額) の1行。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CategoryMonthAmount {
    pub year_month: String,
    pub category_id: i64,
    pub name: String,
    pub type_: String,
    pub amount: i64,
}

/// 1カテゴリの月別推移。`points.len() == months.len()`。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CategorySeries {
    pub category_id: i64,
    pub name: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub points: Vec<i64>,
}

/// 行をカテゴリ毎の系列に畳む。並びは期間合計の降順、同額なら id の昇順。
pub fn pivot_category_series(
    rows: &[CategoryMonthAmount],
    months: &[String],
) -> Vec<CategorySeries> {
    let index: HashMap<&str, usize> = months
        .iter()
        .enumerate()
        .map(|(i, ym)| (ym.as_str(), i))
        .collect();

    let mut by_category: HashMap<i64, CategorySeries> = HashMap::new();
    for row in rows {
        let Some(i) = index.get(row.year_month.as_str()) else {
            continue;
        };
        let series = by_category
            .entry(row.category_id)
            .or_insert_with(|| CategorySeries {
                category_id: row.category_id,
                name: row.name.clone(),
                type_: row.type_.clone(),
                points: vec![0; months.len()],
            });
        series.points[*i] = series.points[*i].saturating_add(row.amount);
    }

    let mut series: Vec<CategorySeries> = by_category.into_values().collect();
    series.sort_by(|a, b| {
        let a_total = a.points.iter().fold(0i64, |s, v| s.saturating_add(*v));
        let b_total = b.points.iter().fold(0i64, |s, v| s.saturating_add(*v));
        b_total
            .cmp(&a_total)
            .then_with(|| a.category_id.cmp(&b.category_id))
    });
    series
}

/// 種別で絞った上位 `n` 件。`aggregates` は金額の降順で渡す前提。
pub fn top_n(aggregates: &[CategoryAggregate], type_: &str, n: usize) -> Vec<CategoryAggregate> {
    aggregates
        .iter()
        .filter(|a| a.type_ == type_)
        .take(n)
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::year_month::year_month_from_date;

    #[test]
    fn fill_pads_with_zero_buckets() {
        let series = fill_monthly_series(
            &[MonthlyBucket {
                year_month: "2026-05".into(),
                income: 1_000,
                expense: 500,
            }],
            YearMonth {
                year: 2026,
                month: 5,
            },
            3,
        )
        .unwrap();

        assert_eq!(
            series,
            vec![
                MonthlyBucket {
                    year_month: "2026-03".into(),
                    income: 0,
                    expense: 0,
                },
                MonthlyBucket {
                    year_month: "2026-04".into(),
                    income: 0,
                    expense: 0,
                },
                MonthlyBucket {
                    year_month: "2026-05".into(),
                    income: 1_000,
                    expense: 500,
                },
            ]
        );
    }

    #[test]
    fn fill_zero_months_returns_empty() {
        let r = fill_monthly_series(
            &[],
            YearMonth {
                year: 2026,
                month: 5,
            },
            0,
        )
        .unwrap();
        assert!(r.is_empty());
    }

    #[test]
    fn fill_drops_buckets_outside_window() {
        let buckets = vec![
            MonthlyBucket {
                year_month: "2025-01".into(),
                income: 1,
                expense: 0,
            },
            MonthlyBucket {
                year_month: "2026-05".into(),
                income: 2,
                expense: 0,
            },
        ];

        let series = fill_monthly_series(
            &buckets,
            YearMonth {
                year: 2026,
                month: 5,
            },
            3,
        )
        .unwrap();

        assert_eq!(series.len(), 3);
        assert!(series.iter().all(|b| b.year_month != "2025-01"));
    }

    #[test]
    fn fill_monthly_series_ends_on_year_month_from_date() {
        let end = year_month_from_date(chrono::NaiveDate::from_ymd_opt(2026, 6, 1).unwrap());
        let series = fill_monthly_series(&[], end, 12).unwrap();
        assert_eq!(series.len(), 12);
        assert_eq!(series.last().unwrap().year_month, "2026-06");
        assert_eq!(series.first().unwrap().year_month, "2025-07");
    }

    fn bucket(ym: &str, income: i64, expense: i64) -> MonthlyBucket {
        MonthlyBucket {
            year_month: ym.into(),
            income,
            expense,
        }
    }

    fn amount(ym: &str, id: i64, name: &str, type_: &str, amount: i64) -> CategoryMonthAmount {
        CategoryMonthAmount {
            year_month: ym.into(),
            category_id: id,
            name: name.into(),
            type_: type_.into(),
            amount,
        }
    }

    #[test]
    fn compare_reports_signed_diffs_and_percent() {
        let d = compare(PeriodTotals::new(300, 200), PeriodTotals::new(250, 160));

        assert_eq!(d.income_diff, 50);
        assert_eq!(d.expense_diff, 40);
        assert_eq!(d.net_diff, 10);
        assert_eq!(d.expense_percent, Some(25));
    }

    #[test]
    fn compare_returns_none_percent_when_previous_expense_is_zero() {
        let d = compare(PeriodTotals::new(300, 200), PeriodTotals::new(300, 0));
        assert_eq!(d.expense_percent, None);
    }

    #[test]
    fn compare_truncates_percent_toward_zero() {
        // -1 / 3 = -33.3% -> -33 (0 方向)
        let d = compare(PeriodTotals::new(0, 200), PeriodTotals::new(0, 300));
        assert_eq!(d.expense_percent, Some(-33));
    }

    #[test]
    fn yearly_stats_divides_by_the_slice_length() {
        let months = vec![bucket("2026-01", 100, 50), bucket("2026-02", 200, 150)];
        let stats = yearly_stats(&months);

        assert_eq!(stats.total_income, 300);
        assert_eq!(stats.total_expense, 200);
        assert_eq!(stats.net, 100);
        assert_eq!(stats.avg_income, 150);
        assert_eq!(stats.avg_expense, 100);
    }

    #[test]
    fn yearly_stats_picks_the_earliest_month_on_a_tie() {
        let months = vec![
            bucket("2026-01", 0, 500),
            bucket("2026-02", 0, 900),
            bucket("2026-03", 0, 900),
        ];
        assert_eq!(
            yearly_stats(&months).max_expense_month,
            Some("2026-02".to_string())
        );
    }

    #[test]
    fn yearly_stats_has_no_max_month_without_expense() {
        let months = vec![bucket("2026-01", 100, 0), bucket("2026-02", 100, 0)];
        let stats = yearly_stats(&months);

        assert_eq!(stats.max_expense_month, None);
        assert_eq!(stats.avg_expense, 0);
    }

    #[test]
    fn yearly_stats_of_an_empty_slice_is_all_zero() {
        let stats = yearly_stats(&[]);

        assert_eq!(stats.total_income, 0);
        assert_eq!(stats.avg_income, 0);
        assert_eq!(stats.max_expense_month, None);
    }

    #[test]
    fn moving_average_leaves_the_unfilled_head_empty() {
        let avg = moving_average(&[10, 20, 60, 40], 3);
        assert_eq!(avg, vec![None, None, Some(30), Some(40)]);
    }

    #[test]
    fn moving_average_truncates_negative_sums_toward_zero() {
        // (-1 + -1 + -2) / 3 = -1.33 -> -1
        assert_eq!(moving_average(&[-1, -1, -2], 3), vec![None, None, Some(-1)]);
    }

    #[test]
    fn moving_average_of_a_short_slice_is_all_empty() {
        assert_eq!(moving_average(&[5, 5], 3), vec![None, None]);
    }

    #[test]
    fn accumulate_net_worth_starts_from_the_opening_balance() {
        assert_eq!(
            accumulate_net_worth(1_000, &[100, -300, 50]),
            vec![1_100, 800, 850]
        );
    }

    #[test]
    fn align_to_months_zero_fills_and_drops_outsiders() {
        let months = vec!["2026-01".to_string(), "2026-02".into(), "2026-03".into()];
        let rows = vec![
            ("2026-03".to_string(), 30),
            ("2025-12".to_string(), 999),
            ("2026-01".to_string(), 10),
        ];

        assert_eq!(align_to_months(&rows, &months), vec![10, 0, 30]);
    }

    #[test]
    fn pivot_orders_by_total_desc_then_id_and_zero_fills() {
        let months = vec!["2026-01".to_string(), "2026-02".into()];
        let rows = vec![
            amount("2026-01", 1, "A", "expense", 100),
            amount("2026-02", 1, "A", "expense", 100),
            amount("2026-01", 2, "B", "expense", 500),
        ];

        let series = pivot_category_series(&rows, &months);

        assert_eq!(series.len(), 2);
        assert_eq!(series[0].category_id, 2);
        assert_eq!(series[0].points, vec![500, 0]);
        assert_eq!(series[1].category_id, 1);
        assert_eq!(series[1].points, vec![100, 100]);
    }

    #[test]
    fn top_n_filters_by_type_and_caps_the_count() {
        let aggregates = vec![
            CategoryAggregate { category_id: 1, name: "a".into(), type_: "expense".into(), amount: 300 },
            CategoryAggregate { category_id: 2, name: "b".into(), type_: "income".into(), amount: 900 },
            CategoryAggregate { category_id: 3, name: "c".into(), type_: "expense".into(), amount: 200 },
        ];

        let top = top_n(&aggregates, "expense", 1);

        assert_eq!(top.len(), 1);
        assert_eq!(top[0].category_id, 1);
        assert_eq!(top_n(&aggregates, "income", 5).len(), 1);
    }

    proptest::proptest! {
        /// 移動平均は入力と同じ長さを返し、窓が埋まる位置から先だけ値を持つ。
        #[test]
        fn moving_average_keeps_the_length(values in proptest::collection::vec(-1_000_000i64..1_000_000, 0..40)) {
            let avg = moving_average(&values, 3);
            proptest::prop_assert_eq!(avg.len(), values.len());
            for (i, point) in avg.iter().enumerate() {
                proptest::prop_assert_eq!(point.is_some(), i + 1 >= 3);
            }
        }

        /// 累積の最終値は開始残高と増減の総和に一致する。
        #[test]
        fn accumulate_ends_at_opening_plus_sum(
            opening in -1_000_000i64..1_000_000,
            deltas in proptest::collection::vec(-100_000i64..100_000, 1..40),
        ) {
            let series = accumulate_net_worth(opening, &deltas);
            let expected = deltas.iter().fold(opening, |a, d| a + d);
            proptest::prop_assert_eq!(*series.last().unwrap(), expected);
        }
    }
}
