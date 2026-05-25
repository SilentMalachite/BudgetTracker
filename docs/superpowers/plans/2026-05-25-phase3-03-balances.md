# Phase 3 — Slice 03: Account balances

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this slice task-by-task.

**Goal:** Compute each account's current balance (`initial_balance + income - expense - transfer_out + transfer_in`) and expose it to the frontend via `list_balances`. The pure-Rust `compute_balance` lives in `domain/balance.rs` and is proptest-covered; the production query is a single SQL pass in `infra/repo/balance_repo.rs`.

**Prerequisite:** Slice 02 complete (transfers can now be inserted via `create_transfer`).

**Spec:** sections 3.2, 5.1, 5.5 of `docs/superpowers/specs/2026-05-24-budget-tracker-design.md`. CLAUDE.md rule 3 (transfers excluded from income/expense aggregates).

---

### Task 1: `domain/balance.rs` — pure `compute_balance` (TDD)

**Files:**
- Create: `src-tauri/src/domain/balance.rs`
- Modify: `src-tauri/src/domain/mod.rs`

- [ ] **Step 1: Wire the module**

In `src-tauri/src/domain/mod.rs`, add:

```rust
pub mod balance;
```

(Existing modules `account`, `category`, `ledger`, `report`, `seed` stay.)

- [ ] **Step 2: Write the pure function with tests**

Create `src-tauri/src/domain/balance.rs`:

