# Phase 3 — Slice 06: Backup roundtrip, E2E, Phase 3 final DoD

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this slice task-by-task.

**Goal:** Close out Phase 3 with three things:
1. Extend `integration_backup.rs` with a round-trip that includes a transfer row and asserts both shape and balance survive an export → import cycle.
2. Add a Playwright E2E (`transfer-flow.spec.ts`) covering: create two accounts, create a transfer, assert per-account balance + total-assets.
3. Run the Phase 3 final Definition-of-Done sweep.

**Prerequisite:** Slices 01-05 complete.

**Spec:** sections 8, 10 of `docs/superpowers/specs/2026-05-24-budget-tracker-design.md`. CLAUDE.md "フェーズ完了条件" (phase completion criteria).

---

### Task 1: Backup roundtrip preserves transfers

**Files:**
- Modify: `src-tauri/tests/integration_backup.rs`

- [ ] **Step 1: Add a transfer-aware roundtrip test**

Append the following test to `src-tauri/tests/integration_backup.rs`. (If your existing helpers use a different `seed` pattern, mirror that pattern; the snippet below assumes `Connection::open_in_memory()` + `migrations::run` matches the convention used in the other tests in the same file.)

```rust
#[test]
fn export_then_overwrite_import_preserves_transfer_row() {
    use budget_tracker_lib::commands::backup::{export_snapshot_json, import_snapshot_json};
    use budget_tracker_lib::domain::account::AccountKind;
    use budget_tracker_lib::domain::ledger::TxType;
    use budget_tracker_lib::infra::migrations;
    use budget_tracker_lib::infra::repo::{account_repo, balance_repo, transaction_repo};

    const NOW: &str = "2026-05-25T00:00:00+00:00";

    // --- Source DB: two accounts + one transfer ---
    let mut src = rusqlite::Connection::open_in_memory().unwrap();
    migrations::run(&mut src).unwrap();
    let cash = account_repo::insert(
        &src,
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
        &src,
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
    let transfer_id = transaction_repo::insert_transfer(
        &src,
        &transaction_repo::InsertTransferInput {
            occurred_on: "2026-05-25",
            amount: 30_000,
            account_id: bank,
            counter_account_id: cash,
            description: "ATM",
            now: NOW,
        },
    )
    .unwrap();

    let src_balances = balance_repo::list_balances(&src).unwrap();
    let src_total: i64 = src_balances.iter().map(|b| b.balance).sum();

    // --- Snapshot to JSON ---
    let snapshot = export_snapshot_json(&src).unwrap();

    // --- Destination DB: empty, imported in 'overwrite' mode ---
    let mut dst = rusqlite::Connection::open_in_memory().unwrap();
    migrations::run(&mut dst).unwrap();
    let _import_result = import_snapshot_json(&mut dst, &snapshot, "overwrite").unwrap();

    // 1) Transfer row is back with the same shape.
    let restored = transaction_repo::find_by_id(&dst, transfer_id).unwrap();
    assert!(matches!(restored.type_, TxType::Transfer));
    assert_eq!(restored.amount, 30_000);
    assert_eq!(restored.account_id, bank);
    assert_eq!(restored.counter_account_id, Some(cash));
    assert!(restored.category_id.is_none());

    // 2) Aggregate balance survives.
    let dst_balances = balance_repo::list_balances(&dst).unwrap();
    let dst_total: i64 = dst_balances.iter().map(|b| b.balance).sum();
    assert_eq!(src_total, dst_total);
}
```

**Important:** This test calls `import_snapshot_json` as if it were a public helper. The current `backup.rs` exposes `export_snapshot_json` but the import path is wired through the `#[tauri::command] import_json` handler. If `import_snapshot_json` is not pub, split the existing handler so the body is a public free function `pub fn import_snapshot_json(conn: &mut Connection, payload: &str, mode: &str) -> AppResult<ImportResult>` and have `import_json` call it. Mirror what was done with `export_snapshot_json`. Commit that refactor as its own step:

