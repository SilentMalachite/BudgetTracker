use std::collections::HashMap;

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

    pub fn parse_key(raw: &str) -> AppResult<Self> {
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

        let year: i32 = year_raw
            .parse()
            .map_err(|_| AppError::InvalidArgument(format!("bad year in '{raw}'")))?;
        let month: u32 = month_raw
            .parse()
            .map_err(|_| AppError::InvalidArgument(format!("bad month in '{raw}'")))?;
        if !(1..=12).contains(&month) {
            return Err(AppError::InvalidArgument(format!(
                "month out of range: {month}"
            )));
        }

        Ok(Self { year, month })
    }

    pub fn step_back(self, months: u32) -> Self {
        let total = i64::from(self.year) * 12 + i64::from(self.month - 1) - i64::from(months);
        let year = total.div_euclid(12) as i32;
        let month = (total.rem_euclid(12) + 1) as u32;
        Self { year, month }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MonthlyBucket {
    pub year_month: String,
    pub income: i64,
    pub expense: i64,
}

pub fn year_month_from_date(date: chrono::NaiveDate) -> YearMonth {
    use chrono::Datelike;
    YearMonth {
        year: date.year(),
        month: date.month(),
    }
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

    #[test]
    fn key_round_trips() {
        let ym = YearMonth {
            year: 2026,
            month: 5,
        };

        assert_eq!(ym.key(), "2026-05");
        assert_eq!(YearMonth::parse_key("2026-05").unwrap(), ym);
    }

    #[test]
    fn parse_rejects_bad_inputs() {
        assert!(YearMonth::parse_key("2026/05").is_err());
        assert!(YearMonth::parse_key("2026-13").is_err());
        assert!(YearMonth::parse_key("2026-0").is_err());
    }

    #[test]
    fn step_back_within_year() {
        assert_eq!(
            YearMonth {
                year: 2026,
                month: 5,
            }
            .step_back(2),
            YearMonth {
                year: 2026,
                month: 3,
            }
        );
    }

    #[test]
    fn step_back_crosses_year() {
        assert_eq!(
            YearMonth {
                year: 2026,
                month: 2,
            }
            .step_back(3),
            YearMonth {
                year: 2025,
                month: 11,
            }
        );
    }

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
    fn year_month_from_date_uses_the_naive_calendar_date() {
        let d = chrono::NaiveDate::from_ymd_opt(2026, 6, 1).unwrap();
        let ym = year_month_from_date(d);
        assert_eq!(ym.key(), "2026-06");
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