```rust
use crate::domain::ledger::{Transaction, TxType};

/// Compute one account's current balance from a slice of transactions.
///
/// The slice may contain transactions for any account; this function picks
/// only the rows touching `account_id` (either as source or as the
/// destination of a transfer).
///
/// The rules (CLAUDE.md rule 3):
/// - `income`   on account_id    -> +amount
/// - `expense`  on account_id    -> -amount
/// - `transfer` on account_id    -> -amount   (money left this account)
/// - `transfer` to counter==id   -> +amount   (money arrived in this account)
///
/// Saturating arithmetic keeps an absurdly large slice from panicking.
pub fn compute_balance(initial_balance: i64, txs: &[Transaction], account_id: i64) -> i64 {
    let mut balance = initial_balance;
    for tx in txs {
        match tx.type_ {
            TxType::Income if tx.account_id == account_id => {
                balance = balance.saturating_add(tx.amount);
            }
            TxType::Expense if tx.account_id == account_id => {
                balance = balance.saturating_sub(tx.amount);
            }
            TxType::Transfer if tx.account_id == account_id => {
                balance = balance.saturating_sub(tx.amount);
            }
            TxType::Transfer if tx.counter_account_id == Some(account_id) => {
                balance = balance.saturating_add(tx.amount);
            }
            _ => {}
        }
    }
    balance
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tx(t: TxType, amount: i64, account_id: i64, counter: Option<i64>) -> Transaction {
        Transaction {
            id: 0,
            occurred_on: "2026-05-25".into(),
            type_: t,
            amount,
            account_id,
            counter_account_id: counter,
            category_id: if matches!(t, TxType::Transfer) { None } else { Some(1) },
            description: String::new(),
            recurring_id: None,
            created_at: "2026-05-25T00:00:00Z".into(),
            updated_at: "2026-05-25T00:00:00Z".into(),
        }
    }

    #[test]
    fn empty_returns_initial() {
        assert_eq!(compute_balance(100_000, &[], 1), 100_000);
    }

    #[test]
    fn income_increases_balance_for_owning_account() {
        let txs = [tx(TxType::Income, 50_000, 1, None)];
        assert_eq!(compute_balance(0, &txs, 1), 50_000);
        // Other accounts are unaffected.
        assert_eq!(compute_balance(0, &txs, 2), 0);
    }

    #[test]
    fn expense_decreases_balance_for_owning_account() {
        let txs = [tx(TxType::Expense, 30_000, 1, None)];
        assert_eq!(compute_balance(100_000, &txs, 1), 70_000);
    }

    #[test]
    fn transfer_moves_money_from_source_to_destination() {
        let txs = [tx(TxType::Transfer, 20_000, 1, Some(2))];
        assert_eq!(compute_balance(50_000, &txs, 1), 30_000); // source -20_000
        assert_eq!(compute_balance(50_000, &txs, 2), 70_000); // destination +20_000
    }

    #[test]
    fn transfer_total_across_two_accounts_is_zero_net() {
        let txs = [tx(TxType::Transfer, 20_000, 1, Some(2))];
        let a = compute_balance(50_000, &txs, 1);
        let b = compute_balance(50_000, &txs, 2);
        assert_eq!((a + b) - (50_000 + 50_000), 0);
    }

    #[test]
    fn unrelated_transactions_have_no_effect() {
        let txs = [
            tx(TxType::Income, 1_000, 99, None),
            tx(TxType::Expense, 500, 99, None),
            tx(TxType::Transfer, 100, 99, Some(98)),
        ];
        assert_eq!(compute_balance(1234, &txs, 1), 1234);
    }
}

#[cfg(test)]
mod prop_tests {
    use super::*;
    use proptest::prelude::*;

    fn tx_strategy(my_account: i64, other: i64) -> impl Strategy<Value = Transaction> {
        (
            prop_oneof![
                Just(TxType::Income),
                Just(TxType::Expense),
                Just(TxType::Transfer),
            ],
            1i64..1_000_000,
            // Pick account_id and (for transfers) counter_account_id from {my_account, other, unrelated}.
            prop_oneof![Just(my_account), Just(other), Just(other + 100)],
            prop_oneof![Just(my_account), Just(other), Just(other + 100)],
        )
            .prop_filter("transfer: source != dest", move |(t, _amt, src, dst)| {
                !matches!(t, TxType::Transfer) || src != dst
            })
            .prop_map(move |(t, amt, src, dst)| Transaction {
                id: 0,
                occurred_on: "2026-05-25".into(),
                type_: t,
                amount: amt,
                account_id: src,
                counter_account_id: if matches!(t, TxType::Transfer) { Some(dst) } else { None },
                category_id: if matches!(t, TxType::Transfer) { None } else { Some(1) },
                description: String::new(),
                recurring_id: None,
                created_at: "2026-05-25T00:00:00Z".into(),
                updated_at: "2026-05-25T00:00:00Z".into(),
            })
    }

    proptest! {
        #[test]
        fn transfers_within_two_accounts_preserve_total(
            txs in proptest::collection::vec(tx_strategy(1, 2), 0..200),
            initial_a in 0i64..1_000_000,
            initial_b in 0i64..1_000_000,
        ) {
            let bal_a = compute_balance(initial_a, &txs, 1);
            let bal_b = compute_balance(initial_b, &txs, 2);

            // Sum of income on {1,2} minus sum of expense on {1,2}.
            let net: i64 = txs.iter().fold(0i64, |acc, t| match t.type_ {
                TxType::Income  if t.account_id == 1 || t.account_id == 2 =>
                    acc.saturating_add(t.amount),
                TxType::Expense if t.account_id == 1 || t.account_id == 2 =>
                    acc.saturating_sub(t.amount),
                _ => acc,
            });

            prop_assert_eq!(bal_a + bal_b, initial_a + initial_b + net);
        }
    }
}
```

- [ ] **Step 3: Run tests + clippy**

```bash
cd src-tauri
cargo test --lib domain::balance
cargo clippy --all-targets -- -D warnings
```

Expected: PASS (6 unit tests + 1 proptest case).

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/domain/balance.rs src-tauri/src/domain/mod.rs
git commit -m "feat(domain): compute_balance pure function with transfer-conservation proptest"
```

---

### Task 2: `infra/repo/balance_repo.rs` — single-pass SQL

**Files:**
- Create: `src-tauri/src/infra/repo/balance_repo.rs`
- Modify: `src-tauri/src/infra/repo/mod.rs`

- [ ] **Step 1: Wire the module**

In `src-tauri/src/infra/repo/mod.rs`, add:

```rust
pub mod balance_repo;
```

- [ ] **Step 2: Implement the repo**

Create `src-tauri/src/infra/repo/balance_repo.rs`:

```rust
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::domain::account::AccountKind;
use crate::error::AppResult;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountBalance {
    pub account_id: i64,
    pub name: String,
    pub kind: AccountKind,
    pub initial_balance: i64,
    pub balance: i64,
    pub archived_at: Option<String>,
    pub display_order: i64,
}

