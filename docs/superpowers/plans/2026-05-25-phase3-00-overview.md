# Phase 3 Implementation Plan — Overview

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement each slice task-by-task. This document is the INDEX; the actual tasks live in the six slice files listed in the next section.

**Goal:** Deliver the Phase 3 scope from spec section 10 of `docs/superpowers/specs/2026-05-24-budget-tracker-design.md` — transfer transactions between accounts, and per-account current-balance display (total assets card on the Dashboard, balance column on the Accounts page) — as a working Tauri app on top of the Phase 2 income/expense CRUD foundation.

**Architecture:** Vertical slices, same pattern as Phase 2 (Rust-first per slice: `domain/` validator → `infra/repo/` SQL → `commands/` Tauri command → frontend `lib/api/` wrapper → `lib/stores/` reactive store → route page). Transfers get their own dedicated commands (`create_transfer` / `update_transfer`) so that the existing income/expense validator stays clean and `counter_account_id` can be modeled as required at the API boundary. Balances are computed in two layers: a pure `domain/balance.rs` function that proptest can exercise, and a `infra/repo/balance_repo.rs` SQL query that returns one row per account in a single `LEFT JOIN` + `GROUP BY` pass.

**Tech Stack:** Tauri 2.x, Rust + rusqlite (`bundled-sqlcipher-vendored-openssl`), Svelte 5 (Runes) + TypeScript, Vite, Chart.js, svelte-spa-router, Vitest, Playwright, `proptest`.

## Slices (execute in order)

| # | File | Scope | Depends on |
|---|------|-------|------------|
| 01 | [`2026-05-25-phase3-01-foundation.md`](./2026-05-25-phase3-01-foundation.md) | V002 migration (counter-account index), `validate_transfer_input` in `domain/ledger.rs`, ChangedDomain audit | — |
| 02 | [`2026-05-25-phase3-02-transfer-commands.md`](./2026-05-25-phase3-02-transfer-commands.md) | `transaction_repo::insert_transfer` / `update_transfer`, transfer-aware list filter (include in `'all'` view), `create_transfer` / `update_transfer` Tauri commands, integration test | 01 |
| 03 | [`2026-05-25-phase3-03-balances.md`](./2026-05-25-phase3-03-balances.md) | `domain/balance.rs` (pure `compute_balance` + proptest), `infra/repo/balance_repo.rs` (single-pass SQL), `commands/balances::list_balances`, frontend `lib/api/balances.ts` + Vitest, `lib/stores/balances.svelte.ts` | 02 |
| 04 | [`2026-05-25-phase3-04-transfer-ui.md`](./2026-05-25-phase3-04-transfer-ui.md) | Transactions page: type selector now includes `transfer`; modal switches to source/destination account fields; transfer rows become editable/deletable | 03 |
| 05 | [`2026-05-25-phase3-05-dashboard-accounts-ui.md`](./2026-05-25-phase3-05-dashboard-accounts-ui.md) | Dashboard total-assets card + per-account balance breakdown; Accounts page shows current balance (replacing initial-balance display) | 03 |
| 06 | [`2026-05-25-phase3-06-backup-e2e.md`](./2026-05-25-phase3-06-backup-e2e.md) | Backup roundtrip test exercising a transfer row; Playwright E2E covering transfer creation + balance update; Phase 3 final DoD sweep | 05 |

**Repository state baseline:** Phase 2 complete on `main`. Routes already include `/transactions`, `/accounts`, `/categories`, `/settings`, `/` (Dashboard). `transaction_repo::list` already filters `WHERE type IN ('income','expense')`; this plan loosens that to `WHERE 1=1` and instead lets the new `TypeFilter` enum surface `'all'` or `'transfer'` explicitly. The DB CHECK constraint on `transactions` already enforces transfer-vs-non-transfer shape (V001). `backup.rs::insert_transaction` already round-trips `counter_account_id`, but no test currently covers a transfer row — slice 06 closes that gap.

**No new dependencies.** Everything Phase 3 needs is already in `Cargo.toml` and `package.json` (chrono, rusqlite, serde, proptest as dev-dep; @tauri-apps/api, chart.js, svelte-spa-router).

---

## File Structure

### Created

**Rust (src-tauri/src/):**

- `domain/balance.rs` — pure `compute_balance(initial, txs, account_id) -> i64`
- `infra/repo/balance_repo.rs` — single-query `list_balances(&conn) -> Vec<AccountBalance>`
- `commands/balances.rs` — Tauri command `list_balances`

**Rust migrations:**