```bash
git add src-tauri/src/commands/backup.rs
git commit -m "refactor(backup): expose import_snapshot_json so integration tests can call it"
```

- [ ] **Step 2: Run the integration tests**

```bash
cd src-tauri
cargo test --test integration_backup
```

Expected: PASS (existing tests + new `export_then_overwrite_import_preserves_transfer_row`).

- [ ] **Step 3: Commit**

```bash
git add src-tauri/tests/integration_backup.rs
git commit -m "test(backup): JSON roundtrip preserves transfer row and total balance"
```

---

### Task 2: Playwright E2E `transfer-flow.spec.ts`

**Files:**
- Create: `tests/e2e/transfer-flow.spec.ts`

- [ ] **Step 1: Write the spec**

Create `tests/e2e/transfer-flow.spec.ts`. Mirror the in-page `__TAURI_INTERNALS__` mocking pattern from the existing `tests/e2e/transaction-flow.spec.ts` (which the agent should read for reference). The new spec stubs `list_balances`, `create_transfer`, and `update_transfer` in addition to the Phase 2 commands.

```ts
import { expect, test } from '@playwright/test';

const seededAccounts = [
  {
    id: 1,
    name: '現金',
    kind: 'cash',
    currency: 'JPY',
    initial_balance: 50_000,
    display_order: 0,
    note: '',
    archived_at: null,
    created_at: '2026-05-25T00:00:00Z',
    updated_at: '2026-05-25T00:00:00Z',
  },
  {
    id: 2,
    name: '銀行',
    kind: 'bank',
    currency: 'JPY',
    initial_balance: 200_000,
    display_order: 1,
    note: '',
    archived_at: null,
    created_at: '2026-05-25T00:00:00Z',
    updated_at: '2026-05-25T00:00:00Z',
  },
];

test('transfer moves money between accounts without changing total assets', async ({ page }) => {
  await page.addInitScript((accounts) => {
    type ListenerPayload = { event: string; id: number; payload: { domain: string } };
    type Tx = {
      id: number;
      occurred_on: string;
      type: 'income' | 'expense' | 'transfer';
      amount: number;
      account_id: number;
      counter_account_id: number | null;
      category_id: number | null;
      description: string;
      recurring_id: null;
      created_at: string;
      updated_at: string;
    };

    const state = {
      categories: [] as any[],
      accounts: structuredClone(accounts),
      transactions: [] as Tx[],
      nextId: 100,
    };
    (window as any).__TEST_STATE__ = state;

    let nextCallbackId = 1;
    const callbacks = new Map<number, (payload: ListenerPayload) => void>();
    const listeners = new Map<string, number[]>();
    function emitChanged(domain: string) {
      for (const id of listeners.get('data:changed') ?? []) {
        callbacks.get(id)?.({ event: 'data:changed', id, payload: { domain } });
      }
    }

    (window as any).__TAURI_EVENT_PLUGIN_INTERNALS__ = {
      unregisterListener: (event: string, id: number) => {
        listeners.set(event, (listeners.get(event) ?? []).filter((x) => x !== id));
        callbacks.delete(id);
      },
    };

    function balances() {
      return state.accounts.map((a: any) => {
        let bal = a.initial_balance;
        for (const t of state.transactions) {
          if (t.type === 'income' && t.account_id === a.id) bal += t.amount;
          else if (t.type === 'expense' && t.account_id === a.id) bal -= t.amount;
          else if (t.type === 'transfer' && t.account_id === a.id) bal -= t.amount;
          else if (t.type === 'transfer' && t.counter_account_id === a.id) bal += t.amount;
        }
        return {
          account_id: a.id,
          name: a.name,
          kind: a.kind,
          initial_balance: a.initial_balance,
          balance: bal,
          archived_at: a.archived_at,
          display_order: a.display_order,
        };
      });
    }

    (window as any).__TAURI_INTERNALS__ = {
      transformCallback: (cb: (p: ListenerPayload) => void) => {
        const id = nextCallbackId++;
        callbacks.set(id, cb);
        return id;
      },
      unregisterCallback: (id: number) => callbacks.delete(id),
      invoke: async (command: string, args: any) => {
        if (command === 'plugin:event|listen') {
          listeners.set(args.event, [...(listeners.get(args.event) ?? []), args.handler]);
          return args.handler;
        }
        if (command === 'plugin:event|unlisten') {
          (window as any).__TAURI_EVENT_PLUGIN_INTERNALS__.unregisterListener(
            args.event,
            args.eventId,
          );
          return null;
        }
        switch (command) {
          case 'app_info':
            return { schema_version: 2, db_path: '/tmp/data.db' };
          case 'list_categories':
            return state.categories;
          case 'list_accounts':
            return state.accounts;
          case 'list_transactions': {
            const sorted = [...state.transactions].sort((a, b) => b.id - a.id);
            return {
              items: sorted.slice(args.page * args.pageSize, (args.page + 1) * args.pageSize),
              total: sorted.length,
            };
          }
          case 'list_balances':
            return balances();
          case 'monthly_summary': {
            const inc = state.transactions
              .filter((t) => t.type === 'income')
              .reduce((s, t) => s + t.amount, 0);
            const exp = state.transactions
              .filter((t) => t.type === 'expense')
              .reduce((s, t) => s + t.amount, 0);
            return { income: inc, expense: exp, net: inc - exp, by_category: [] };
          }
          case 'monthly_series':
            return Array.from({ length: args.months }, (_, i) => ({
              year_month: `2026-${String(i + 1).padStart(2, '0')}`,
              income: 0,
              expense: 0,
            }));
          case 'create_transfer': {
            const tx: Tx = {
              id: state.nextId++,
              occurred_on: args.input.occurred_on,
              type: 'transfer',
              amount: args.input.amount,
              account_id: args.input.account_id,
              counter_account_id: args.input.counter_account_id,
              category_id: null,
              description: args.input.description ?? '',
              recurring_id: null,
              created_at: '2026-05-25T00:00:00Z',
              updated_at: '2026-05-25T00:00:00Z',
            };
            state.transactions.push(tx);
            emitChanged('transactions');
            return tx;
          }
          case 'get_db_path':
            return '/tmp/data.db';
          case 'get_last_backup_at':
            return null;
          default:
            return null;
        }
      },
    };
  }, seededAccounts);

  await page.goto('/');

  // Total before any transactions = 50,000 + 200,000 = 250,000
  await expect(page.getByTestId('card-total-assets')).toContainText('250,000');
  await expect(page.getByTestId('balance-1')).toContainText('50,000');
  await expect(page.getByTestId('balance-2')).toContainText('200,000');

  // Create a transfer 30,000 from 銀行 (id=2) -> 現金 (id=1).
  await page.getByTestId('nav-transactions').click();
  await page.getByRole('button', { name: '+ 取引を追加' }).click();
  await page.getByTestId('tx-type').selectOption('transfer');
  await page.getByTestId('tx-amount').fill('30000');
  await page.getByTestId('tx-account').selectOption('2');           // source = 銀行
  await page.getByTestId('tx-counter-account').selectOption('1');   // dest   = 現金
  await page.getByTestId('tx-description').fill('ATM入金');
  await page.getByRole('dialog').getByRole('button', { name: '追加', exact: true }).click();

  await expect
    .poll(() => page.evaluate(() => (window as any).__TEST_STATE__.transactions.length))
    .toBe(1);

  // Transfer row renders as `銀行 → 現金` in the table.
  await expect(page.getByTestId('tx-row-transfer')).toContainText('銀行 → 現金');

  // Back on the Dashboard: total unchanged at 250,000, per-account moved.
  await page.getByTestId('nav-dashboard').click();
  await expect(page.getByTestId('card-total-assets')).toContainText('250,000');
  await expect(page.getByTestId('balance-1')).toContainText('80,000');  // 50,000 + 30,000
  await expect(page.getByTestId('balance-2')).toContainText('170,000'); // 200,000 - 30,000
});
```

