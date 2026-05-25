use chrono::Datelike;
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TxType {
    Income,
    Expense,
    Transfer,
}

impl TxType {
    pub fn as_sql(self) -> &'static str {
        match self {
            TxType::Income => "income",
            TxType::Expense => "expense",
            TxType::Transfer => "transfer",
        }
    }

    pub fn parse(raw: &str) -> AppResult<Self> {
        match raw {
            "income" => Ok(Self::Income),
            "expense" => Ok(Self::Expense),
            "transfer" => Ok(Self::Transfer),
            other => Err(AppError::InvalidArgument(format!(
                "transaction type must be income|expense|transfer, got '{other}'"
            ))),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub id: i64,
    pub occurred_on: String,
    #[serde(rename = "type")]
    pub type_: TxType,
    pub amount: i64,
    pub account_id: i64,
    pub counter_account_id: Option<i64>,
    pub category_id: Option<i64>,
    pub description: String,
    pub recurring_id: Option<i64>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct ValidatedInput {
    pub occurred_on: String,
    pub type_: TxType,
    pub amount: i64,
    pub account_id: i64,
    pub category_id: i64,
    pub description: String,
}

const MAX_DESCRIPTION_LEN: usize = 200;

pub struct RawInput<'a> {
    pub occurred_on: &'a str,
    pub type_: &'a str,
    pub amount: i64,
    pub account_id: i64,
    pub category_id: Option<i64>,
    pub description: &'a str,
}

/// Validate an income/expense transaction input. Phase 2 rejects `transfer`.
pub fn validate_input(raw: &RawInput<'_>) -> AppResult<ValidatedInput> {
    chrono::NaiveDate::parse_from_str(raw.occurred_on, "%Y-%m-%d").map_err(|_| {
        AppError::InvalidArgument(format!(
            "occurred_on must be YYYY-MM-DD, got '{}'",
            raw.occurred_on
        ))
    })?;

    let type_ = TxType::parse(raw.type_)?;
    if matches!(type_, TxType::Transfer) {
        return Err(AppError::InvalidArgument(
            "transfer transactions are not supported in Phase 2".into(),
        ));
    }

    if raw.amount <= 0 {
        return Err(AppError::InvalidArgument(format!(
            "amount must be positive, got {}",
            raw.amount
        )));
    }

    let category_id = raw.category_id.ok_or_else(|| {
        AppError::InvalidArgument("income/expense transactions require a category_id".into())
    })?;

    if raw.description.chars().count() > MAX_DESCRIPTION_LEN {
        return Err(AppError::InvalidArgument(format!(
            "description must be {MAX_DESCRIPTION_LEN} chars or fewer"
        )));
    }

    Ok(ValidatedInput {
        occurred_on: raw.occurred_on.to_string(),
        type_,
        amount: raw.amount,
        account_id: raw.account_id,
        category_id,
        description: raw.description.to_string(),
    })
}

#[derive(Debug, Clone, Serialize)]
pub struct MonthlySummary {
    pub income: i64,
    pub expense: i64,
    pub net: i64,
}

/// Aggregate a slice of transactions into income / expense / net for a given
/// (year, month). Transfer rows are silently excluded (Phase 2 invariant).
pub fn aggregate_monthly(txs: &[Transaction], year: i32, month: u32) -> MonthlySummary {
    let mut income: i64 = 0;
    let mut expense: i64 = 0;
    for tx in txs {
        if !matches!(tx.type_, TxType::Income | TxType::Expense) {
            continue;
        }
        let Ok(date) = chrono::NaiveDate::parse_from_str(&tx.occurred_on, "%Y-%m-%d") else {
            continue;
        };
        if date.year() != year || date.month() != month {
            continue;
        }
        match tx.type_ {
            TxType::Income => income = income.saturating_add(tx.amount),
            TxType::Expense => expense = expense.saturating_add(tx.amount),
            TxType::Transfer => {}
        }
    }
    MonthlySummary {
        income,
        expense,
        net: income - expense,
    }
}

#[derive(Debug, Clone)]
pub struct ValidatedTransferInput {
    pub occurred_on: String,
    pub amount: i64,
    pub account_id: i64,
    pub counter_account_id: i64,
    pub description: String,
}

pub struct RawTransferInput<'a> {
    pub occurred_on: &'a str,
    pub amount: i64,
    pub account_id: i64,
    pub counter_account_id: i64,
    pub description: &'a str,
}

