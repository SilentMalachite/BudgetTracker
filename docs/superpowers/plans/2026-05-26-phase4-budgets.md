# Phase 4 Budgets Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement Phase 4 budget management: monthly category budgets, progress status, warning/over-budget badges, and a Dashboard budget widget.

**Architecture:** Follow the existing vertical-slice pattern from Phase 2/3: Rust domain validation and budget evaluation first, SQLite repo queries second, thin Tauri commands third, then typed TypeScript API/store and Svelte route UI. Svelte displays Rust results only; spending totals, projection, thresholds, and alert decisions stay in Rust.

**Tech Stack:** Tauri 2.x, Rust + rusqlite (`bundled-sqlcipher-vendored-openssl`), Svelte 5 (Runes) + TypeScript, Vite, Chart.js, Vitest, Playwright, `proptest`.

---

## Scope

Phase 4 is limited to the canonical spec's "予算管理" scope:

- Monthly budgets for expense categories.
- Budget progress list for the selected month.
- Warning / over-budget UI badges.
- Dashboard Top 3 budget progress widget for the current month.
- Backup/import and docs updates needed so budgets are not second-class data.

Out of scope for Phase 4:

- Recurring transactions and startup expansion. Keep this for Phase 5.
- Advanced reports and report tabs. Keep this for Phase 5.
- Excel import/export. Keep this out of Phase 4 unless a later spec explicitly moves it forward.
- Yearly budget UI. The DB supports `period='yearly'`, but Phase 4 UI/API only writes `monthly` budgets.

## File Structure

### Create

- `src-tauri/src/domain/budget.rs` - pure budget input validation and `BudgetStatus` evaluation.
- `src-tauri/src/infra/repo/budget_repo.rs` - SQL access for budgets and category/month spending.
- `src-tauri/src/commands/budgets.rs` - Tauri commands: `list_budget_statuses`, `set_budget`.
- `src-tauri/migrations/V003__budget_lookup_indexes.sql` - query indexes for budget status lookup.
- `src-tauri/tests/integration_budgets.rs` - repo/command-level budget behavior.
- `src/lib/api/budgets.ts` - typed invoke wrappers.
- `src/lib/api/budgets.test.ts` - Vitest API wrapper coverage.
- `src/lib/stores/budgets.svelte.ts` - Svelte 5 Runes store for selected month budget status.
- `src/lib/stores/budgets.test.ts` - store reload/event behavior.
- `src/routes/Budgets.svelte` - budget management page.
- `tests/e2e/budget-flow.spec.ts` - budget setup and warning badge flow.

### Modify

- `src-tauri/src/domain/mod.rs` - expose `budget`.
- `src-tauri/src/infra/repo/mod.rs` - expose `budget_repo`.
- `src-tauri/src/commands/mod.rs` - expose `budgets`.
- `src-tauri/src/lib.rs` - register budget commands.
- `src-tauri/src/infra/events.rs` - add `ChangedDomain::Budgets`.
- `src-tauri/src/infra/migrations.rs` - assert V003 index exists in migration tests.
- `src-tauri/src/commands/backup.rs` - include budget count in `ImportResult`; emit `budgets` changed on import.
- `src-tauri/tests/integration_backup.rs` - assert budget export/import count and row preservation.
- `src/lib/api/events.ts` - add `budgets` to `ChangedDomain`.
- `src/lib/api/index.ts` - export budgets API.
- `src/lib/api/backup.ts` - add `budgets` count to import result type.
- `src/App.svelte` - add `/budgets` route.
- `src/lib/components/Sidebar.svelte` - add "予算" nav item.
- `src/routes/Dashboard.svelte` - add current-month Top 3 budget widget.
- `src/routes/Settings.svelte` - show imported budget count in import result message.
- `README.md` - update roadmap/status wording so Phase 4 is budgets only.
- `AGENTS.md` / `CLAUDE.md` - update current phase pointer after Phase 4 is complete.

## Interfaces

### Rust command payloads

Use snake_case field names to match the existing Tauri payload style.

