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
}