/// Validate a transfer transaction input.
///
/// Invariants enforced here (the V001 CHECK enforces shape; the validator
/// enforces things SQL cannot, like source != destination):
/// - `occurred_on` is `YYYY-MM-DD`.
/// - `amount > 0`.
/// - `account_id != counter_account_id` (the CHECK constraint does not catch this).
/// - `description.chars().count() <= MAX_DESCRIPTION_LEN`.
pub fn validate_transfer_input(raw: &RawTransferInput<'_>) -> AppResult<ValidatedTransferInput> {
    chrono::NaiveDate::parse_from_str(raw.occurred_on, "%Y-%m-%d").map_err(|_| {
        AppError::InvalidArgument(format!(
            "occurred_on must be YYYY-MM-DD, got '{}'",
            raw.occurred_on
        ))
    })?;

    if raw.amount <= 0 {
        return Err(AppError::InvalidArgument(format!(
            "amount must be positive, got {}",
            raw.amount
        )));
    }

    if raw.account_id == raw.counter_account_id {
        return Err(AppError::InvalidArgument(
            "transfer source and destination must differ".into(),
        ));
    }

    if raw.description.chars().count() > MAX_DESCRIPTION_LEN {
        return Err(AppError::InvalidArgument(format!(
            "description must be {MAX_DESCRIPTION_LEN} chars or fewer"
        )));
    }

    Ok(ValidatedTransferInput {
        occurred_on: raw.occurred_on.to_string(),
        amount: raw.amount,
        account_id: raw.account_id,
        counter_account_id: raw.counter_account_id,
        description: raw.description.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(amount: i64) -> RawInput<'static> {
        RawInput {
            occurred_on: "2026-05-25",
            type_: "expense",
            amount,
            account_id: 1,
            category_id: Some(2),
            description: "ランチ",
        }
    }

    #[test]
    fn accepts_minimal_valid_input() {
        let v = validate_input(&ok(500)).unwrap();
        assert_eq!(v.amount, 500);
        assert_eq!(v.type_, TxType::Expense);
        assert_eq!(v.category_id, 2);
    }

    #[test]
    fn rejects_zero_or_negative_amount() {
        assert!(matches!(
            validate_input(&ok(0)).unwrap_err(),
            AppError::InvalidArgument(_)
        ));
        assert!(matches!(
            validate_input(&ok(-1)).unwrap_err(),
            AppError::InvalidArgument(_)
        ));
    }

    #[test]
    fn rejects_bad_date() {
        let mut bad = ok(100);
        bad.occurred_on = "2026/05/25";
        assert!(matches!(
            validate_input(&bad).unwrap_err(),
            AppError::InvalidArgument(_)
        ));
    }

    #[test]
    fn rejects_transfer_type() {
        let mut bad = ok(100);
        bad.type_ = "transfer";
        let err = validate_input(&bad).unwrap_err();
        assert!(matches!(err, AppError::InvalidArgument(_)));
        assert!(err.to_string().contains("transfer"));
    }

    #[test]
    fn rejects_missing_category() {
        let mut bad = ok(100);
        bad.category_id = None;
        assert!(matches!(
            validate_input(&bad).unwrap_err(),
            AppError::InvalidArgument(_)
        ));
    }

    #[test]
    fn rejects_too_long_description() {
        let long = "あ".repeat(201);
        let bad = RawInput {
            occurred_on: "2026-05-25",
            type_: "expense",
            amount: 100,
            account_id: 1,
            category_id: Some(2),
            description: &long,
        };
        assert!(matches!(
            validate_input(&bad).unwrap_err(),
            AppError::InvalidArgument(_)
        ));
    }

    fn tx(id: i64, t: TxType, amt: i64, date: &str) -> Transaction {
        Transaction {
            id,
            occurred_on: date.into(),
            type_: t,
            amount: amt,
            account_id: 1,
            counter_account_id: if matches!(t, TxType::Transfer) {
                Some(2)
            } else {
                None
            },
            category_id: if matches!(t, TxType::Transfer) {
                None
            } else {
                Some(1)
            },
            description: String::new(),
            recurring_id: None,
            created_at: date.into(),
            updated_at: date.into(),
        }
    }

    #[test]
    fn aggregate_sums_income_and_expense_for_target_month() {
        let txs = vec![
            tx(1, TxType::Income, 300_000, "2026-05-01"),
            tx(2, TxType::Expense, 1_500, "2026-05-15"),
            tx(3, TxType::Expense, 2_500, "2026-05-25"),
        ];
        let s = aggregate_monthly(&txs, 2026, 5);
        assert_eq!(s.income, 300_000);
        assert_eq!(s.expense, 4_000);
        assert_eq!(s.net, 296_000);
    }

    #[test]
    fn aggregate_ignores_other_months() {
        let txs = vec![
            tx(1, TxType::Income, 100, "2026-04-30"),
            tx(2, TxType::Income, 200, "2026-05-01"),
            tx(3, TxType::Income, 300, "2026-06-01"),
        ];
        let s = aggregate_monthly(&txs, 2026, 5);
        assert_eq!(s.income, 200);
    }

    #[test]
    fn aggregate_excludes_transfers() {
        let txs = vec![
            tx(1, TxType::Transfer, 1_000_000, "2026-05-10"),
            tx(2, TxType::Income, 100, "2026-05-10"),
            tx(3, TxType::Expense, 50, "2026-05-10"),
        ];
        let s = aggregate_monthly(&txs, 2026, 5);
        assert_eq!(s.income, 100);
        assert_eq!(s.expense, 50);
        assert_eq!(s.net, 50);
    }

    #[test]
    fn aggregate_ignores_malformed_dates() {
        let mut bad = tx(1, TxType::Income, 100, "2026-13-99");
        bad.occurred_on = "garbage".into();
        let s = aggregate_monthly(&[bad], 2026, 5);
        assert_eq!(s.income, 0);
    }

    fn ok_transfer() -> RawTransferInput<'static> {
        RawTransferInput {
            occurred_on: "2026-05-25",
            amount: 50_000,
            account_id: 1,
            counter_account_id: 2,
            description: "現金→銀行",
        }
    }

    #[test]
    fn transfer_accepts_minimal_valid_input() {
        let v = validate_transfer_input(&ok_transfer()).unwrap();
        assert_eq!(v.amount, 50_000);
        assert_eq!(v.account_id, 1);
        assert_eq!(v.counter_account_id, 2);
    }

    #[test]
    fn transfer_rejects_same_source_and_destination() {
        let mut bad = ok_transfer();
        bad.counter_account_id = bad.account_id;
        let err = validate_transfer_input(&bad).unwrap_err();
        assert!(matches!(err, AppError::InvalidArgument(_)));
        assert!(err.to_string().contains("source and destination"));
    }

    #[test]
    fn transfer_rejects_zero_or_negative_amount() {
        let mut zero = ok_transfer();
        zero.amount = 0;
        assert!(matches!(
            validate_transfer_input(&zero).unwrap_err(),
            AppError::InvalidArgument(_)
        ));
        let mut neg = ok_transfer();
        neg.amount = -1;
        assert!(matches!(
            validate_transfer_input(&neg).unwrap_err(),
            AppError::InvalidArgument(_)
        ));
    }

    #[test]
    fn transfer_rejects_bad_date() {
        let mut bad = ok_transfer();
        bad.occurred_on = "2026/05/25";
        assert!(matches!(
            validate_transfer_input(&bad).unwrap_err(),
            AppError::InvalidArgument(_)
        ));
    }

    #[test]
    fn transfer_rejects_too_long_description() {
        let long = "あ".repeat(201);
        let bad = RawTransferInput {
            occurred_on: "2026-05-25",
            amount: 1,
            account_id: 1,
            counter_account_id: 2,
            description: &long,
        };
        assert!(matches!(
            validate_transfer_input(&bad).unwrap_err(),
            AppError::InvalidArgument(_)
        ));
    }
}

