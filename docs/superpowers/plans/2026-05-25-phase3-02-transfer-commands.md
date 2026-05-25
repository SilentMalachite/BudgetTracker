# Phase 3 — Slice 02: Transfer repo + commands

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this slice task-by-task.

**Goal:** Persist transfer transactions through `transaction_repo` and expose dedicated `create_transfer` / `update_transfer` Tauri commands. Update `transaction_repo::list` so transfer rows are returned (Phase 2 hid them by clamping `WHERE type IN ('income','expense')`); from this slice forward the existing Transactions page lists transfers too, and the existing income/expense `validate_input` continues to reject `transfer` so the old `create_transaction` cannot create one.

**Prerequisite:** Slice 01 complete (`validate_transfer_input` lives in `domain/ledger.rs`, V002 migration applied).

**Spec:** sections 4.1, 4.2, 5.2 of `docs/superpowers/specs/2026-05-24-budget-tracker-design.md`.

**Phase 3 invariant carried into this slice:**
- `transaction_repo::list` now returns rows of any type. The existing Dashboard SQL aggregates already filter `WHERE type IN ('income','expense')` in `report_repo`, so removing the clamp from `transaction_repo::build_where` does NOT corrupt income/expense aggregates. (Slice 06 has an integration test that double-checks this.)
- `update_transaction` (income/expense) keeps writing `counter_account_id = NULL`. `update_transfer` writes `category_id = NULL` and a non-null `counter_account_id`. Crossing types (e.g. converting a transfer to an expense) is NOT supported — the UI guides the user to delete + re-add.

---

### Task 1: `transaction_repo::list` returns all types

**Files:**
- Modify: `src-tauri/src/infra/repo/transaction_repo.rs`

- [ ] **Step 1: Drop the income/expense clamp from `build_where`**

In `src-tauri/src/infra/repo/transaction_repo.rs`, find:

```rust
fn build_where(filter: &ListFilter) -> (String, Vec<Box<dyn ToSql>>) {
    let mut clauses: Vec<String> = vec!["type IN ('income','expense')".into()];
    let mut binds: Vec<Box<dyn ToSql>> = Vec::new();
```

Replace with:

```rust
fn build_where(filter: &ListFilter) -> (String, Vec<Box<dyn ToSql>>) {
    let mut clauses: Vec<String> = Vec::new();
    let mut binds: Vec<Box<dyn ToSql>> = Vec::new();
```

Then find the trailing return:

```rust
    (format!(" WHERE {}", clauses.join(" AND ")), binds)
}
```

Replace with (empty clauses case must produce no `WHERE`):

```rust
    let sql = if clauses.is_empty() {
        String::new()
    } else {
        format!(" WHERE {}", clauses.join(" AND "))
    };
    (sql, binds)
}
```

- [ ] **Step 2: Build + clippy**

```bash
cd src-tauri
cargo build
cargo clippy --all-targets -- -D warnings
```

Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/infra/repo/transaction_repo.rs
git commit -m "refactor(infra): transaction_repo::list no longer hides transfer rows"
```

---

### Task 2: `transaction_repo::insert_transfer` and `update_transfer`

**Files:**
- Modify: `src-tauri/src/infra/repo/transaction_repo.rs`

- [ ] **Step 1: Append the transfer-specific structs and functions to `transaction_repo.rs`**

Add to the bottom of `src-tauri/src/infra/repo/transaction_repo.rs`:

```rust
pub struct InsertTransferInput<'a> {
    pub occurred_on: &'a str,
    pub amount: i64,
    pub account_id: i64,
    pub counter_account_id: i64,
    pub description: &'a str,
    pub now: &'a str,
}