```rust
#[derive(Debug, serde::Deserialize)]
pub struct SetBudgetInput {
    pub category_id: i64,
    pub year_month: String,       // YYYY-MM
    pub amount: i64,              // integer yen, >= 0
    pub alert_threshold: i64,     // 0..=200
}

#[derive(Debug, serde::Serialize)]
pub struct Budget {
    pub id: i64,
    pub category_id: i64,
    pub period: String,           // always "monthly" in Phase 4 commands
    pub amount: i64,
    pub starts_on: String,        // YYYY-MM-01
    pub ends_on: Option<String>,
    pub alert_threshold: i64,
}

#[derive(Debug, serde::Serialize)]
pub struct BudgetStatus {
    pub category_id: i64,
    pub category_name: String,
    pub category_color: Option<String>,
    pub category_icon: Option<String>,
    pub budget_id: Option<i64>,
    pub budgeted: i64,
    pub spent: i64,
    pub percent: i64,
    pub progress_percent: i64,
    pub days_left: i64,
    pub projected: i64,
    pub alert_threshold: i64,
    pub threshold_reached: bool,
    pub projected_over_budget: bool,
}
```

### Commands

- `list_budget_statuses(yearMonth: string): Promise<BudgetStatus[]>`
  - Validate `YYYY-MM`.
  - Return active expense categories only.
  - Include categories with no budget as `budgeted=0`, `budget_id=null`, `alert_threshold=80`.
  - Spend only counts `transactions.type='expense'` inside the selected month.
  - Income and transfer rows must never affect budget progress.

- `set_budget(input: SetBudgetInput): Promise<Budget>`
  - Validate category exists, is `expense`, and is not archived.
  - Validate `amount >= 0`.
  - Validate `alert_threshold` is `0..=200`.
  - Store as `period='monthly'`, `starts_on='{YYYY-MM}-01'`, `ends_on=NULL`.
  - Upsert by existing `UNIQUE(category_id, starts_on)`.

### Budget calculation rules

- `percent = floor(spent * 100 / budgeted)` for `budgeted > 0`.
- `budgeted == 0 && spent == 0` => `percent = 0`.
- `budgeted == 0 && spent > 0` => `percent = 200`.
- `progress_percent = min(percent, 100)`.
- `days_left` is based on the selected month and "today"; for past months use `0`, for future months use full month days, for current month count remaining calendar days including today.
- `projected` extends the current daily spending pace to month end. For past months use `spent`; for future months with no elapsed days use `0`.
- `threshold_reached = budgeted > 0 && percent >= alert_threshold`.
- `projected_over_budget = budgeted > 0 && projected > budgeted`.

## Task 1: Rust Budget Domain

**Files:**
- Create: `src-tauri/src/domain/budget.rs`
- Modify: `src-tauri/src/domain/mod.rs`

- [ ] Write failing unit tests for `parse_year_month`, `month_bounds`, and `validate_set_budget_input`.
- [ ] Run `cd src-tauri && cargo test domain::budget -- --nocapture`; expected: fail because module/functions do not exist.
- [ ] Implement `YearMonth`, input validation, and date helpers using `chrono::NaiveDate`.
- [ ] Write failing tests for status calculation: normal spending, zero budget, threshold reached, projected over budget, past/future month behavior.
- [ ] Add `proptest` coverage for large integer values and assert no panic, no negative `progress_percent`, and `progress_percent <= 100`.
- [ ] Implement `evaluate_status` with integer-only yen arithmetic and saturating operations where needed.
- [ ] Run `cd src-tauri && cargo test domain::budget`; expected: pass.
- [ ] Commit: `feat(budgets): add budget domain evaluation`.

## Task 2: Budget Repository and Migration

**Files:**
- Create: `src-tauri/src/infra/repo/budget_repo.rs`
- Create: `src-tauri/migrations/V003__budget_lookup_indexes.sql`
- Create: `src-tauri/tests/integration_budgets.rs`
- Modify: `src-tauri/src/infra/repo/mod.rs`
- Modify: `src-tauri/src/infra/migrations.rs`

- [ ] Write failing integration tests for listing budget statuses with active expense categories, no budget rows, and expense-only spending totals.
- [ ] Write failing integration tests that transfer/income rows do not affect `spent`.
- [ ] Write failing integration tests for `set_budget` insert then update on the same category/month.
- [ ] Run `cd src-tauri && cargo test --test integration_budgets`; expected: fail because repo does not exist.
- [ ] Add V003 indexes:

```sql
CREATE INDEX IF NOT EXISTS idx_budgets_category_starts
  ON budgets(category_id, starts_on);

CREATE INDEX IF NOT EXISTS idx_tx_budget_month_category
  ON transactions(type, occurred_on, category_id);
```