#[cfg(test)]
mod prop_tests {
    use super::*;
    use proptest::prelude::*;

    fn tx_strategy() -> impl Strategy<Value = Transaction> {
        (
            1i64..1_000,
            prop_oneof![
                Just(TxType::Income),
                Just(TxType::Expense),
                Just(TxType::Transfer)
            ],
            1i64..1_000_000_000,
            2026i32..2027,
            1u32..=12u32,
            1u32..=28u32,
        )
            .prop_map(|(id, t, amt, y, m, d)| Transaction {
                id,
                occurred_on: format!("{y:04}-{m:02}-{d:02}"),
                type_: t,
                amount: amt,
                account_id: 1,
                counter_account_id: if matches!(t, TxType::Transfer) {
                    Some(2)
                } else {
                    None
                },
                category_id: if matches!(t, TxType::Transfer) {
                    None
                } else {
                    Some(1)
                },
                description: String::new(),
                recurring_id: None,
                created_at: "2026-01-01".into(),
                updated_at: "2026-01-01".into(),
            })
    }

    proptest! {
        #[test]
        fn net_equals_income_minus_expense(txs in proptest::collection::vec(tx_strategy(), 0..200)) {
            let s = aggregate_monthly(&txs, 2026, 5);
            prop_assert_eq!(s.net, s.income - s.expense);
            prop_assert!(s.income >= 0);
            prop_assert!(s.expense >= 0);
        }

        #[test]
        fn transfers_never_contribute(txs in proptest::collection::vec(tx_strategy(), 0..100)) {
            let s_all = aggregate_monthly(&txs, 2026, 5);
            let no_transfer: Vec<_> = txs
                .into_iter()
                .filter(|t| !matches!(t.type_, TxType::Transfer))
                .collect();
            let s_filtered = aggregate_monthly(&no_transfer, 2026, 5);
            prop_assert_eq!(s_all.income, s_filtered.income);
            prop_assert_eq!(s_all.expense, s_filtered.expense);
        }
    }
}
