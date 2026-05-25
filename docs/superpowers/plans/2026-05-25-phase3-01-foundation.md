# Phase 3 — Slice 01: Foundation (V002 migration + transfer validator)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this slice task-by-task.

**Goal:** Add the schema-side index that makes the per-account-balance query fast, and the pure-Rust `validate_transfer_input` function that the new `create_transfer` / `update_transfer` commands will rely on in slice 02. No commands are exposed yet in this slice — slice 01 is strictly "Rust types + SQL prep + tests".

**Prerequisite:** Phase 2 merged to `main`. `src-tauri/src/domain/ledger.rs` already contains `TxType`, `Transaction`, `RawInput`, `ValidatedInput`, and `validate_input` (income/expense only).

**Spec:** sections 3.3, 4.1, 4.2, 5.2, 5.5 of `docs/superpowers/specs/2026-05-24-budget-tracker-design.md` and CLAUDE.md rules 3 + 6.

---

### Task 1: V002 migration adding the counter-account index

**Files:**
- Create: `src-tauri/migrations/V002__add_counter_account_idx.sql`

- [ ] **Step 1: Write the migration**

```sql
-- =========================================================
-- V002: index transactions.counter_account_id for the
--       per-account balance query (LEFT JOIN ON
--       t.account_id = a.id OR t.counter_account_id = a.id).
-- =========================================================

CREATE INDEX IF NOT EXISTS idx_tx_counter_account
  ON transactions(counter_account_id);
```

- [ ] **Step 2: Add a migration unit test that asserts V002 is applied**

Append to `src-tauri/src/infra/migrations.rs` inside the existing `#[cfg(test)] mod tests` block:

```rust
    #[test]
    fn applies_v002_counter_account_index() {
        let mut conn = fresh();
        let version = run(&mut conn).unwrap();
        assert!(version >= 2, "expected V002 applied, got {version}");
        let exists: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master
                  WHERE type='index' AND name='idx_tx_counter_account'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(exists, 1, "idx_tx_counter_account index must exist after V002");
    }
```

Also update the existing `is_idempotent_on_second_run` test — it currently asserts the version is `1`. Replace the assertion so it accepts the latest applied version:

```rust
    #[test]
    fn is_idempotent_on_second_run() {
        let mut conn = fresh();
        let v1 = run(&mut conn).unwrap();
        let v2 = run(&mut conn).unwrap();
        assert_eq!(v1, v2);
        assert!(v2 >= 2, "expected at least V002, got {v2}");
    }
```

And `applies_v001_to_empty_db` — its name still applies because V001 is the table-creating one. Leave it alone.

- [ ] **Step 3: Run migration tests**

```bash
cd src-tauri
cargo test --lib infra::migrations
```

Expected: PASS, three tests (`applies_v001_to_empty_db`, `is_idempotent_on_second_run`, `applies_v002_counter_account_index`, plus the existing `enforces_transfer_check_constraint`).

- [ ] **Step 4: Commit**

```bash
git add src-tauri/migrations/V002__add_counter_account_idx.sql src-tauri/src/infra/migrations.rs
git commit -m "feat(infra): V002 adds idx_tx_counter_account for balance queries"
```

---

### Task 2: Transfer types and `validate_transfer_input` (TDD)

**Files:**
- Modify: `src-tauri/src/domain/ledger.rs`

- [ ] **Step 1: Append the transfer types and validator (above the existing `#[cfg(test)] mod tests` block)**

```rust
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
```

- [ ] **Step 2: Append unit tests**

Inside the existing `#[cfg(test)] mod tests` block (the one with `accepts_minimal_valid_input` etc.), append:

```rust
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
```

- [ ] **Step 3: Run tests + clippy**

```bash
cd src-tauri
cargo test --lib domain::ledger
cargo clippy --all-targets -- -D warnings
```

Expected: PASS (existing 6 tests + 5 new transfer tests + proptest module).

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/domain/ledger.rs
git commit -m "feat(domain): validate_transfer_input rejects self-transfer/bad-amount/bad-date"
```

---

### Task 3: Audit `ChangedDomain` and confirm no enum change is needed

**Files:**
- Read-only: `src-tauri/src/infra/events.rs`

- [ ] **Step 1: Confirm transfers will use existing variants**

Open `src-tauri/src/infra/events.rs` and verify the `ChangedDomain` enum has variants `Categories`, `Accounts`, `Transactions`, `Meta`. Phase 3 only needs:
- `Transactions` — emitted by `create_transfer` / `update_transfer` / `delete_transaction` (already covered)
- `Accounts` — emitted by account CRUD (already covered)

The new balances store listens to BOTH `'transactions'` and `'accounts'`. No enum change is required. If the audit finds anything missing (e.g. the variant list does not match the above), STOP and add the variant before continuing to slice 02.

- [ ] **Step 2: No commit if no change**

If no change is needed (the expected outcome), skip the commit. If something was added, commit with:

```bash
git add src-tauri/src/infra/events.rs
git commit -m "chore(events): note ChangedDomain variants covering Phase 3 broadcasts"
```

---

### Slice 01 DoD

- [ ] `src-tauri/migrations/V002__add_counter_account_idx.sql` exists; running migrations on a fresh DB produces an `idx_tx_counter_account` index.
- [ ] `src-tauri/src/domain/ledger.rs` exposes `RawTransferInput`, `ValidatedTransferInput`, `validate_transfer_input`.
- [ ] `cargo test --lib` green.
- [ ] `cargo clippy --all-targets -- -D warnings` green.
- [ ] No frontend changes yet — slice 01 is Rust-only foundation.
