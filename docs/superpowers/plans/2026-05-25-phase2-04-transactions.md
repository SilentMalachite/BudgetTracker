# Phase 2 — Slice 04: Transactions

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this slice task-by-task.

**Goal:** Implement income/expense transaction CRUD with filtered + paginated listing, plus the ledger aggregation primitives the Dashboard slice will consume.

**Prerequisite:** Slices 02 (categories) and 03 (accounts) completed.

**Spec:** sections 4.3, 5.2, 5.3 of `docs/superpowers/specs/2026-05-25-phase2-transactions-design.md`.

**Phase 2 constraint:** `type='transfer'` is rejected at the validator level. The DB CHECK constraint already allows `transfer` rows (created in Phase 1's V001), so any `transfer` row inserted by JSON import in slice 06 is preserved and must be excluded from monthly aggregates by SQL (`WHERE type IN ('income','expense')`).

---

### Task 1: `domain/ledger.rs` types and `validate_input` (TDD)

**Files:**
- Modify: `src-tauri/src/domain/ledger.rs`

- [ ] **Step 1: Write the file with tests**

```rust
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
    chrono::NaiveDate::parse_from_str(raw.occurred_on, "%Y-%m-%d")
        .map_err(|_| AppError::InvalidArgument(format!(
            "occurred_on must be YYYY-MM-DD, got '{}'", raw.occurred_on
        )))?;

    let type_ = TxType::parse(raw.type_)?;
    if matches!(type_, TxType::Transfer) {
        return Err(AppError::InvalidArgument(
            "transfer transactions are not supported in Phase 2".into(),
        ));
    }

    if raw.amount <= 0 {
        return Err(AppError::InvalidArgument(format!(
            "amount must be positive, got {}", raw.amount
        )));
    }

    let category_id = raw.category_id.ok_or_else(|| AppError::InvalidArgument(
        "income/expense transactions require a category_id".into(),
    ))?;

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
        assert!(matches!(validate_input(&ok(0)).unwrap_err(), AppError::InvalidArgument(_)));
        assert!(matches!(validate_input(&ok(-1)).unwrap_err(), AppError::InvalidArgument(_)));
    }

    #[test]
    fn rejects_bad_date() {
        let mut bad = ok(100);
        bad.occurred_on = "2026/05/25";
        assert!(matches!(validate_input(&bad).unwrap_err(), AppError::InvalidArgument(_)));
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
        assert!(matches!(validate_input(&bad).unwrap_err(), AppError::InvalidArgument(_)));
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
        assert!(matches!(validate_input(&bad).unwrap_err(), AppError::InvalidArgument(_)));
    }
}
```

- [ ] **Step 2: Test + clippy**

```bash
cargo test --lib domain::ledger
cargo clippy --all-targets -- -D warnings
```

Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/domain/ledger.rs
git commit -m "feat(domain): TxType + transaction input validator (rejects transfer in Phase 2)"
```

---

### Task 2: `aggregate_monthly` pure aggregator + proptest

**Files:**
- Modify: `src-tauri/src/domain/ledger.rs`

- [ ] **Step 1: Append types and function**

Add to the bottom of `src-tauri/src/domain/ledger.rs` (above the `#[cfg(test)]` block):

```rust
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
    MonthlySummary { income, expense, net: income - expense }
}
```

Add the missing `use` at the top of the file:

```rust
use chrono::Datelike;
```

- [ ] **Step 2: Append unit tests**

Inside the existing `#[cfg(test)] mod tests` block, append:

```rust
    fn tx(id: i64, t: TxType, amt: i64, date: &str) -> Transaction {
        Transaction {
            id,
            occurred_on: date.into(),
            type_: t,
            amount: amt,
            account_id: 1,
            counter_account_id: if matches!(t, TxType::Transfer) { Some(2) } else { None },
            category_id: if matches!(t, TxType::Transfer) { None } else { Some(1) },
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
```

- [ ] **Step 3: Add a proptest invariant**

Add a separate `#[cfg(test)] mod prop` module at the bottom of the file:

```rust
#[cfg(test)]
mod prop {
    use super::*;
    use proptest::prelude::*;

    fn tx_strategy() -> impl Strategy<Value = Transaction> {
        (
            1i64..1_000,
            prop_oneof![Just(TxType::Income), Just(TxType::Expense), Just(TxType::Transfer)],
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
                counter_account_id: if matches!(t, TxType::Transfer) { Some(2) } else { None },
                category_id: if matches!(t, TxType::Transfer) { None } else { Some(1) },
                description: String::new(),
                recurring_id: None,
                created_at: "2026-01-01".into(),
                updated_at: "2026-01-01".into(),
            })
    }

    proptest! {
        #[test]
        fn net_equals_income_minus_expense(txs in prop::collection::vec(tx_strategy(), 0..200)) {
            let s = aggregate_monthly(&txs, 2026, 5);
            prop_assert_eq!(s.net, s.income - s.expense);
            prop_assert!(s.income >= 0);
            prop_assert!(s.expense >= 0);
        }

        #[test]
        fn transfers_never_contribute(
            txs in prop::collection::vec(tx_strategy(), 0..100)
        ) {
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
```

- [ ] **Step 4: Run tests**

```bash
cargo test --lib domain::ledger
```

Expected: PASS (proptest runs 256 cases by default).

- [ ] **Step 5: Clippy**

```bash
cargo clippy --all-targets -- -D warnings
```

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/domain/ledger.rs
git commit -m "feat(domain): aggregate_monthly pure aggregator with proptest invariants"
```

---

### Task 3: `infra/repo/transaction_repo.rs` (list with filter + insert/update/delete)

**Files:**
- Create: `src-tauri/src/infra/repo/transaction_repo.rs`
- Modify: `src-tauri/src/infra/repo/mod.rs`

- [ ] **Step 1: Wire the module**

In `src-tauri/src/infra/repo/mod.rs`:

```rust
pub mod transaction_repo;
```

- [ ] **Step 2: Implement repo**

```rust
// src-tauri/src/infra/repo/transaction_repo.rs
use rusqlite::{Connection, OptionalExtension, ToSql, params};

use crate::domain::ledger::{Transaction, TxType};
use crate::error::{AppError, AppResult};

#[derive(Debug, Default, Clone)]
pub struct ListFilter {
    pub from: Option<String>,
    pub to: Option<String>,
    pub type_: Option<TxType>,
    pub category_id: Option<i64>,
    pub account_id: Option<i64>,
    pub search: Option<String>,
}

fn row_to_tx(row: &rusqlite::Row<'_>) -> rusqlite::Result<Transaction> {
    let type_raw: String = row.get("type")?;
    let type_ = match type_raw.as_str() {
        "income" => TxType::Income,
        "expense" => TxType::Expense,
        "transfer" => TxType::Transfer,
        other => {
            return Err(rusqlite::Error::FromSqlConversionFailure(
                0,
                rusqlite::types::Type::Text,
                format!("unknown tx type '{other}'").into(),
            ));
        }
    };
    Ok(Transaction {
        id: row.get("id")?,
        occurred_on: row.get("occurred_on")?,
        type_,
        amount: row.get("amount")?,
        account_id: row.get("account_id")?,
        counter_account_id: row.get("counter_account_id")?,
        category_id: row.get("category_id")?,
        description: row.get("description")?,
        recurring_id: row.get("recurring_id")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

fn build_where(filter: &ListFilter) -> (String, Vec<Box<dyn ToSql>>) {
    let mut clauses: Vec<String> = Vec::new();
    let mut binds: Vec<Box<dyn ToSql>> = Vec::new();
    if let Some(from) = &filter.from {
        clauses.push("occurred_on >= ?".into());
        binds.push(Box::new(from.clone()));
    }
    if let Some(to) = &filter.to {
        clauses.push("occurred_on <= ?".into());
        binds.push(Box::new(to.clone()));
    }
    if let Some(t) = filter.type_ {
        clauses.push("type = ?".into());
        binds.push(Box::new(t.as_sql().to_string()));
    }
    if let Some(cat) = filter.category_id {
        clauses.push("category_id = ?".into());
        binds.push(Box::new(cat));
    }
    if let Some(acc) = filter.account_id {
        clauses.push("account_id = ?".into());
        binds.push(Box::new(acc));
    }
    if let Some(q) = &filter.search {
        if !q.is_empty() {
            clauses.push("description LIKE ?".into());
            binds.push(Box::new(format!("%{q}%")));
        }
    }
    let sql = if clauses.is_empty() {
        String::new()
    } else {
        format!(" WHERE {}", clauses.join(" AND "))
    };
    (sql, binds)
}

pub fn list(
    conn: &Connection,
    filter: &ListFilter,
    page: u32,
    page_size: u32,
) -> AppResult<(Vec<Transaction>, u32)> {
    let (where_sql, binds) = build_where(filter);

    let total: u32 = {
        let count_sql = format!("SELECT COUNT(*) FROM transactions{where_sql}");
        let mut stmt = conn.prepare(&count_sql)?;
        let params_refs: Vec<&dyn ToSql> = binds.iter().map(|b| b.as_ref()).collect();
        stmt.query_row(params_refs.as_slice(), |r| r.get::<_, i64>(0))? as u32
    };

    let limit = page_size.max(1);
    let offset = page.saturating_mul(limit);
    let list_sql = format!(
        "SELECT id, occurred_on, type, amount, account_id, counter_account_id,
                category_id, description, recurring_id, created_at, updated_at
           FROM transactions{where_sql}
          ORDER BY occurred_on DESC, id DESC
          LIMIT {limit} OFFSET {offset}",
    );
    let mut stmt = conn.prepare(&list_sql)?;
    let params_refs: Vec<&dyn ToSql> = binds.iter().map(|b| b.as_ref()).collect();
    let items: Vec<Transaction> = stmt
        .query_map(params_refs.as_slice(), row_to_tx)?
        .collect::<rusqlite::Result<_>>()?;
    Ok((items, total))
}

pub fn find_by_id(conn: &Connection, id: i64) -> AppResult<Transaction> {
    conn.query_row(
        "SELECT id, occurred_on, type, amount, account_id, counter_account_id,
                category_id, description, recurring_id, created_at, updated_at
           FROM transactions WHERE id = ?1",
        params![id],
        row_to_tx,
    )
    .optional()?
    .ok_or_else(|| AppError::NotFound(format!("transaction {id}")))
}

pub struct InsertInput<'a> {
    pub occurred_on: &'a str,
    pub type_: TxType,
    pub amount: i64,
    pub account_id: i64,
    pub category_id: i64,
    pub description: &'a str,
    pub now: &'a str,
}

pub fn insert(conn: &Connection, input: &InsertInput<'_>) -> AppResult<i64> {
    conn.execute(
        "INSERT INTO transactions(occurred_on, type, amount, account_id, category_id,
                                   description, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)",
        params![
            input.occurred_on,
            input.type_.as_sql(),
            input.amount,
            input.account_id,
            input.category_id,
            input.description,
            input.now,
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

pub struct UpdateInput<'a> {
    pub occurred_on: &'a str,
    pub type_: TxType,
    pub amount: i64,
    pub account_id: i64,
    pub category_id: i64,
    pub description: &'a str,
    pub now: &'a str,
}

pub fn update(conn: &Connection, id: i64, input: &UpdateInput<'_>) -> AppResult<()> {
    let n = conn.execute(
        "UPDATE transactions
            SET occurred_on = ?1, type = ?2, amount = ?3, account_id = ?4,
                category_id = ?5, description = ?6, updated_at = ?7
          WHERE id = ?8",
        params![
            input.occurred_on,
            input.type_.as_sql(),
            input.amount,
            input.account_id,
            input.category_id,
            input.description,
            input.now,
            id,
        ],
    )?;
    if n == 0 {
        return Err(AppError::NotFound(format!("transaction {id}")));
    }
    Ok(())
}

pub fn delete(conn: &Connection, id: i64) -> AppResult<()> {
    let n = conn.execute("DELETE FROM transactions WHERE id = ?1", params![id])?;
    if n == 0 {
        return Err(AppError::NotFound(format!("transaction {id}")));
    }
    Ok(())
}
```

- [ ] **Step 3: Build + clippy**

```bash
cargo build
cargo clippy --all-targets -- -D warnings
```

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/infra/repo
git commit -m "feat(infra): transaction_repo with filtered/paginated list and CRUD"
```

---

### Task 4: Integration test for transaction_repo

**Files:**
- Create: `src-tauri/tests/integration_transactions.rs`

- [ ] **Step 1: Write tests**

```rust
use budget_tracker_lib::domain::account::AccountKind;
use budget_tracker_lib::domain::category::CategoryType;
use budget_tracker_lib::domain::ledger::TxType;
use budget_tracker_lib::infra::migrations;
use budget_tracker_lib::infra::repo::{account_repo, category_repo, transaction_repo};
use rusqlite::Connection;

const NOW: &str = "2026-05-25T00:00:00+00:00";

fn seeded_db() -> (Connection, i64, i64) {
    let mut conn = Connection::open_in_memory().unwrap();
    migrations::run(&mut conn).unwrap();
    let acc = account_repo::insert(
        &conn,
        &account_repo::InsertInput {
            name: "cash",
            kind: AccountKind::Cash,
            currency: "JPY",
            initial_balance: 0,
            display_order: 0,
            note: "",
            now: NOW,
        },
    )
    .unwrap();
    let cat = category_repo::insert(
        &conn,
        &category_repo::InsertInput {
            name: "Food",
            type_: CategoryType::Expense,
            color: None,
            icon: None,
            display_order: 0,
        },
    )
    .unwrap();
    (conn, acc, cat)
}

#[test]
fn insert_then_list_pagination() {
    let (conn, acc, cat) = seeded_db();
    for i in 1..=25 {
        transaction_repo::insert(
            &conn,
            &transaction_repo::InsertInput {
                occurred_on: &format!("2026-05-{:02}", (i % 28) + 1),
                type_: TxType::Expense,
                amount: 100 * i as i64,
                account_id: acc,
                category_id: cat,
                description: "lunch",
                now: NOW,
            },
        )
        .unwrap();
    }

    let (page0, total) = transaction_repo::list(
        &conn,
        &transaction_repo::ListFilter::default(),
        0,
        10,
    )
    .unwrap();
    assert_eq!(total, 25);
    assert_eq!(page0.len(), 10);

    let (page2, _) = transaction_repo::list(
        &conn,
        &transaction_repo::ListFilter::default(),
        2,
        10,
    )
    .unwrap();
    assert_eq!(page2.len(), 5);
}

#[test]
fn filter_by_search_and_type() {
    let (conn, acc, cat) = seeded_db();
    transaction_repo::insert(
        &conn,
        &transaction_repo::InsertInput {
            occurred_on: "2026-05-10",
            type_: TxType::Expense,
            amount: 100,
            account_id: acc,
            category_id: cat,
            description: "コーヒー",
            now: NOW,
        },
    )
    .unwrap();
    let income_cat = category_repo::insert(
        &conn,
        &category_repo::InsertInput {
            name: "Salary",
            type_: CategoryType::Income,
            color: None,
            icon: None,
            display_order: 0,
        },
    )
    .unwrap();
    transaction_repo::insert(
        &conn,
        &transaction_repo::InsertInput {
            occurred_on: "2026-05-25",
            type_: TxType::Income,
            amount: 200_000,
            account_id: acc,
            category_id: income_cat,
            description: "monthly salary",
            now: NOW,
        },
    )
    .unwrap();

    let (only_income, n_income) = transaction_repo::list(
        &conn,
        &transaction_repo::ListFilter {
            type_: Some(TxType::Income),
            ..Default::default()
        },
        0,
        50,
    )
    .unwrap();
    assert_eq!(n_income, 1);
    assert_eq!(only_income[0].amount, 200_000);

    let (coffee, n_coffee) = transaction_repo::list(
        &conn,
        &transaction_repo::ListFilter {
            search: Some("コーヒー".into()),
            ..Default::default()
        },
        0,
        50,
    )
    .unwrap();
    assert_eq!(n_coffee, 1);
    assert_eq!(coffee[0].description, "コーヒー");
}

#[test]
fn delete_removes_row() {
    let (conn, acc, cat) = seeded_db();
    let id = transaction_repo::insert(
        &conn,
        &transaction_repo::InsertInput {
            occurred_on: "2026-05-10",
            type_: TxType::Expense,
            amount: 100,
            account_id: acc,
            category_id: cat,
            description: "",
            now: NOW,
        },
    )
    .unwrap();
    transaction_repo::delete(&conn, id).unwrap();
    let err = transaction_repo::find_by_id(&conn, id).unwrap_err();
    assert!(matches!(err, budget_tracker_lib::error::AppError::NotFound(_)));
}
```

- [ ] **Step 2: Run**

```bash
cargo test --test integration_transactions
```

Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/tests/integration_transactions.rs
git commit -m "test(transaction_repo): pagination, filter, delete against memory db"
```

---

### Task 5: `commands/transactions.rs` + handler registration

**Files:**
- Modify: `src-tauri/src/commands/transactions.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Implement handlers**

```rust
// src-tauri/src/commands/transactions.rs
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};

use crate::commands::meta::AppState;
use crate::domain::ledger::{self, Transaction, TxType};
use crate::error::{AppError, AppResult};
use crate::infra::events::{ChangedDomain, emit_changed};
use crate::infra::repo::transaction_repo;

fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339()
}

#[derive(Debug, Deserialize, Default)]
pub struct ListTransactionFilter {
    pub from: Option<String>,
    pub to: Option<String>,
    #[serde(rename = "type")]
    pub type_: Option<String>,
    pub category_id: Option<i64>,
    pub account_id: Option<i64>,
    pub search: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ListTransactionResult {
    pub items: Vec<Transaction>,
    pub total: u32,
}

#[tauri::command]
pub fn list_transactions(
    state: State<'_, AppState>,
    filter: ListTransactionFilter,
    page: u32,
    page_size: u32,
) -> AppResult<ListTransactionResult> {
    let type_ = filter.type_.as_deref().map(TxType::parse).transpose()?;
    let repo_filter = transaction_repo::ListFilter {
        from: filter.from,
        to: filter.to,
        type_,
        category_id: filter.category_id,
        account_id: filter.account_id,
        search: filter.search,
    };
    let conn = state.conn.lock().map_err(|_| AppError::Corrupt("conn poisoned".into()))?;
    let (items, total) = transaction_repo::list(&conn, &repo_filter, page, page_size)?;
    Ok(ListTransactionResult { items, total })
}

#[derive(Debug, Deserialize)]
pub struct CreateTransactionInput {
    pub occurred_on: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub amount: i64,
    pub account_id: i64,
    pub category_id: Option<i64>,
    #[serde(default)]
    pub description: String,
}

#[tauri::command]
pub fn create_transaction(
    app: AppHandle,
    state: State<'_, AppState>,
    input: CreateTransactionInput,
) -> AppResult<Transaction> {
    let validated = ledger::validate_input(&ledger::RawInput {
        occurred_on: &input.occurred_on,
        type_: &input.type_,
        amount: input.amount,
        account_id: input.account_id,
        category_id: input.category_id,
        description: &input.description,
    })?;
    let now = now_iso();
    let conn = state.conn.lock().map_err(|_| AppError::Corrupt("conn poisoned".into()))?;
    let id = transaction_repo::insert(
        &conn,
        &transaction_repo::InsertInput {
            occurred_on: &validated.occurred_on,
            type_: validated.type_,
            amount: validated.amount,
            account_id: validated.account_id,
            category_id: validated.category_id,
            description: &validated.description,
            now: &now,
        },
    )?;
    let tx = transaction_repo::find_by_id(&conn, id)?;
    drop(conn);
    emit_changed(&app, ChangedDomain::Transactions);
    Ok(tx)
}

#[derive(Debug, Deserialize)]
pub struct UpdateTransactionPatch {
    pub occurred_on: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub amount: i64,
    pub account_id: i64,
    pub category_id: Option<i64>,
    #[serde(default)]
    pub description: String,
}

#[tauri::command]
pub fn update_transaction(
    app: AppHandle,
    state: State<'_, AppState>,
    id: i64,
    patch: UpdateTransactionPatch,
) -> AppResult<Transaction> {
    let validated = ledger::validate_input(&ledger::RawInput {
        occurred_on: &patch.occurred_on,
        type_: &patch.type_,
        amount: patch.amount,
        account_id: patch.account_id,
        category_id: patch.category_id,
        description: &patch.description,
    })?;
    let now = now_iso();
    let conn = state.conn.lock().map_err(|_| AppError::Corrupt("conn poisoned".into()))?;
    transaction_repo::update(
        &conn,
        id,
        &transaction_repo::UpdateInput {
            occurred_on: &validated.occurred_on,
            type_: validated.type_,
            amount: validated.amount,
            account_id: validated.account_id,
            category_id: validated.category_id,
            description: &validated.description,
            now: &now,
        },
    )?;
    let tx = transaction_repo::find_by_id(&conn, id)?;
    drop(conn);
    emit_changed(&app, ChangedDomain::Transactions);
    Ok(tx)
}

#[tauri::command]
pub fn delete_transaction(app: AppHandle, state: State<'_, AppState>, id: i64) -> AppResult<()> {
    let conn = state.conn.lock().map_err(|_| AppError::Corrupt("conn poisoned".into()))?;
    transaction_repo::delete(&conn, id)?;
    drop(conn);
    emit_changed(&app, ChangedDomain::Transactions);
    Ok(())
}
```

- [ ] **Step 2: Register handlers**

Add to `generate_handler!` in `lib.rs`:

```rust
    commands::transactions::list_transactions,
    commands::transactions::create_transaction,
    commands::transactions::update_transaction,
    commands::transactions::delete_transaction,
```

- [ ] **Step 3: Build + clippy + test**

```bash
cargo build
cargo clippy --all-targets -- -D warnings
cargo test
```

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/commands/transactions.rs src-tauri/src/lib.rs
git commit -m "feat(commands): expose transactions CRUD over tauri invoke"
```

---

### Task 6: Frontend `lib/api/transactions.ts` + Vitest

**Files:**
- Create: `src/lib/api/transactions.ts`
- Create: `src/lib/api/transactions.test.ts`
- Modify: `src/lib/api/index.ts`

- [ ] **Step 1: API wrapper**

```ts
// src/lib/api/transactions.ts
import { invoke } from '@tauri-apps/api/core';

export type TxType = 'income' | 'expense' | 'transfer';

export type Transaction = {
  id: number;
  occurred_on: string;
  type: TxType;
  amount: number;
  account_id: number;
  counter_account_id: number | null;
  category_id: number | null;
  description: string;
  recurring_id: number | null;
  created_at: string;
  updated_at: string;
};

export type ListTransactionFilter = {
  from?: string;
  to?: string;
  type?: 'income' | 'expense';
  category_id?: number;
  account_id?: number;
  search?: string;
};

export type ListTransactionResult = {
  items: Transaction[];
  total: number;
};

export function listTransactions(
  filter: ListTransactionFilter,
  page: number,
  pageSize: number,
): Promise<ListTransactionResult> {
  return invoke<ListTransactionResult>('list_transactions', {
    filter,
    page,
    pageSize,
  });
}

export type CreateTransactionInput = {
  occurred_on: string;
  type: 'income' | 'expense';
  amount: number;
  account_id: number;
  category_id: number;
  description?: string;
};

export function createTransaction(input: CreateTransactionInput): Promise<Transaction> {
  return invoke<Transaction>('create_transaction', { input });
}

export type UpdateTransactionPatch = CreateTransactionInput;

export function updateTransaction(
  id: number,
  patch: UpdateTransactionPatch,
): Promise<Transaction> {
  return invoke<Transaction>('update_transaction', { id, patch });
}

export function deleteTransaction(id: number): Promise<void> {
  return invoke('delete_transaction', { id });
}
```

- [ ] **Step 2: Vitest**

```ts
// src/lib/api/transactions.test.ts
import { describe, expect, it, vi, beforeEach } from 'vitest';

const invokeMock = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

import {
  listTransactions,
  createTransaction,
  updateTransaction,
  deleteTransaction,
} from './transactions';

describe('transactions api', () => {
  beforeEach(() => invokeMock.mockReset());

  it('passes filter/page/pageSize through to invoke', async () => {
    invokeMock.mockResolvedValueOnce({ items: [], total: 0 });
    await listTransactions({ type: 'expense' }, 2, 25);
    expect(invokeMock).toHaveBeenCalledWith('list_transactions', {
      filter: { type: 'expense' },
      page: 2,
      pageSize: 25,
    });
  });

  it('wraps input under "input" for create', async () => {
    invokeMock.mockResolvedValueOnce({});
    await createTransaction({
      occurred_on: '2026-05-25',
      type: 'income',
      amount: 1000,
      account_id: 1,
      category_id: 2,
      description: '',
    });
    expect(invokeMock).toHaveBeenCalledWith('create_transaction', {
      input: expect.objectContaining({ amount: 1000 }),
    });
  });

  it('wraps id+patch for update', async () => {
    invokeMock.mockResolvedValueOnce({});
    await updateTransaction(42, {
      occurred_on: '2026-05-25',
      type: 'expense',
      amount: 500,
      account_id: 1,
      category_id: 2,
      description: 'x',
    });
    expect(invokeMock).toHaveBeenCalledWith('update_transaction', {
      id: 42,
      patch: expect.objectContaining({ description: 'x' }),
    });
  });

  it('passes just id for delete', async () => {
    invokeMock.mockResolvedValueOnce(undefined);
    await deleteTransaction(7);
    expect(invokeMock).toHaveBeenCalledWith('delete_transaction', { id: 7 });
  });
});
```

- [ ] **Step 3: Append to barrel**

In `src/lib/api/index.ts`:

```ts
export * from './transactions';
```

- [ ] **Step 4: Run**

```bash
pnpm test
pnpm check
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/lib/api
git commit -m "feat(api): typed transactions wrapper with vitest"
```

---

### Task 7: `transactions.svelte.ts` store (filter + pagination state)

**Files:**
- Create: `src/lib/stores/transactions.svelte.ts`

- [ ] **Step 1: Write store**

```ts
// src/lib/stores/transactions.svelte.ts
import {
  listTransactions,
  type ListTransactionFilter,
  type Transaction,
} from '../api/transactions';
import { onDataChanged } from '../api/events';
import type { UnlistenFn } from '@tauri-apps/api/event';

export type TransactionsStore = {
  readonly items: Transaction[];
  readonly total: number;
  readonly loading: boolean;
  readonly error: string | null;
  readonly page: number;
  readonly pageSize: number;
  readonly filter: ListTransactionFilter;
  load(): Promise<void>;
  setFilter(filter: ListTransactionFilter): void;
  setPage(page: number): void;
  dispose(): Promise<void>;
};

export function createTransactionsStore(
  initialFilter: ListTransactionFilter = {},
  initialPageSize = 50,
): TransactionsStore {
  let items = $state<Transaction[]>([]);
  let total = $state(0);
  let loading = $state(false);
  let error = $state<string | null>(null);
  let page = $state(0);
  let pageSize = $state(initialPageSize);
  let filter = $state<ListTransactionFilter>(initialFilter);
  let unlisten: UnlistenFn | null = null;

  async function load() {
    loading = true;
    error = null;
    try {
      const res = await listTransactions(filter, page, pageSize);
      items = res.items;
      total = res.total;
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }

  function setFilter(next: ListTransactionFilter) {
    filter = next;
    page = 0;
    void load();
  }

  function setPage(next: number) {
    page = Math.max(0, next);
    void load();
  }

  void (async () => {
    unlisten = await onDataChanged((domain) => {
      if (domain === 'transactions' || domain === 'categories' || domain === 'accounts') {
        void load();
      }
    });
    await load();
  })();

  return {
    get items() { return items; },
    get total() { return total; },
    get loading() { return loading; },
    get error() { return error; },
    get page() { return page; },
    get pageSize() { return pageSize; },
    get filter() { return filter; },
    load,
    setFilter,
    setPage,
    async dispose() { unlisten?.(); unlisten = null; },
  };
}
```

- [ ] **Step 2: svelte-check**

```bash
pnpm check
```

- [ ] **Step 3: Commit**

```bash
git add src/lib/stores/transactions.svelte.ts
git commit -m "feat(store): transactions store with filter and pagination state"
```

---

### Task 8: `DatePicker.svelte` component

**Files:**
- Create: `src/lib/components/DatePicker.svelte`

- [ ] **Step 1: Implement**

```svelte
<script lang="ts">
  let {
    label,
    value = $bindable(''),
    required = false,
    testid,
  }: {
    label: string;
    value?: string;
    required?: boolean;
    testid?: string;
  } = $props();
</script>
<label class="field">
  <span>{label}{#if required}<em>*</em>{/if}</span>
  <input type="date" bind:value {required} data-testid={testid} />
</label>
<style>
  .field { display: grid; gap: var(--space-2); }
  .field span { font-size: 0.85rem; color: var(--muted); }
  .field em { color: var(--danger); margin-left: var(--space-1); font-style: normal; }
  input {
    padding: var(--space-3);
    border-radius: var(--radius-sm);
    border: 1px solid var(--border);
    background: var(--surface);
  }
</style>
```

- [ ] **Step 2: Commit**

```bash
git add src/lib/components/DatePicker.svelte
git commit -m "feat(ui): DatePicker component wrapping native input[type=date]"
```

---

### Task 9: `routes/Transactions.svelte`

**Files:**
- Modify: `src/routes/Transactions.svelte`
- Create: `src/lib/utils/yearMonth.ts`

- [ ] **Step 1: yearMonth helpers**

```ts
// src/lib/utils/yearMonth.ts
export function isoToday(): string {
  return new Date().toISOString().slice(0, 10);
}

export function monthRange(year: number, month: number): { from: string; to: string } {
  const m = String(month).padStart(2, '0');
  const last = new Date(year, month, 0).getDate();
  return {
    from: `${year}-${m}-01`,
    to: `${year}-${m}-${String(last).padStart(2, '0')}`,
  };
}
```

- [ ] **Step 2: Replace placeholder route**

```svelte
<script lang="ts">
  import Card from '../lib/components/Card.svelte';
  import Button from '../lib/components/Button.svelte';
  import Modal from '../lib/components/Modal.svelte';
  import TextField from '../lib/components/TextField.svelte';
  import Select from '../lib/components/Select.svelte';
  import DatePicker from '../lib/components/DatePicker.svelte';
  import EmptyState from '../lib/components/EmptyState.svelte';
  import { createTransactionsStore } from '../lib/stores/transactions.svelte';
  import { createCategoriesStore } from '../lib/stores/categories.svelte';
  import { createAccountsStore } from '../lib/stores/accounts.svelte';
  import {
    createTransaction,
    updateTransaction,
    deleteTransaction,
    type Transaction,
  } from '../lib/api/transactions';
  import { isoToday } from '../lib/utils/yearMonth';

  const txStore = createTransactionsStore({}, 50);
  const catStore = createCategoriesStore({ include_archived: false });
  const accStore = createAccountsStore(false);

  const yen = new Intl.NumberFormat('ja-JP', { style: 'currency', currency: 'JPY' });

  let filterType = $state<'all' | 'income' | 'expense'>('all');
  let filterFrom = $state('');
  let filterTo = $state('');
  let filterSearch = $state('');

  function applyFilter() {
    txStore.setFilter({
      type: filterType === 'all' ? undefined : filterType,
      from: filterFrom || undefined,
      to: filterTo || undefined,
      search: filterSearch || undefined,
    });
  }

  let modalOpen = $state(false);
  let editing = $state<Transaction | null>(null);
  let formType = $state<'income' | 'expense'>('expense');
  let formDate = $state(isoToday());
  let formAmount = $state('');
  let formAccount = $state('');
  let formCategory = $state('');
  let formDescription = $state('');
  let formError = $state<string | null>(null);

  function openCreate() {
    editing = null;
    formType = 'expense';
    formDate = isoToday();
    formAmount = '';
    formAccount = accStore.items[0] ? String(accStore.items[0].id) : '';
    const expenseCat = catStore.items.find((c) => c.type === 'expense');
    formCategory = expenseCat ? String(expenseCat.id) : '';
    formDescription = '';
    formError = null;
    modalOpen = true;
  }

  function openEdit(tx: Transaction) {
    if (tx.type === 'transfer') return; // Phase 2 doesn't edit transfers
    editing = tx;
    formType = tx.type;
    formDate = tx.occurred_on;
    formAmount = String(tx.amount);
    formAccount = String(tx.account_id);
    formCategory = tx.category_id ? String(tx.category_id) : '';
    formDescription = tx.description;
    formError = null;
    modalOpen = true;
  }

  async function submit() {
    formError = null;
    const amount = Number.parseInt(formAmount, 10);
    if (!Number.isFinite(amount) || amount <= 0) {
      formError = '金額は正の整数を入力してください';
      return;
    }
    const accountId = Number.parseInt(formAccount, 10);
    const categoryId = Number.parseInt(formCategory, 10);
    if (!Number.isFinite(accountId)) { formError = '口座を選択してください'; return; }
    if (!Number.isFinite(categoryId)) { formError = 'カテゴリを選択してください'; return; }
    try {
      if (editing) {
        await updateTransaction(editing.id, {
          occurred_on: formDate,
          type: formType,
          amount,
          account_id: accountId,
          category_id: categoryId,
          description: formDescription,
        });
      } else {
        await createTransaction({
          occurred_on: formDate,
          type: formType,
          amount,
          account_id: accountId,
          category_id: categoryId,
          description: formDescription,
        });
      }
      modalOpen = false;
    } catch (e) {
      formError = e instanceof Error ? e.message : String(e);
    }
  }

  async function remove(tx: Transaction) {
    if (!confirm(`「${tx.description || '取引'}」を削除しますか?`)) return;
    await deleteTransaction(tx.id);
  }

  const categoryById = $derived(
    new Map(catStore.items.map((c) => [c.id, c])),
  );
  const accountById = $derived(
    new Map(accStore.items.map((a) => [a.id, a])),
  );

  const categoryOptions = $derived(
    catStore.items
      .filter((c) => c.type === formType && !c.archived_at)
      .map((c) => ({ value: String(c.id), label: c.name })),
  );
  const accountOptions = $derived(
    accStore.items
      .filter((a) => !a.archived_at)
      .map((a) => ({ value: String(a.id), label: a.name })),
  );
</script>

<section>
  <header class="page-header">
    <h1>取引</h1>
    <Button onclick={openCreate}>
      {#snippet children()}+ 取引を追加{/snippet}
    </Button>
  </header>

  <Card>
    {#snippet children()}
      <div class="filters">
        <Select
          label="種別"
          bind:value={filterType}
          options={[
            { value: 'all', label: 'すべて' },
            { value: 'income', label: '収入' },
            { value: 'expense', label: '支出' },
          ]}
        />
        <DatePicker label="開始" bind:value={filterFrom} />
        <DatePicker label="終了" bind:value={filterTo} />
        <TextField label="フリーワード" bind:value={filterSearch} placeholder="メモを検索" />
        <Button onclick={applyFilter}>{#snippet children()}適用{/snippet}</Button>
      </div>
    {/snippet}
  </Card>

  <Card>
    {#snippet children()}
      {#if txStore.items.length === 0 && !txStore.loading}
        <EmptyState title="該当する取引がありません" hint="右上から取引を追加できます" />
      {:else}
        <table data-testid="tx-table">
          <thead>
            <tr>
              <th>日付</th><th>カテゴリ</th><th>口座</th><th>金額</th><th>メモ</th><th></th>
            </tr>
          </thead>
          <tbody>
            {#each txStore.items as tx (tx.id)}
              <tr>
                <td>{tx.occurred_on}</td>
                <td>
                  {#if tx.category_id != null}
                    {categoryById.get(tx.category_id)?.name ?? '-'}
                  {:else}振替{/if}
                </td>
                <td>{accountById.get(tx.account_id)?.name ?? '-'}</td>
                <td class:income={tx.type === 'income'} class:expense={tx.type === 'expense'}>
                  {tx.type === 'expense' ? '-' : '+'}{yen.format(tx.amount)}
                </td>
                <td>{tx.description}</td>
                <td class="actions">
                  {#if tx.type !== 'transfer'}
                    <Button variant="ghost" onclick={() => openEdit(tx)}>
                      {#snippet children()}編集{/snippet}
                    </Button>
                    <Button variant="ghost" onclick={() => remove(tx)}>
                      {#snippet children()}削除{/snippet}
                    </Button>
                  {/if}
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
        <footer class="pager">
          <span>合計 {txStore.total} 件 / ページ {txStore.page + 1}</span>
          <span class="spacer"></span>
          <Button variant="ghost" disabled={txStore.page === 0} onclick={() => txStore.setPage(txStore.page - 1)}>
            {#snippet children()}前へ{/snippet}
          </Button>
          <Button
            variant="ghost"
            disabled={(txStore.page + 1) * txStore.pageSize >= txStore.total}
            onclick={() => txStore.setPage(txStore.page + 1)}
          >
            {#snippet children()}次へ{/snippet}
          </Button>
        </footer>
      {/if}
    {/snippet}
  </Card>
</section>

<Modal
  open={modalOpen}
  title={editing ? '取引を編集' : '取引を追加'}
  onclose={() => (modalOpen = false)}
>
  {#snippet children()}
    <Select
      label="種別"
      required
      bind:value={formType}
      options={[
        { value: 'expense', label: '支出' },
        { value: 'income', label: '収入' },
      ]}
      testid="tx-type"
    />
    <DatePicker label="日付" required bind:value={formDate} testid="tx-date" />
    <TextField label="金額 (円)" required type="number" bind:value={formAmount} testid="tx-amount" />
    <Select label="口座" required bind:value={formAccount} options={accountOptions} testid="tx-account" />
    <Select label="カテゴリ" required bind:value={formCategory} options={categoryOptions} testid="tx-category" />
    <TextField label="メモ" bind:value={formDescription} testid="tx-description" />
    {#if formError}<small class="error">{formError}</small>{/if}
  {/snippet}
  {#snippet footer()}
    <Button variant="ghost" onclick={() => (modalOpen = false)}>
      {#snippet children()}キャンセル{/snippet}
    </Button>
    <Button onclick={submit}>
      {#snippet children()}{editing ? '更新' : '追加'}{/snippet}
    </Button>
  {/snippet}
</Modal>

<style>
  .page-header { display: flex; justify-content: space-between; align-items: center; margin-bottom: var(--space-5); }
  h1 { color: white; margin: 0; }
  .filters { display: grid; grid-template-columns: repeat(5, 1fr); gap: var(--space-4); align-items: end; }
  table { width: 100%; border-collapse: collapse; }
  th, td { padding: var(--space-3); border-bottom: 1px solid var(--border); text-align: left; }
  td.income { color: var(--success); font-weight: 700; }
  td.expense { color: var(--danger); font-weight: 700; }
  .actions { display: flex; gap: var(--space-2); justify-content: flex-end; }
  .pager { display: flex; align-items: center; gap: var(--space-3); margin-top: var(--space-4); }
  .pager .spacer { flex: 1; }
  .error { color: var(--danger); }
</style>
```

- [ ] **Step 3: svelte-check**

```bash
pnpm check
```

Expected: PASS.

- [ ] **Step 4: Manual smoke**

```bash
pnpm tauri dev
```

- Add an account (if not done in slice 03) and verify the dropdown lists it.
- Add an expense and an income.
- Edit the amount; delete one row; apply a filter (type + date range + search) and watch the list narrow.

Stop the dev server.

- [ ] **Step 5: Commit**

```bash
git add src/routes/Transactions.svelte src/lib/utils/yearMonth.ts
git commit -m "feat(ui): transactions page with filter, pagination, add/edit/delete"
```

---

### Transactions slice DoD

- [ ] `cargo clippy --all-targets -- -D warnings` green.
- [ ] `cargo test` green (domain + proptest + integration).
- [ ] `pnpm test` green.
- [ ] `pnpm check` green.
- [ ] Manual: create/edit/delete an income and an expense; filter narrows the list; pagination works at 50/page.
- [ ] DB-level: opening sqlite shell on the DB and inserting a `transfer` row (`INSERT INTO transactions(...) VALUES (..., 'transfer', ...)`) does NOT cause that row to appear in the editable list and is never editable from the UI.