- [ ] Implement `budget_repo::set_monthly_budget`.
- [ ] Implement `budget_repo::list_status_inputs(conn, year_month)` or equivalent query that returns category + budget + spent inputs.
- [ ] Feed repo rows into `domain::budget::evaluate_status`.
- [ ] Update migration tests to assert both V003 indexes exist and schema version is at least 3.
- [ ] Run `cd src-tauri && cargo test --test integration_budgets`; expected: pass.
- [ ] Run `cd src-tauri && cargo test infra::migrations`; expected: pass.
- [ ] Commit: `feat(budgets): persist monthly budgets`.

## Task 3: Tauri Commands and Events

**Files:**
- Create: `src-tauri/src/commands/budgets.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/infra/events.rs`
- Test: `src-tauri/tests/integration_budgets.rs`

- [ ] Write command-level tests or integration coverage for invalid month, income category rejection, archived category rejection, and successful upsert.
- [ ] Run `cd src-tauri && cargo test --test integration_budgets`; expected: fail on missing commands or validation.
- [ ] Implement `list_budget_statuses(state, year_month)`.
- [ ] Implement `set_budget(app, state, input)` with `TransactionBehavior::Immediate`.
- [ ] Emit `ChangedDomain::Budgets` after a successful write.
- [ ] Register commands in `tauri::generate_handler!`.
- [ ] Run `cd src-tauri && cargo test --test integration_budgets`; expected: pass.
- [ ] Run `cd src-tauri && cargo clippy --all-targets -- -D warnings`; expected: pass.
- [ ] Commit: `feat(budgets): expose budget commands`.

## Task 4: Frontend API and Store

**Files:**
- Create: `src/lib/api/budgets.ts`
- Create: `src/lib/api/budgets.test.ts`
- Create: `src/lib/stores/budgets.svelte.ts`
- Create: `src/lib/stores/budgets.test.ts`
- Modify: `src/lib/api/events.ts`
- Modify: `src/lib/api/index.ts`

- [ ] Write Vitest tests that `listBudgetStatuses('2026-05')` invokes `list_budget_statuses` with `{ yearMonth: '2026-05' }`.
- [ ] Write Vitest tests that `setBudget(input)` wraps payload under `input`.
- [ ] Write store tests for initial load, selected month change, and reload on `budgets`, `categories`, and `transactions` events.
- [ ] Run `pnpm test src/lib/api/budgets.test.ts src/lib/stores/budgets.test.ts`; expected: fail because files do not exist.
- [ ] Implement TypeScript types mirroring Rust structs.
- [ ] Implement `createBudgetsStore(initialYearMonth)` with `items`, `loading`, `error`, `yearMonth`, `setYearMonth`, `load`, and `dispose`.
- [ ] Update event type union to include `'budgets'`.
- [ ] Run `pnpm test src/lib/api/budgets.test.ts src/lib/stores/budgets.test.ts`; expected: pass.
- [ ] Commit: `feat(budgets): add frontend budget api store`.

## Task 5: Budgets Page UI

**Files:**
- Create: `src/routes/Budgets.svelte`
- Modify: `src/App.svelte`
- Modify: `src/lib/components/Sidebar.svelte`

- [ ] Add `/budgets` route and Sidebar item labeled `予算`.
- [ ] Build the Budgets page with:
  - Month selector with previous/next controls.
  - List of active expense categories.
  - Budget/spent/progress display.
  - Warning badge when `threshold_reached`.
  - Projected-over-budget badge when `projected_over_budget`.
  - Modal for setting amount and threshold.
- [ ] Keep UI text Japanese and use existing `Card`, `Button`, `Modal`, `TextField`, `Select`, and `CategoryBadge` where they fit.
- [ ] Validate frontend form inputs only for usability: integer yen and integer threshold. Treat Rust validation as authoritative.
- [ ] Use `data-testid` values:
  - `page-budgets`
  - `nav-budgets`
  - `budget-month`
  - `budget-row-{category_id}`
  - `budget-progress-{category_id}`
  - `budget-alert-{category_id}`
  - `budget-edit-{category_id}`
  - `budget-amount`
  - `budget-threshold`
- [ ] Run `pnpm check`; expected: pass.
- [ ] Commit: `feat(budgets): add budget management page`.

## Task 6: Dashboard Budget Widget

**Files:**
- Modify: `src/routes/Dashboard.svelte`