- [ ] **Step 2: Run E2E**

```bash
pnpm test:e2e tests/e2e/transfer-flow.spec.ts
```

Expected: PASS. If the smoke spec fails because the `nav-*` testids changed, fix the spec (we did not change those testids).

- [ ] **Step 3: Commit**

```bash
git add tests/e2e/transfer-flow.spec.ts
git commit -m "test(e2e): transfer flow preserves total assets and moves per-account"
```

---

### Task 3: Phase 3 final Definition of Done

**Files:**
- (read-only verification)

- [ ] **Step 1: Run the full local quality gate**

From the repo root:

```bash
cd src-tauri && cargo clippy --all-targets -- -D warnings && cargo test
cd .. && pnpm test && pnpm check
pnpm test:e2e
```

Expected: ALL GREEN. If anything fails:
1. Read the failure.
2. Identify the slice it belongs to.
3. Loop back to that slice and fix.

Do not paper over a failure with `--no-verify` or test-skipping.

- [ ] **Step 2: Manual smoke against `pnpm tauri dev`**

Run `pnpm tauri dev` and exercise the canonical Phase 3 path:

1. Create accounts `現金` (initial ¥50,000) and `銀行` (initial ¥200,000) on the Accounts page if not already present.
2. Dashboard shows 総資産 = ¥250,000.
3. Add expense ¥3,000 (Food) from 現金 — Dashboard total drops to ¥247,000.
4. Add income ¥300,000 (Salary) to 銀行 — Dashboard total = ¥547,000.
5. Add transfer ¥30,000 銀行 → 現金 — Dashboard total still ¥547,000; 現金 row shows ¥77,000 (50,000 - 3,000 + 30,000); 銀行 row shows ¥470,000 (200,000 + 300,000 - 30,000).
6. Edit the transfer to ¥50,000 — Dashboard total still ¥547,000; per-account values move.
7. Delete the transfer — Dashboard total ¥547,000 unchanged; per-account returns to step-4 values.
8. Settings → Export JSON; open the file and confirm a `"type":"transfer"` row exists.
9. Settings → Import JSON (overwrite) with the file from step 8 — all data reappears, balances unchanged.

