# Phase 2 Implementation Plan — Overview

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement each slice task-by-task. This document is the INDEX; the actual tasks live in the six slice files listed in the next section.

**Goal:** Deliver the Phase 2 scope from `docs/superpowers/specs/2026-05-25-phase2-transactions-design.md` — transaction CRUD (income/expense), categories, accounts, dashboard, JSON backup — as a working Tauri app on top of the Phase 1 SQLCipher foundation.

**Architecture:** Vertical slices (Approach A from brainstorming). Each domain (categories → accounts → transactions → dashboard → backup) is taken Rust-first: `domain/` validator (test-first) → `infra/repo/` SQL → `commands/` Tauri command → frontend `lib/api/` wrapper → `lib/stores/` reactive store → route page. Components are added in the first slice that needs them. Tests live alongside code (domain unit), in `src-tauri/tests/` (integration), and `tests/e2e/` (Playwright).

**Tech Stack:** Tauri 2.x, Rust + rusqlite (`bundled-sqlcipher-vendored-openssl`), Svelte 5 (Runes) + TypeScript, Vite, Chart.js, svelte-spa-router, Vitest, Playwright, `proptest`.

## Slices (execute in order)

| # | File | Scope | Depends on |
|---|------|-------|------------|
| 01 | [`2026-05-25-phase2-01-foundation.md`](./2026-05-25-phase2-01-foundation.md) | deps, error variants, app.css, router shell, module scaffolding, `data:changed` helper, `proptest` dev-dep | — |
| 02 | [`2026-05-25-phase2-02-categories.md`](./2026-05-25-phase2-02-categories.md) | category domain/repo/commands/api/store/UI, default-category seed, shared UI primitives (Card, Button, Modal, TextField, Select, CategoryBadge, EmptyState) | 01 |
| 03 | [`2026-05-25-phase2-03-accounts.md`](./2026-05-25-phase2-03-accounts.md) | account domain/repo/commands/api/store/UI | 02 |
| 04 | [`2026-05-25-phase2-04-transactions.md`](./2026-05-25-phase2-04-transactions.md) | ledger validators, `aggregate_monthly` + proptest, transaction repo (filter+pagination), commands, api, store, DatePicker, transactions page | 03 |
| 05 | [`2026-05-25-phase2-05-dashboard.md`](./2026-05-25-phase2-05-dashboard.md) | `report` domain (YearMonth, fill_monthly_series), `report_repo`, monthly_summary + monthly_series commands, Dashboard cards + Chart.js bar | 04 |
| 06 | [`2026-05-25-phase2-06-backup-settings.md`](./2026-05-25-phase2-06-backup-settings.md) | JSON export/import (overwrite+append), Settings page, Playwright E2E, Phase 2 final DoD sweep | 05 |

**Repository state baseline:** `main @ 4e6be98` (spec committed). `src-tauri/src/{commands/meta.rs, infra/{db,keychain,migrations}.rs, error.rs}` and migration `V001__init.sql` already exist; `AppState { conn: Mutex<Connection>, db_path }` is wired in `lib.rs`. `error.rs` already declares `NotFound` and `InvalidArgument` variants (currently `#[allow(dead_code)]`). `chrono` with `serde` is already a dependency.

---

## File Structure

### Created

**Rust (src-tauri/src/):**

- `commands/categories.rs`, `commands/accounts.rs`, `commands/transactions.rs`, `commands/reports.rs`, `commands/backup.rs`, `commands/settings.rs` — Tauri command handlers
- `domain/category.rs`, `domain/account.rs`, `domain/ledger.rs`, `domain/report.rs`, `domain/seed.rs` — pure-Rust types and validators
- `infra/repo/mod.rs`, `infra/repo/{category,account,transaction,report,meta}_repo.rs` — SQL access
- `infra/events.rs` — `data:changed` emit helper

**Rust integration tests (src-tauri/tests/):**

- `integration_categories.rs`, `integration_accounts.rs`, `integration_transactions.rs`, `integration_backup.rs`, `integration_reports.rs`

**Frontend (src/):**

- `lib/api/{categories,accounts,transactions,reports,backup,settings}.ts`
- `lib/api/events.ts` — typed `data:changed` listener
- `lib/stores/{categories,accounts,transactions}.svelte.ts`
- `lib/components/{Card,Button,Modal,TextField,Select,DatePicker,CategoryBadge,Sidebar,EmptyState}.svelte`
- `lib/utils/yearMonth.ts` — date helpers
- `routes/{Dashboard,Transactions,Categories,Accounts,Settings}.svelte`
- `lib/api/transactions.test.ts`, `lib/stores/categories.test.ts` — Vitest

**E2E (tests/e2e/):**

- `transaction-flow.spec.ts`

### Modified

- `src-tauri/src/lib.rs` — register new commands, run default-category seed
- `src-tauri/src/commands/mod.rs` — `pub mod` declarations for the six new modules
- `src-tauri/src/domain/mod.rs` — `pub mod` declarations for five new modules
- `src-tauri/src/infra/mod.rs` — `pub mod repo;` and `pub mod events;`
- `src-tauri/src/error.rs` — drop `#[allow(dead_code)]` on `NotFound` / `InvalidArgument`; add `Conflict` variant
- `src-tauri/Cargo.toml` — add `proptest` (dev-dep)
- `package.json` — add `svelte-spa-router`, `chart.js`
- `src/App.svelte` — replace stub with router shell
- `src/app.css` — design tokens, layout primitives
- `src/lib/api/index.ts` — barrel re-exports of new modules

---

## Conventions Used in This Plan

- All RFC3339 timestamps come from `chrono::Utc::now().to_rfc3339()` on the Rust side. Frontend never invents `created_at`.
- All `occurred_on` values are `'YYYY-MM-DD'` strings. Rust validates with `chrono::NaiveDate::parse_from_str(..., "%Y-%m-%d")`.
- Rust types serialized to the frontend use `#[serde(rename_all = "snake_case")]` on enums and field names matching the SQL column names (snake_case). TypeScript types mirror this.
- Money is always `i64` in Rust, `number` in TypeScript. Never `f64` or string.
- After every write command succeeds, the handler calls `infra::events::emit_changed(&app, "<domain>")`.
- Each task ends with a commit using Conventional Commits (`feat`, `fix`, `refactor`, `test`, `docs`, `chore`).
- The `cargo` working directory is `src-tauri/`; the `pnpm` working directory is the repo root.

---