- `src-tauri/migrations/V002__add_counter_account_idx.sql` — `CREATE INDEX idx_tx_counter_account ON transactions(counter_account_id);`

**Rust integration tests (src-tauri/tests/):**

- `integration_transfers.rs` — insert + update + delete a transfer through the repo, assert balance recalculation
- `integration_balances.rs` — seeded DB → `list_balances` returns correct values across all account kinds

**Frontend (src/):**

- `lib/api/balances.ts` — typed `listBalances()`
- `lib/api/balances.test.ts` — Vitest stub
- `lib/stores/balances.svelte.ts` — reactive store listening to `data:changed`

**E2E (tests/e2e/):**

- `transfer-flow.spec.ts` — create source/dest accounts, create a transfer, assert Dashboard total-assets is unchanged and per-account breakdown moves money

### Modified

- `src-tauri/src/domain/ledger.rs` — add `RawTransferInput`, `ValidatedTransferInput`, `validate_transfer_input`
- `src-tauri/src/infra/repo/transaction_repo.rs` — drop the `type IN ('income','expense')` clamp from `build_where`; add `insert_transfer` / `update_transfer` paths
- `src-tauri/src/infra/repo/mod.rs` — `pub mod balance_repo;`
- `src-tauri/src/commands/transactions.rs` — add `create_transfer`, `update_transfer` handlers; existing `update_transaction` keeps clearing `counter_account_id` (income/expense only)
- `src-tauri/src/commands/mod.rs` — `pub mod balances;`
- `src-tauri/src/domain/mod.rs` — `pub mod balance;`
- `src-tauri/src/lib.rs` — register `commands::balances::list_balances`, `commands::transactions::create_transfer`, `commands::transactions::update_transfer`
- `src/lib/api/transactions.ts` — add `createTransfer` / `updateTransfer` typed wrappers; keep `TxType` widened to include `'transfer'`
- `src/lib/api/index.ts` — re-export `balances`
- `src/routes/Transactions.svelte` — modal: when type=transfer, show source + destination dropdowns and hide category; allow editing transfer rows
- `src/routes/Dashboard.svelte` — new total-assets card + per-account balance list driven by the balances store
- `src/routes/Accounts.svelte` — list shows current balance instead of `initial_balance`

---

## Conventions Used in This Plan

- All RFC3339 timestamps come from `chrono::Utc::now().to_rfc3339()` on the Rust side. Frontend never invents `created_at`.
- All `occurred_on` values are `'YYYY-MM-DD'` strings. Rust validates with `chrono::NaiveDate::parse_from_str(..., "%Y-%m-%d")`.
- Rust types serialized to the frontend use `#[serde(rename_all = "snake_case")]` on enums and field names matching SQL columns (snake_case). TypeScript types mirror this.
- Money is always `i64` in Rust, `number` in TypeScript. Never `f64` or string.
- After every write command succeeds, the handler calls `infra::events::emit_changed(&app, ChangedDomain::Transactions)` (transfers) or `ChangedDomain::Accounts` (balances are derived from transactions; the balances store listens to both `'transactions'` and `'accounts'`).
- Each task ends with a commit using Conventional Commits (`feat`, `fix`, `refactor`, `test`, `docs`, `chore`).
- The `cargo` working directory is `src-tauri/`; the `pnpm` working directory is the repo root.
- Transfer invariants (enforced by `validate_transfer_input` AND by the DB CHECK constraint from V001):
  1. `account_id != counter_account_id` (no self-transfer; CHECK does not catch this — validator MUST).
  2. `amount > 0`.
  3. `counter_account_id` is required; `category_id` is forbidden.
  4. `occurred_on` is a valid `YYYY-MM-DD`.

---

## Phase 3 Final DoD (verified in slice 06)

- [ ] `cargo clippy --all-targets -- -D warnings` green.
- [ ] `cargo test` green (existing 2 phases + new Phase 3 tests including proptest invariants).
- [ ] `pnpm test` green.
- [ ] `pnpm check` (svelte-check) green.
- [ ] `pnpm test:e2e` green including new `transfer-flow.spec.ts`.
- [ ] Manual: create two accounts; create an expense, an income, and a transfer; verify the Dashboard total-assets equals `initialA + initialB + income - expense` (transfer does not change the total) and the per-account balances move correctly.
- [ ] Manual: export JSON, wipe DB, import JSON with `mode='overwrite'`; transfer row reappears, balances unchanged.
- [ ] `pnpm tauri build` succeeds on macOS (and Windows if available).