pub fn insert_transfer(conn: &Connection, input: &InsertTransferInput<'_>) -> AppResult<i64> {
    conn.execute(
        "INSERT INTO transactions(occurred_on, type, amount, account_id,
                                  counter_account_id, category_id,
                                  description, created_at, updated_at)
         VALUES (?1, 'transfer', ?2, ?3, ?4, NULL, ?5, ?6, ?6)",
        params![
            input.occurred_on,
            input.amount,
            input.account_id,
            input.counter_account_id,
            input.description,
            input.now,
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

pub struct UpdateTransferInput<'a> {
    pub occurred_on: &'a str,
    pub amount: i64,
    pub account_id: i64,
    pub counter_account_id: i64,
    pub description: &'a str,
    pub now: &'a str,
}

pub fn update_transfer(conn: &Connection, id: i64, input: &UpdateTransferInput<'_>) -> AppResult<()> {
    let n = conn.execute(
        "UPDATE transactions
            SET occurred_on = ?1, type = 'transfer', amount = ?2, account_id = ?3,
                counter_account_id = ?4, category_id = NULL,
                description = ?5, updated_at = ?6
          WHERE id = ?7 AND type = 'transfer'",
        params![
            input.occurred_on,
            input.amount,
            input.account_id,
            input.counter_account_id,
            input.description,
            input.now,
            id,
        ],
    )?;
    if n == 0 {
        return Err(AppError::NotFound(format!("transfer {id}")));
    }
    Ok(())
}
```

Notes:
- `update_transfer` includes `AND type = 'transfer'` in the WHERE so callers cannot accidentally turn an expense into a transfer through this path.
- The existing `update` function (income/expense) already writes `counter_account_id = NULL`; cross-type updates are an explicit non-feature.
- `delete` works for any type — no change needed.

- [ ] **Step 2: Build + clippy**

```bash
cd src-tauri
cargo build
cargo clippy --all-targets -- -D warnings
```

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/infra/repo/transaction_repo.rs
git commit -m "feat(infra): transaction_repo::insert_transfer + update_transfer"
```

---

### Task 3: Integration test for transfer round-trip

**Files:**
- Create: `src-tauri/tests/integration_transfers.rs`

- [ ] **Step 1: Write the test**

```rust
use budget_tracker_lib::domain::account::AccountKind;
use budget_tracker_lib::domain::ledger::TxType;
use budget_tracker_lib::error::AppError;
use budget_tracker_lib::infra::migrations;
use budget_tracker_lib::infra::repo::{account_repo, transaction_repo};
use rusqlite::Connection;

const NOW: &str = "2026-05-25T00:00:00+00:00";

fn seeded_db() -> (Connection, i64, i64) {
    let mut conn = Connection::open_in_memory().unwrap();
    migrations::run(&mut conn).unwrap();
    let cash = account_repo::insert(
        &conn,
        &account_repo::InsertInput {
            name: "cash",
            kind: AccountKind::Cash,
            currency: "JPY",
            initial_balance: 100_000,
            display_order: 0,
            note: "",
            now: NOW,
        },
    )
    .unwrap();
    let bank = account_repo::insert(
        &conn,
        &account_repo::InsertInput {
            name: "bank",
            kind: AccountKind::Bank,
            currency: "JPY",
            initial_balance: 500_000,
            display_order: 1,
            note: "",
            now: NOW,
        },
    )
    .unwrap();
    (conn, cash, bank)
}

#[test]
fn insert_transfer_persists_with_correct_shape() {
    let (conn, cash, bank) = seeded_db();
    let id = transaction_repo::insert_transfer(
        &conn,
        &transaction_repo::InsertTransferInput {
            occurred_on: "2026-05-25",
            amount: 30_000,
            account_id: cash,
            counter_account_id: bank,
            description: "ATM入金",
            now: NOW,
        },
    )
    .unwrap();

    let tx = transaction_repo::find_by_id(&conn, id).unwrap();
    assert!(matches!(tx.type_, TxType::Transfer));
    assert_eq!(tx.amount, 30_000);
    assert_eq!(tx.account_id, cash);
    assert_eq!(tx.counter_account_id, Some(bank));
    assert!(tx.category_id.is_none());
    assert_eq!(tx.description, "ATM入金");
}

#[test]
fn transfer_appears_in_list_unfiltered() {
    let (conn, cash, bank) = seeded_db();
    transaction_repo::insert_transfer(
        &conn,
        &transaction_repo::InsertTransferInput {
            occurred_on: "2026-05-25",
            amount: 30_000,
            account_id: cash,
            counter_account_id: bank,
            description: "",
            now: NOW,
        },
    )
    .unwrap();

    let (items, total) = transaction_repo::list(
        &conn,
        &transaction_repo::ListFilter::default(),
        0,
        50,
    )
    .unwrap();
    assert_eq!(total, 1);
    assert!(matches!(items[0].type_, TxType::Transfer));
}

#[test]
fn list_can_filter_by_transfer_type() {
    let (conn, cash, bank) = seeded_db();
    transaction_repo::insert_transfer(
        &conn,
        &transaction_repo::InsertTransferInput {
            occurred_on: "2026-05-25",
            amount: 10_000,
            account_id: cash,
            counter_account_id: bank,
            description: "",
            now: NOW,
        },
    )
    .unwrap();

    let (items, total) = transaction_repo::list(
        &conn,
        &transaction_repo::ListFilter {
            type_: Some(TxType::Transfer),
            ..Default::default()
        },
        0,
        50,
    )
    .unwrap();
    assert_eq!(total, 1);
    assert_eq!(items.len(), 1);
}

#[test]
fn update_transfer_changes_amount_and_destination() {
    let (conn, cash, bank) = seeded_db();
    let id = transaction_repo::insert_transfer(
        &conn,
        &transaction_repo::InsertTransferInput {
            occurred_on: "2026-05-25",
            amount: 1_000,
            account_id: cash,
            counter_account_id: bank,
            description: "",
            now: NOW,
        },
    )
    .unwrap();

    // Add a third account and update the transfer to send into it.
    let wallet = account_repo::insert(
        &conn,
        &account_repo::InsertInput {
            name: "wallet",
            kind: AccountKind::Cash,
            currency: "JPY",
            initial_balance: 0,
            display_order: 2,
            note: "",
            now: NOW,
        },
    )
    .unwrap();

    transaction_repo::update_transfer(
        &conn,
        id,
        &transaction_repo::UpdateTransferInput {
            occurred_on: "2026-05-26",
            amount: 2_000,
            account_id: cash,
            counter_account_id: wallet,
            description: "fix",
            now: NOW,
        },
    )
    .unwrap();

    let tx = transaction_repo::find_by_id(&conn, id).unwrap();
    assert_eq!(tx.amount, 2_000);
    assert_eq!(tx.counter_account_id, Some(wallet));
    assert_eq!(tx.occurred_on, "2026-05-26");
}

#[test]
fn update_transfer_refuses_non_transfer_row() {
    use budget_tracker_lib::domain::category::CategoryType;
    use budget_tracker_lib::infra::repo::category_repo;
    let (conn, cash, bank) = seeded_db();
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
    let expense_id = transaction_repo::insert(
        &conn,
        &transaction_repo::InsertInput {
            occurred_on: "2026-05-25",
            type_: TxType::Expense,
            amount: 100,
            account_id: cash,
            category_id: cat,
            description: "",
            now: NOW,
        },
    )
    .unwrap();

    let err = transaction_repo::update_transfer(
        &conn,
        expense_id,
        &transaction_repo::UpdateTransferInput {
            occurred_on: "2026-05-25",
            amount: 100,
            account_id: cash,
            counter_account_id: bank,
            description: "",
            now: NOW,
        },
    )
    .unwrap_err();
    assert!(matches!(err, AppError::NotFound(_)));
}

#[test]
fn delete_works_on_transfer_row() {
    let (conn, cash, bank) = seeded_db();
    let id = transaction_repo::insert_transfer(
        &conn,
        &transaction_repo::InsertTransferInput {
            occurred_on: "2026-05-25",
            amount: 500,
            account_id: cash,
            counter_account_id: bank,
            description: "",
            now: NOW,
        },
    )
    .unwrap();
    transaction_repo::delete(&conn, id).unwrap();
    let err = transaction_repo::find_by_id(&conn, id).unwrap_err();
    assert!(matches!(err, AppError::NotFound(_)));
}
```

- [ ] **Step 2: Run**

```bash
cd src-tauri
cargo test --test integration_transfers
```

Expected: PASS, six tests.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/tests/integration_transfers.rs
git commit -m "test(transfer): repo round-trip + cross-type guard against memory db"
```

---

### Task 4: `commands::transactions::create_transfer` + `update_transfer`

**Files:**
- Modify: `src-tauri/src/commands/transactions.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Append the two handlers to `commands/transactions.rs`**

Add to the bottom of `src-tauri/src/commands/transactions.rs` (above the `#[cfg(test)] mod tests` block):

```rust
#[derive(Debug, Deserialize)]
pub struct CreateTransferInput {
    pub occurred_on: String,
    pub amount: i64,
    pub account_id: i64,
    pub counter_account_id: i64,
    #[serde(default)]
    pub description: String,
}

#[tauri::command]
pub fn create_transfer(
    app: AppHandle,
    state: State<'_, AppState>,
    input: CreateTransferInput,
) -> AppResult<Transaction> {
    let validated = ledger::validate_transfer_input(&ledger::RawTransferInput {
        occurred_on: &input.occurred_on,
        amount: input.amount,
        account_id: input.account_id,
        counter_account_id: input.counter_account_id,
        description: &input.description,
    })?;
    let now = now_iso();
    let mut conn = state
        .conn
        .lock()
        .map_err(|_| AppError::Corrupt("connection mutex poisoned".into()))?;
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let id = transaction_repo::insert_transfer(
        &tx,
        &transaction_repo::InsertTransferInput {
            occurred_on: &validated.occurred_on,
            amount: validated.amount,
            account_id: validated.account_id,
            counter_account_id: validated.counter_account_id,
            description: &validated.description,
            now: &now,
        },
    )?;
    let transaction = transaction_repo::find_by_id(&tx, id)?;
    tx.commit()?;
    drop(conn);
    emit_changed(&app, ChangedDomain::Transactions);
    Ok(transaction)
}

#[derive(Debug, Deserialize)]
pub struct UpdateTransferPatch {
    pub occurred_on: String,
    pub amount: i64,
    pub account_id: i64,
    pub counter_account_id: i64,
    #[serde(default)]
    pub description: String,
}

#[tauri::command]
pub fn update_transfer(
    app: AppHandle,
    state: State<'_, AppState>,
    id: i64,
    patch: UpdateTransferPatch,
) -> AppResult<Transaction> {
    let validated = ledger::validate_transfer_input(&ledger::RawTransferInput {
        occurred_on: &patch.occurred_on,
        amount: patch.amount,
        account_id: patch.account_id,
        counter_account_id: patch.counter_account_id,
        description: &patch.description,
    })?;
    let now = now_iso();
    let mut conn = state
        .conn
        .lock()
        .map_err(|_| AppError::Corrupt("connection mutex poisoned".into()))?;
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    transaction_repo::update_transfer(
        &tx,
        id,
        &transaction_repo::UpdateTransferInput {
            occurred_on: &validated.occurred_on,
            amount: validated.amount,
            account_id: validated.account_id,
            counter_account_id: validated.counter_account_id,
            description: &validated.description,
            now: &now,
        },
    )?;
    let transaction = transaction_repo::find_by_id(&tx, id)?;
    tx.commit()?;
    drop(conn);
    emit_changed(&app, ChangedDomain::Transactions);
    Ok(transaction)
}
```

- [ ] **Step 2: Register handlers in `lib.rs`**

In `src-tauri/src/lib.rs`, inside `invoke_handler(tauri::generate_handler![...])`, append after `commands::transactions::delete_transaction,`:

```rust
            commands::transactions::create_transfer,
            commands::transactions::update_transfer,
```

- [ ] **Step 3: Build + clippy + run all tests**

```bash
cd src-tauri
cargo build
cargo clippy --all-targets -- -D warnings
cargo test
```

Expected: PASS — all Phase 2 tests + new transfer integration tests + new command-level deserializer tests if added.

- [ ] **Step 4: Add deserializer sanity test**

Append to the existing `#[cfg(test)] mod tests` in `commands/transactions.rs`:

```rust
    #[test]
    fn create_transfer_input_accepts_minimal_payload() {
        let input: CreateTransferInput = serde_json::from_str(
            r#"{
                "occurred_on": "2026-05-25",
                "amount": 30000,
                "account_id": 1,
                "counter_account_id": 2
            }"#,
        )
        .unwrap();
        assert_eq!(input.amount, 30_000);
        assert_eq!(input.description, "");
    }
```

Re-run:

```bash
cargo test --lib commands::transactions
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commands/transactions.rs src-tauri/src/lib.rs
git commit -m "feat(commands): expose create_transfer + update_transfer via invoke"
```

---

### Task 5: Frontend `lib/api/transactions.ts` — add typed transfer wrappers

**Files:**
- Modify: `src/lib/api/transactions.ts`

- [ ] **Step 1: Append transfer types and helpers**

Open `src/lib/api/transactions.ts`. The file already has `TxType = 'income' | 'expense' | 'transfer'` from Phase 2 (verify; if not, widen it now to include `'transfer'`). Append at the bottom:

```ts
export type CreateTransferInput = {
  occurred_on: string;
  amount: number;
  account_id: number;
  counter_account_id: number;
  description?: string;
};

export function createTransfer(input: CreateTransferInput): Promise<Transaction> {
  return invoke<Transaction>('create_transfer', { input });
}

export type UpdateTransferPatch = CreateTransferInput;

export function updateTransfer(
  id: number,
  patch: UpdateTransferPatch,
): Promise<Transaction> {
  return invoke<Transaction>('update_transfer', { id, patch });
}
```

- [ ] **Step 2: Extend the existing Vitest file**

Append to `src/lib/api/transactions.test.ts`:

```ts
import { createTransfer, updateTransfer } from './transactions';

describe('transfer api', () => {
  beforeEach(() => invokeMock.mockReset());

  it('wraps input under "input" for create_transfer', async () => {
    invokeMock.mockResolvedValueOnce({});
    await createTransfer({
      occurred_on: '2026-05-25',
      amount: 50_000,
      account_id: 1,
      counter_account_id: 2,
      description: 'ATM',
    });
    expect(invokeMock).toHaveBeenCalledWith('create_transfer', {
      input: expect.objectContaining({
        account_id: 1,
        counter_account_id: 2,
        amount: 50_000,
      }),
    });
  });

  it('wraps id+patch for update_transfer', async () => {
    invokeMock.mockResolvedValueOnce({});
    await updateTransfer(42, {
      occurred_on: '2026-05-25',
      amount: 10,
      account_id: 1,
      counter_account_id: 2,
      description: '',
    });
    expect(invokeMock).toHaveBeenCalledWith('update_transfer', {
      id: 42,
      patch: expect.objectContaining({ amount: 10 }),
    });
  });
});
```

- [ ] **Step 3: Run frontend tests + svelte-check**

```bash
pnpm test
pnpm check
```

Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add src/lib/api/transactions.ts src/lib/api/transactions.test.ts
git commit -m "feat(api): typed createTransfer + updateTransfer wrappers"
```

---

### Slice 02 DoD

- [ ] `cargo clippy --all-targets -- -D warnings` green.
- [ ] `cargo test` green (Phase 2 tests + new transfer integration + command deserializer tests).
- [ ] `pnpm test` green.
- [ ] `pnpm check` green.
- [ ] `transaction_repo::list` returns transfer rows. `list_transactions` in the Tauri command now exposes them too (verified in Task 1 + Task 3).
- [ ] `commands::transactions::create_transfer` and `update_transfer` are registered in `lib.rs` and reachable from `invoke`.
