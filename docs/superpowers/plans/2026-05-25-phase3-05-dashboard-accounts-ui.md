# Phase 3 — Slice 05: Dashboard total assets + Accounts page balances

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this slice task-by-task.

**Goal:** Surface the new `balances` store on two pages:
- **Dashboard** gets a "総資産" (total assets) card at the top, plus a per-account balance breakdown card next to the existing recent-transactions card.
- **Accounts** page now shows each account's **current balance** (from the balances store) instead of the static `initial_balance`. Archived accounts keep showing their initial balance only.

**Prerequisite:** Slice 03 complete (`createBalancesStore`, `lib/api/balances.ts` available).

**Spec:** sections 3.2, 5.1, 5.5 of `docs/superpowers/specs/2026-05-24-budget-tracker-design.md`.

**Phase 3 UI invariants this slice adds:**
- "総資産" = sum of `balance` over **non-archived** accounts. Archived accounts are excluded so a long-closed credit card does not skew the total.
- The per-account breakdown card on the Dashboard lists at most 8 accounts; if more exist, append a "他 N 件" row.
- The Accounts page shows the current balance to the right of the account name and the initial balance as a smaller secondary line (only when initial != current).
- When the balances store has `error !== null`, show the error inline in the relevant card; do not silently hide accounts.

---

### Task 1: Dashboard total-assets card + per-account breakdown

**Files:**
- Modify: `src/routes/Dashboard.svelte`

- [ ] **Step 1: Replace the `<script>` block**

Open `src/routes/Dashboard.svelte`. Replace the import section and the existing reactive state with the version below (do not touch the chart-related code; only add balances integration and the new card).

After the existing import lines, add:

```ts
  import { createBalancesStore } from '../lib/stores/balances.svelte';
```

Then, immediately after the existing `const catStore = createCategoriesStore({ include_archived: true });` line, add:

```ts
  const balancesStore = createBalancesStore();
```

And in the existing `onDestroy(...)` block, add `void balancesStore.dispose();` next to the existing `void catStore.dispose();` line. (If you used the alternative pattern where the Dashboard does not call `dispose` for `catStore`, add the same pattern for `balancesStore`.)

- [ ] **Step 2: Add the "総資産" summary card and per-account breakdown card to the template**

Locate the existing `<div class="summary-grid"> ... </div>` and the `<div class="dashboard-grid"> ... </div>`.

Insert a new top-level card ABOVE the existing summary-grid (so the layout becomes: total-assets card → 3-card month grid → dashboard-grid):

```svelte
  <Card>
    {#snippet children()}
      <div class="assets-card">
        <div class="assets-head">
          <small>総資産</small>
          <strong data-testid="card-total-assets">{yen.format(balancesStore.totalAssets)}</strong>
          {#if balancesStore.error}
            <small class="error">エラー: {balancesStore.error}</small>
          {/if}
        </div>
        {#if balancesStore.items.length === 0 && !balancesStore.loading}
          <EmptyState title="口座がありません" hint="口座ページから追加してください" />
        {:else}
          <ul class="assets-list" data-testid="assets-list">
            {#each balancesStore.items.filter((b) => b.archived_at == null).slice(0, 8) as account (account.account_id)}
              <li>
                <span class="acct-name">{account.name}</span>
                <span class="acct-balance" data-testid={`balance-${account.account_id}`}>
                  {yen.format(account.balance)}
                </span>
              </li>
            {/each}
            {#if balancesStore.items.filter((b) => b.archived_at == null).length > 8}
              <li class="more">
                他 {balancesStore.items.filter((b) => b.archived_at == null).length - 8} 件
              </li>
            {/if}
          </ul>
        {/if}
      </div>
    {/snippet}
  </Card>
```

- [ ] **Step 3: Add CSS for the assets card**

Append to the existing `<style>` block:

```css
  .assets-card {
    display: grid;
    gap: var(--space-4);
  }
  .assets-head {
    display: flex;
    align-items: baseline;
    gap: var(--space-3);
    flex-wrap: wrap;
  }
  .assets-head strong {
    font-size: 2rem;
    margin: 0;
  }
  .assets-list {
    list-style: none;
    padding: 0;
    margin: 0;
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
    gap: var(--space-2) var(--space-4);
  }
  .assets-list li {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    border-bottom: 1px dashed var(--border);
    padding: var(--space-2) 0;
  }
  .assets-list li.more {
    color: var(--muted);
    justify-content: center;
    grid-column: 1 / -1;
  }
  .acct-name {
    color: var(--muted);
  }
  .acct-balance {
    font-weight: 700;
    font-variant-numeric: tabular-nums;
  }
```

- [ ] **Step 4: svelte-check + manual smoke**

```bash
pnpm check
pnpm tauri dev
```

In the dev window, verify on the Dashboard:
- The "総資産" card appears above the income/expense/net cards.
- Adding a transfer between two accounts does NOT change the displayed total (only per-account values swap).
- Adding an expense reduces the total by the expense amount.

Stop the dev server.

- [ ] **Step 5: Commit**

```bash
git add src/routes/Dashboard.svelte
git commit -m "feat(dashboard): total-assets card + per-account balance breakdown"
```

---

### Task 2: Accounts page shows current balance

**Files:**
- Modify: `src/routes/Accounts.svelte`

- [ ] **Step 1: Add the balances store to the script block**

Open `src/routes/Accounts.svelte`. After the existing `const store = createAccountsStore(true);` line, add:

```ts
  import { createBalancesStore } from '../lib/stores/balances.svelte';
  const balances = createBalancesStore();
```

In `onDestroy`, append:

```ts
    void balances.dispose();
```

- [ ] **Step 2: Add a derived map from account id to current balance**

In the same `<script>` block, near the other `$derived` lines, add:

```ts
  const balanceById = $derived(
    new Map(balances.items.map((row) => [row.account_id, row.balance])),
  );
```

- [ ] **Step 3: Replace the balance display in the active accounts list**

In the existing list of non-archived accounts (`{#each visible as account ...}`), find:

```svelte
              <span class="balance">{yen.format(account.initial_balance)}</span>
```

Replace with:

```svelte
              <span class="balance" data-testid={`account-balance-${account.id}`}>
                <strong>{yen.format(balanceById.get(account.id) ?? account.initial_balance)}</strong>
                {#if (balanceById.get(account.id) ?? account.initial_balance) !== account.initial_balance}
                  <small>初期 {yen.format(account.initial_balance)}</small>
                {/if}
              </span>
```

Leave the archived-accounts list unchanged — it keeps showing `initial_balance` only (archived accounts are intentionally excluded from the balances store's display).

- [ ] **Step 4: Adjust CSS so balance can stack the secondary line**

In the existing `<style>` block, replace:

```css
  .balance {
    font-weight: 700;
    font-variant-numeric: tabular-nums;
  }
```

with:

```css
  .balance {
    font-variant-numeric: tabular-nums;
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    line-height: 1.2;
  }
  .balance strong {
    font-weight: 700;
  }
  .balance small {
    color: var(--muted);
  }
```

- [ ] **Step 5: svelte-check + manual smoke**

```bash
pnpm check
pnpm tauri dev
```

Verify on the Accounts page:
- After adding a transaction, the balance updates without manual refresh (the balances store reloads on `data:changed`).
- Accounts whose current balance equals their initial balance show only the strong line.
- Accounts whose balance has changed show the smaller "初期 ¥X" line underneath.
- Archived accounts (toggle by archiving one) still show their initial balance only.

Stop the dev server.

- [ ] **Step 6: Commit**

```bash
git add src/routes/Accounts.svelte
git commit -m "feat(accounts): show current balance with secondary initial line"
```

---

### Slice 05 DoD

- [ ] `pnpm check` green.
- [ ] Dashboard renders the "総資産" card with sum-over-non-archived; transfers preserve the total while shifting per-account.
- [ ] Accounts page shows current balance (driven by the balances store) for non-archived accounts; archived rows still show `initial_balance`.
- [ ] Both pages react to `data:changed` (no page reload required after creating / editing / deleting a transaction).
- [ ] No regressions on Categories, Settings, or Transactions pages.