/// One row per account (including archived). Caller filters as needed.
///
/// SQL:
/// - LEFT JOIN matches a transaction on either `account_id = a.id` or `counter_account_id = a.id`.
/// - The CASE expression buckets the matched amount with the correct sign.
/// - Self-transfer is impossible (validator rejects it; SQL would not double-count anyway because
///   a row can match either side, not both, for a given `a.id` — V001 CHECK forbids the same id
///   in both columns indirectly: source != destination is a validator-level invariant).
pub fn list_balances(conn: &Connection) -> AppResult<Vec<AccountBalance>> {
    let mut stmt = conn.prepare(
        "SELECT a.id,
                a.name,
                a.kind,
                a.initial_balance,
                a.archived_at,
                a.display_order,
                a.initial_balance + COALESCE(SUM(
                    CASE
                        WHEN t.account_id = a.id AND t.type = 'income'   THEN  t.amount
                        WHEN t.account_id = a.id AND t.type = 'expense'  THEN -t.amount
                        WHEN t.account_id = a.id AND t.type = 'transfer' THEN -t.amount
                        WHEN t.counter_account_id = a.id AND t.type = 'transfer' THEN t.amount
                        ELSE 0
                    END
                ), 0) AS balance
           FROM accounts a
           LEFT JOIN transactions t
             ON t.account_id = a.id OR t.counter_account_id = a.id
          GROUP BY a.id
          ORDER BY a.display_order ASC, a.id ASC",
    )?;
    let rows = stmt
        .query_map([], |row| {
            let kind_raw: String = row.get(2)?;
            let kind = match kind_raw.as_str() {
                "cash" => AccountKind::Cash,
                "bank" => AccountKind::Bank,
                "credit_card" => AccountKind::CreditCard,
                "e_money" => AccountKind::EMoney,
                "investment" => AccountKind::Investment,
                other => {
                    return Err(rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        format!("unknown account kind '{other}'").into(),
                    ));
                }
            };
            Ok(AccountBalance {
                account_id: row.get(0)?,
                name: row.get(1)?,
                kind,
                initial_balance: row.get(3)?,
                archived_at: row.get(4)?,
                display_order: row.get(5)?,
                balance: row.get(6)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}
```

- [ ] **Step 3: Build + clippy**

```bash
cd src-tauri
cargo build
cargo clippy --all-targets -- -D warnings
```

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/infra/repo/balance_repo.rs src-tauri/src/infra/repo/mod.rs
git commit -m "feat(infra): balance_repo::list_balances single-pass SQL"
```

---

### Task 3: Integration test asserting balance SQL agrees with `compute_balance`

**Files:**
- Create: `src-tauri/tests/integration_balances.rs`

- [ ] **Step 1: Write the test**

```rust
use budget_tracker_lib::domain::account::AccountKind;
use budget_tracker_lib::domain::balance::compute_balance;
use budget_tracker_lib::domain::category::CategoryType;
use budget_tracker_lib::domain::ledger::TxType;
use budget_tracker_lib::infra::migrations;
use budget_tracker_lib::infra::repo::{
    account_repo, balance_repo, category_repo, transaction_repo,
};
use rusqlite::Connection;

const NOW: &str = "2026-05-25T00:00:00+00:00";

fn seeded_db() -> (Connection, i64, i64, i64) {
    let mut conn = Connection::open_in_memory().unwrap();
    migrations::run(&mut conn).unwrap();
    let cash = account_repo::insert(
        &conn,
        &account_repo::InsertInput {
            name: "cash",
            kind: AccountKind::Cash,
            currency: "JPY",
            initial_balance: 50_000,
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
            initial_balance: 200_000,
            display_order: 1,
            note: "",
            now: NOW,
        },
    )
    .unwrap();
    let food = category_repo::insert(
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
    (conn, cash, bank, food)
}

#[test]
fn empty_db_returns_initial_balances() {
    let (conn, cash, bank, _) = seeded_db();
    let balances = balance_repo::list_balances(&conn).unwrap();
    let by_id: std::collections::HashMap<i64, i64> =
        balances.iter().map(|b| (b.account_id, b.balance)).collect();
    assert_eq!(by_id[&cash], 50_000);
    assert_eq!(by_id[&bank], 200_000);
}

#[test]
fn income_expense_and_transfer_all_flow_through() {
    let (conn, cash, bank, food) = seeded_db();
    // Expense 3,000 from cash
    transaction_repo::insert(
        &conn,
        &transaction_repo::InsertInput {
            occurred_on: "2026-05-25",
            type_: TxType::Expense,
            amount: 3_000,
            account_id: cash,
            category_id: food,
            description: "",
            now: NOW,
        },
    )
    .unwrap();
    // Transfer 10,000 from cash -> bank
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

    let balances = balance_repo::list_balances(&conn).unwrap();
    let by_id: std::collections::HashMap<i64, i64> =
        balances.iter().map(|b| (b.account_id, b.balance)).collect();
    assert_eq!(by_id[&cash], 50_000 - 3_000 - 10_000);
    assert_eq!(by_id[&bank], 200_000 + 10_000);
}

#[test]
fn balance_repo_matches_pure_compute_balance() {
    let (conn, cash, bank, food) = seeded_db();
    transaction_repo::insert(
        &conn,
        &transaction_repo::InsertInput {
            occurred_on: "2026-05-25",
            type_: TxType::Expense,
            amount: 1_234,
            account_id: cash,
            category_id: food,
            description: "",
            now: NOW,
        },
    )
    .unwrap();
    transaction_repo::insert_transfer(
        &conn,
        &transaction_repo::InsertTransferInput {
            occurred_on: "2026-05-25",
            amount: 5_678,
            account_id: bank,
            counter_account_id: cash,
            description: "",
            now: NOW,
        },
    )
    .unwrap();

    let (all_txs, _) = transaction_repo::list(
        &conn,
        &transaction_repo::ListFilter::default(),
        0,
        1_000,
    )
    .unwrap();
    let balances_sql = balance_repo::list_balances(&conn).unwrap();
    for row in &balances_sql {
        let initial = row.initial_balance;
        let pure = compute_balance(initial, &all_txs, row.account_id);
        assert_eq!(
            row.balance, pure,
            "SQL balance ({}) != pure compute_balance ({}) for account {}",
            row.balance, pure, row.account_id
        );
    }
}
```

- [ ] **Step 2: Run**

```bash
cd src-tauri
cargo test --test integration_balances
```

Expected: PASS, three tests.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/tests/integration_balances.rs
git commit -m "test(balance): SQL list_balances agrees with pure compute_balance"
```

---

### Task 4: `commands/balances.rs` + handler registration

**Files:**
- Create: `src-tauri/src/commands/balances.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Wire module**

In `src-tauri/src/commands/mod.rs`, add:

```rust
pub mod balances;
```

- [ ] **Step 2: Implement the command**

Create `src-tauri/src/commands/balances.rs`:

```rust
use tauri::State;

use crate::commands::meta::AppState;
use crate::error::{AppError, AppResult};
use crate::infra::repo::balance_repo::{self, AccountBalance};

#[tauri::command]
pub fn list_balances(state: State<'_, AppState>) -> AppResult<Vec<AccountBalance>> {
    let conn = state
        .conn
        .lock()
        .map_err(|_| AppError::Corrupt("connection mutex poisoned".into()))?;
    balance_repo::list_balances(&conn)
}
```

- [ ] **Step 3: Register in `lib.rs`**

In `src-tauri/src/lib.rs`, inside `invoke_handler(tauri::generate_handler![...])`, append:

```rust
            commands::balances::list_balances,
```

- [ ] **Step 4: Build + clippy + test**

```bash
cd src-tauri
cargo build
cargo clippy --all-targets -- -D warnings
cargo test
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commands/balances.rs src-tauri/src/commands/mod.rs src-tauri/src/lib.rs
git commit -m "feat(commands): list_balances tauri command"
```

---

### Task 5: Frontend `lib/api/balances.ts` + Vitest

**Files:**
- Create: `src/lib/api/balances.ts`
- Create: `src/lib/api/balances.test.ts`
- Modify: `src/lib/api/index.ts`

- [ ] **Step 1: API wrapper**

Create `src/lib/api/balances.ts`:

```ts
import { invoke } from '@tauri-apps/api/core';
import type { AccountKind } from './accounts';

export type AccountBalance = {
  account_id: number;
  name: string;
  kind: AccountKind;
  initial_balance: number;
  balance: number;
  archived_at: string | null;
  display_order: number;
};

export function listBalances(): Promise<AccountBalance[]> {
  return invoke<AccountBalance[]>('list_balances');
}
```

- [ ] **Step 2: Vitest**

Create `src/lib/api/balances.test.ts`:

```ts
import { beforeEach, describe, expect, it, vi } from 'vitest';

const invokeMock = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

import { listBalances } from './balances';

describe('balances api', () => {
  beforeEach(() => invokeMock.mockReset());

  it('invokes list_balances with no args', async () => {
    invokeMock.mockResolvedValueOnce([]);
    await listBalances();
    expect(invokeMock).toHaveBeenCalledWith('list_balances');
  });

  it('returns the rust payload unchanged', async () => {
    const payload = [
      {
        account_id: 1,
        name: 'cash',
        kind: 'cash',
        initial_balance: 1000,
        balance: 1500,
        archived_at: null,
        display_order: 0,
      },
    ];
    invokeMock.mockResolvedValueOnce(payload);
    const result = await listBalances();
    expect(result).toEqual(payload);
  });
});
```

- [ ] **Step 3: Add barrel export**

In `src/lib/api/index.ts`, append:

```ts
export * from './balances';
```

- [ ] **Step 4: Run tests + svelte-check**

```bash
pnpm test
pnpm check
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/lib/api/balances.ts src/lib/api/balances.test.ts src/lib/api/index.ts
git commit -m "feat(api): typed listBalances wrapper with vitest"
```

---

### Task 6: `lib/stores/balances.svelte.ts` — reactive store

**Files:**
- Create: `src/lib/stores/balances.svelte.ts`

- [ ] **Step 1: Write the store**

Create `src/lib/stores/balances.svelte.ts`:

```ts
import type { UnlistenFn } from '@tauri-apps/api/event';
import { listBalances, type AccountBalance } from '../api/balances';
import { onDataChanged } from '../api/events';

export type BalancesStore = {
  readonly items: AccountBalance[];
  readonly loading: boolean;
  readonly error: string | null;
  readonly totalAssets: number;
  load(): Promise<void>;
  dispose(): Promise<void>;
};

export function createBalancesStore(): BalancesStore {
  let items = $state<AccountBalance[]>([]);
  let loading = $state(false);
  let error = $state<string | null>(null);
  let unlisten: UnlistenFn | null = null;
  let disposed = false;

  async function load() {
    loading = true;
    error = null;
    try {
      items = await listBalances();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }

  void (async () => {
    await load();
    try {
      const nextUnlisten = await onDataChanged((domain) => {
        if (domain === 'transactions' || domain === 'accounts') void load();
      });
      if (disposed) nextUnlisten();
      else unlisten = nextUnlisten;
    } catch {
      // Browser-only E2E has no Tauri event bus; initial load still ran.
    }
  })();

  return {
    get items() {
      return items;
    },
    get loading() {
      return loading;
    },
    get error() {
      return error;
    },
    get totalAssets() {
      return items
        .filter((row) => row.archived_at == null)
        .reduce((sum, row) => sum + row.balance, 0);
    },
    load,
    async dispose() {
      disposed = true;
      if (unlisten) {
        unlisten();
        unlisten = null;
      }
    },
  };
}
```

- [ ] **Step 2: svelte-check**

```bash
pnpm check
```

Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add src/lib/stores/balances.svelte.ts
git commit -m "feat(store): balances store reacts to transactions and accounts changes"
```

---

### Slice 03 DoD

- [ ] `cargo clippy --all-targets -- -D warnings` green.
- [ ] `cargo test` green (Phase 2 + transfer + new balance unit/proptest + integration).
- [ ] `pnpm test` green (balances Vitest added).
- [ ] `pnpm check` green.
- [ ] `list_balances` is registered as a Tauri command and reachable from frontend.
- [ ] `createBalancesStore()` returns reactive `items` and a `totalAssets` getter that excludes archived accounts.