Stop the dev server.

- [ ] **Step 3: Confirm Phase 3 DoD checklist (also in `phase3-00-overview.md`)**

- [ ] `cargo clippy --all-targets -- -D warnings` green.
- [ ] `cargo test` green.
- [ ] `pnpm test` green.
- [ ] `pnpm check` green.
- [ ] `pnpm test:e2e` green including `transfer-flow.spec.ts`.
- [ ] Manual smoke (Step 2 of this task) passes end-to-end.
- [ ] Backup JSON roundtrip preserves transfers and balances.
- [ ] `pnpm tauri build` succeeds on the available platform(s).

- [ ] **Step 4: Update memory + CLAUDE.md status**

In `CLAUDE.md`, find the `## 現在の状態` section and update Phase status from "Phase 2 完了" to "Phase 3 完了 (複数口座 + 振替 + 残高表示)". Edit the next-phase pointer to Phase 4 (予算管理).

```bash
git add CLAUDE.md
git commit -m "docs(claude-md): mark Phase 3 complete, point to Phase 4 budgets"
```

- [ ] **Step 5: Final cleanup**

If any of the manual smoke steps surfaced regressions in Categories / Settings / income+expense flows, file them as separate fix tasks. Do NOT bundle unrelated fixes into Phase 3.

---

### Slice 06 DoD

- [ ] `integration_backup.rs::export_then_overwrite_import_preserves_transfer_row` passes.
- [ ] `tests/e2e/transfer-flow.spec.ts` passes.
- [ ] All checks in Task 3 Step 1 are green.
- [ ] Manual smoke (Task 3 Step 2) passes.
- [ ] CLAUDE.md reflects Phase 3 completion.
