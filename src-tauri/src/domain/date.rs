//! Canonical ISO-8601 calendar dates.
//!
//! Every date column in the schema (`transactions.occurred_on`,
//! `budgets.starts_on`, `recurring_rules.starts_on` / `ends_on`, ...) is a
//! TEXT column that SQL compares lexically: `occurred_on BETWEEN 'YYYY-MM-01'
//! AND 'YYYY-MM-31'`, `strftime('%Y-%m', occurred_on)`, `occurred_on >= ?`.
//! That only works when every stored value has the exact shape `YYYY-MM-DD`.
//! chrono's `%Y-%m-%d` alone is lenient (it accepts `2026-5-5`,
//! ` 2026-05-05`, `+2026-05-05`), so the parser here checks the byte shape
//! first and only then asks chrono whether the calendar date exists.

use chrono::NaiveDate;

use crate::error::{AppError, AppResult};

const ISO_DATE_LEN: usize = 10;

/// `DDDD-DD-DD`: exactly ten ASCII bytes, digits everywhere except the two
/// hyphens at byte offsets 4 and 7.
fn has_iso_shape(raw: &str) -> bool {
    let bytes = raw.as_bytes();
    bytes.len() == ISO_DATE_LEN
        && bytes.iter().enumerate().all(|(i, b)| match i {
            4 | 7 => *b == b'-',
            _ => b.is_ascii_digit(),
        })
}

/// Parse `raw` as a canonical `YYYY-MM-DD` calendar date.
///
/// `field` names the offending input in the error message (`occurred_on`,
/// `starts_on`, `from`, ...) so callers keep the existing
/// "`<field> must be YYYY-MM-DD, got '...'`" wording.
pub fn parse_iso_date(field: &str, raw: &str) -> AppResult<NaiveDate> {
    let parsed = if has_iso_shape(raw) {
        NaiveDate::parse_from_str(raw, "%Y-%m-%d").ok()
    } else {
        None
    };
    parsed.ok_or_else(|| {
        AppError::InvalidArgument(format!("{field} must be YYYY-MM-DD, got '{raw}'"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_canonical_dates_including_leap_day() {
        assert_eq!(
            parse_iso_date("d", "2026-05-05").unwrap(),
            NaiveDate::from_ymd_opt(2026, 5, 5).unwrap()
        );
        assert_eq!(
            parse_iso_date("d", "2024-02-29").unwrap(),
            NaiveDate::from_ymd_opt(2024, 2, 29).unwrap()
        );
    }

    #[test]
    fn rejects_non_canonical_shapes_chrono_would_accept() {
        for raw in [
            "2026-5-5",
            " 2026-05-05",
            "+2026-05-05",
            "2026-05-05 ",
            "2026/05/05",
            "20260505",
            "２０２６-05-05",
            "",
        ] {
            let err = parse_iso_date("occurred_on", raw).unwrap_err();
            assert!(
                matches!(err, AppError::InvalidArgument(_)),
                "{raw:?} must be rejected, got {err:?}"
            );
            assert!(
                err.to_string().contains("occurred_on must be YYYY-MM-DD"),
                "{err}"
            );
        }
    }

    #[test]
    fn rejects_well_shaped_but_impossible_calendar_dates() {
        for raw in [
            "2026-02-30",
            "2025-02-29",
            "2026-13-01",
            "2026-00-10",
            "2026-04-31",
            "2026-05-00",
        ] {
            assert!(
                parse_iso_date("d", raw).is_err(),
                "{raw:?} must be rejected"
            );
        }
    }
}

#[cfg(test)]
mod prop_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn every_formatted_valid_date_round_trips(
            y in 1i32..=9999,
            m in 1u32..=12,
            d in 1u32..=28,
        ) {
            let date = NaiveDate::from_ymd_opt(y, m, d).unwrap();
            let raw = date.format("%Y-%m-%d").to_string();
            prop_assert_eq!(parse_iso_date("d", &raw).unwrap(), date);
        }

        #[test]
        fn anything_accepted_has_the_exact_iso_shape(raw in "[ +0-9/-]{0,12}") {
            if parse_iso_date("d", &raw).is_ok() {
                prop_assert_eq!(raw.len(), ISO_DATE_LEN);
                prop_assert!(has_iso_shape(&raw));
            }
        }
    }
}
