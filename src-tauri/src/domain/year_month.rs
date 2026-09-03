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

    /// 経過月数（`self` が後ならば正）。区間長の計算に使う。
    pub fn months_since(self, earlier: Self) -> i64 {
        (i64::from(self.year) * 12 + i64::from(self.month))
            - (i64::from(earlier.year) * 12 + i64::from(earlier.month))
    }
}

pub fn year_month_from_date(date: chrono::NaiveDate) -> YearMonth {
    use chrono::Datelike;
    YearMonth {
        year: date.year(),
        month: date.month(),
    }
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
    fn year_month_from_date_uses_the_naive_calendar_date() {
        let d = chrono::NaiveDate::from_ymd_opt(2026, 6, 1).unwrap();
        let ym = year_month_from_date(d);
        assert_eq!(ym.key(), "2026-06");
    }

    #[test]
    fn months_since_counts_forward_and_backward() {
        let may = YearMonth { year: 2026, month: 5 };
        let mar = YearMonth { year: 2026, month: 3 };
        let last_nov = YearMonth { year: 2025, month: 11 };

        assert_eq!(may.months_since(mar), 2);
        assert_eq!(mar.months_since(may), -2);
        assert_eq!(may.months_since(last_nov), 6);
        assert_eq!(may.months_since(may), 0);
    }
}