- [ ] Load current-month budget statuses using the budgets API/store.
- [ ] Add a Top 3 budget widget near the existing summary/dashboard content.
- [ ] Sort statuses by `percent` descending, then `projected_over_budget`, then category name.
- [ ] Show category name, `spent / budgeted`, progress bar, and alert/projected badges.
- [ ] For no budgets/categories, show an `EmptyState` that points users to the budget page.
- [ ] Ensure Dashboard still reloads on `transactions`, `categories`, `accounts`, and now `budgets` changes.
- [ ] Run `pnpm check`; expected: pass.
- [ ] Commit: `feat(dashboard): show budget progress widget`.

## Task 7: Backup and Settings Follow-Up

**Files:**
- Modify: `src-tauri/src/commands/backup.rs`
- Modify: `src-tauri/tests/integration_backup.rs`
- Modify: `src/lib/api/backup.ts`
- Modify: `src/routes/Settings.svelte`

- [ ] Add failing backup integration test that export/import preserves one budget and reports `result.budgets == 1`.
- [ ] Run `cd src-tauri && cargo test --test integration_backup`; expected: fail because `ImportResult` has no `budgets`.
- [ ] Add `budgets: u32` to Rust `ImportResult`.
- [ ] Count inserted budgets in both overwrite and append imports.
- [ ] Emit `ChangedDomain::Budgets` after import.
- [ ] Add `budgets` to TypeScript `ImportResult`.
- [ ] Update Settings import result text to include `予算`.
- [ ] Run `cd src-tauri && cargo test --test integration_backup`; expected: pass.
- [ ] Run `pnpm check`; expected: pass.
- [ ] Commit: `feat(backup): report imported budgets`.

## Task 8: E2E and Documentation

**Files:**
- Create: `tests/e2e/budget-flow.spec.ts`
- Modify: `README.md`
- Modify: `AGENTS.md`
- Modify: `CLAUDE.md`

- [ ] Write Playwright test with mocked Tauri internals:
  - Seed one expense category and one account.
  - Navigate to `予算`.
  - Set category budget to `50000` and threshold to `80`.
  - Add an expense transaction of `45000`.
  - Assert budget row shows progress around `90%` and warning badge.
  - Return to Dashboard and assert the budget widget shows the same category.
- [ ] Run `pnpm test:e2e tests/e2e/budget-flow.spec.ts`; expected: fail until UI/data mocks are complete.
- [ ] Complete mocks for `list_budget_statuses` and `set_budget`; make the test pass.
- [ ] Update README roadmap/status:
  - Phase 1, 2, 3 complete if already true in repo history.
  - Phase 4 described as `予算管理`.
  - Phase 5 described as `定期取引 + 分析レポート強化`.
- [ ] Update `AGENTS.md` and `CLAUDE.md` current-state pointers after implementation is verified.
- [ ] Run `pnpm test:e2e`; expected: pass.
- [ ] Commit: `test(budgets): cover budget warning flow`.

## Final Verification

- [ ] `cd src-tauri && cargo clippy --all-targets -- -D warnings`
- [ ] `cd src-tauri && cargo test`
- [ ] `pnpm test`
- [ ] `pnpm check`
- [ ] `pnpm test:e2e`
- [ ] `pnpm tauri build --target universal-apple-darwin`
- [ ] Windows verification: `pnpm tauri build --target x86_64-pc-windows-msvc` on Windows runner or machine.

## Acceptance Criteria

- A user can open `予算`, select a month, and set a monthly budget for an expense category.
- Budget progress is calculated from expense transactions only.
- Transfer and income transactions do not affect budget spending.
- Warning badge appears when `percent >= alert_threshold`.
- Dashboard shows the current month Top 3 budget progress.
- JSON backup/export/import preserves budgets and reports imported budget count.
- No new dependencies are added.
- All money remains integer yen end-to-end.
- Phase 4 does not implement recurring transactions, advanced reports, or Excel export.

## Assumptions

- `docs/superpowers/specs/2026-05-24-budget-tracker-design.md` is the source of truth.
- The README line that describes Phase 4 as `予算 + 定期取引 + アラート(UI バッジ)` is stale; treat recurring transactions as Phase 5 work.
- Existing migration files must not be edited. Add V003 only.
- Existing category and account names remain user-defined. Do not add hardcoded budget category presets.
- Existing physical deletes for transaction rows are outside this Phase 4 scope; do not refactor unrelated delete behavior in this phase.
