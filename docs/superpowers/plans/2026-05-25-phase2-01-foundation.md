# Phase 2 — Slice 01: Foundation

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this slice task-by-task.

**Goal:** Land the prerequisites every later slice depends on: new dev-dependencies, error variants, design tokens, router shell with placeholder pages, module scaffolding, and the `data:changed` emit helper.

**Architecture:** Pure plumbing — no DB writes, no domain rules. After this slice the app still does nothing new functionally; it just renders a sidebar + 5 empty routes and exposes a typed event helper for later slices.

**Sibling slices:** Slices 02–06 in this directory.

**Spec:** `docs/superpowers/specs/2026-05-25-phase2-transactions-design.md` sections 2, 3, 6.

---

### Task 1: Add JS dependencies (svelte-spa-router, chart.js)

**Files:**
- Modify: `package.json`

- [ ] **Step 1: Add dependencies**

Run from the repo root:

```bash
pnpm add svelte-spa-router@^4.0.1 chart.js@^4.4.0
```

Expected: `package.json` `dependencies` now includes `svelte-spa-router` and `chart.js`; `pnpm-lock.yaml` updated.

- [ ] **Step 2: Verify install**

```bash
pnpm check
```

Expected: PASS (no svelte-check errors — the new packages are unused so far, that's fine).

- [ ] **Step 3: Commit**

```bash
git add package.json pnpm-lock.yaml
git commit -m "chore(deps): add svelte-spa-router and chart.js for phase 2"
```

---

### Task 2: Add `Conflict` error variant and stop allowing dead code

**Files:**
- Modify: `src-tauri/src/error.rs`

- [ ] **Step 1: Add the failing test**

Add to the `#[cfg(test)] mod tests` block at the bottom of `src-tauri/src/error.rs`:

```rust
    #[test]
    fn conflict_displays_message() {
        let err = AppError::Conflict("name already exists".into());
        assert_eq!(err.to_string(), "conflict: name already exists");
    }

    #[test]
    fn not_found_displays_message() {
        let err = AppError::NotFound("category 42".into());
        assert_eq!(err.to_string(), "not found: category 42");
    }
```

- [ ] **Step 2: Run the new tests and watch them fail**

From `src-tauri/`:

```bash
cargo test --lib error
```

Expected: `conflict_displays_message` FAILS (no `Conflict` variant); `not_found_displays_message` already PASSES.

- [ ] **Step 3: Update the enum**

In `src-tauri/src/error.rs`, replace the `NotFound` and `InvalidArgument` variants and add `Conflict`:

```rust
    #[error("not found: {0}")]
    NotFound(String),

    #[error("invalid argument: {0}")]
    InvalidArgument(String),

    #[error("conflict: {0}")]
    Conflict(String),
```

(Drop both `#[allow(dead_code)]` attributes that were above `NotFound` and `InvalidArgument`.)

- [ ] **Step 4: Run all error tests**

```bash
cargo test --lib error
```

Expected: PASS.

- [ ] **Step 5: Run clippy**

```bash
cargo clippy --all-targets -- -D warnings
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/error.rs
git commit -m "feat(error): add Conflict variant and unblock NotFound/InvalidArgument usage"
```

---

### Task 3: Expand `app.css` design tokens

**Files:**
- Modify: `src/app.css`

- [ ] **Step 1: Replace the file**

Replace the contents of `src/app.css` with:

```css
:root {
  --bg-grad-start: #667eea;
  --bg-grad-end: #764ba2;
  --accent-grad-start: #4facfe;
  --accent-grad-end: #00f2fe;
  --text: #1a1a2e;
  --muted: #5a5a7a;
  --danger: #e53e3e;
  --warning: #dd8500;
  --success: #2f855a;
  --card-bg: rgba(255, 255, 255, 0.95);
  --surface: #ffffff;
  --border: #e2e2ee;
  --radius-sm: 8px;
  --radius-md: 12px;
  --radius-lg: 16px;
  --shadow-sm: 0 2px 8px rgba(0, 0, 0, 0.06);
  --shadow-md: 0 8px 24px rgba(0, 0, 0, 0.10);
  --shadow-lg: 0 12px 40px rgba(0, 0, 0, 0.18);
  --space-1: 0.25rem;
  --space-2: 0.5rem;
  --space-3: 0.75rem;
  --space-4: 1rem;
  --space-5: 1.5rem;
  --space-6: 2rem;
  --sidebar-width: 220px;
  font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', 'Hiragino Sans',
    'Yu Gothic UI', sans-serif;
}

* { box-sizing: border-box; }

html, body {
  margin: 0;
  padding: 0;
  min-height: 100vh;
  color: var(--text);
  background: linear-gradient(135deg, var(--bg-grad-start), var(--bg-grad-end));
}

#app { min-height: 100vh; }

button { font-family: inherit; }

input, select, textarea {
  font-family: inherit;
  font-size: 1rem;
}

.app-shell {
  display: grid;
  grid-template-columns: var(--sidebar-width) 1fr;
  min-height: 100vh;
}

.app-main {
  padding: var(--space-6);
  overflow-y: auto;
}
```

- [ ] **Step 2: Verify the dev server still renders**

```bash
pnpm dev
```

Open `http://localhost:1420` in a browser. Expected: existing schema-version page still loads with the gradient background. Stop the dev server.

- [ ] **Step 3: Commit**

```bash
git add src/app.css
git commit -m "feat(ui): expand design tokens for phase 2 layout"
```

---

### Task 4: Add `Sidebar.svelte` component

**Files:**
- Create: `src/lib/components/Sidebar.svelte`

- [ ] **Step 1: Create the component**

```svelte
<script lang="ts">
  import { link, location } from 'svelte-spa-router';

  const items = [
    { path: '/',             label: 'ダッシュボード', icon: '🏠' },
    { path: '/transactions', label: '取引',           icon: '💸' },
    { path: '/categories',   label: 'カテゴリ',       icon: '🏷️' },
    { path: '/accounts',     label: '口座',           icon: '🏦' },
    { path: '/settings',     label: '設定',           icon: '⚙️' },
  ];

  function isActive(path: string, current: string): boolean {
    if (path === '/') return current === '/';
    return current.startsWith(path);
  }
</script>

<nav class="sidebar" aria-label="Primary navigation">
  <div class="brand">BudgetTracker</div>
  <ul>
    {#each items as item}
      <li>
        <a
          href={item.path}
          use:link
          class:active={isActive(item.path, $location)}
          data-testid={`nav-${item.path === '/' ? 'dashboard' : item.path.slice(1)}`}
        >
          <span class="icon" aria-hidden="true">{item.icon}</span>
          <span>{item.label}</span>
        </a>
      </li>
    {/each}
  </ul>
</nav>

<style>
  .sidebar {
    background: rgba(0, 0, 0, 0.25);
    color: white;
    padding: var(--space-5) var(--space-3);
    backdrop-filter: blur(8px);
  }
  .brand {
    font-weight: 700;
    font-size: 1.2rem;
    margin: 0 var(--space-3) var(--space-5);
    letter-spacing: 0.04em;
  }
  ul { list-style: none; padding: 0; margin: 0; }
  a {
    display: flex;
    gap: var(--space-3);
    align-items: center;
    padding: var(--space-3) var(--space-4);
    border-radius: var(--radius-md);
    color: inherit;
    text-decoration: none;
    opacity: 0.85;
  }
  a:hover { opacity: 1; background: rgba(255, 255, 255, 0.08); }
  a.active { opacity: 1; background: rgba(255, 255, 255, 0.18); font-weight: 600; }
  .icon { font-size: 1.1rem; }
</style>
```

- [ ] **Step 2: Commit**

```bash
git add src/lib/components/Sidebar.svelte
git commit -m "feat(ui): add Sidebar component with svelte-spa-router links"
```

---

### Task 5: Replace `App.svelte` with router shell + placeholder routes

**Files:**
- Modify: `src/App.svelte`
- Create: `src/routes/Dashboard.svelte`
- Create: `src/routes/Transactions.svelte`
- Create: `src/routes/Categories.svelte`
- Create: `src/routes/Accounts.svelte`
- Create: `src/routes/Settings.svelte`

- [ ] **Step 1: Create five placeholder route components**

For each of `Dashboard.svelte`, `Transactions.svelte`, `Categories.svelte`, `Accounts.svelte`, `Settings.svelte` under `src/routes/`, write the same shape with the matching title and `data-testid`:

```svelte
<!-- src/routes/Dashboard.svelte -->
<section>
  <h1>ダッシュボード</h1>
  <p data-testid="page-dashboard">Phase 2 で実装します。</p>
</section>
```

Variations (only the visible title and the `data-testid` value change):

- `Transactions.svelte` → `<h1>取引</h1>` + `data-testid="page-transactions"`
- `Categories.svelte` → `<h1>カテゴリ</h1>` + `data-testid="page-categories"`
- `Accounts.svelte` → `<h1>口座</h1>` + `data-testid="page-accounts"`
- `Settings.svelte` → `<h1>設定</h1>` + `data-testid="page-settings"`

- [ ] **Step 2: Replace `App.svelte`**

```svelte
<script lang="ts">
  import Router from 'svelte-spa-router';
  import Sidebar from './lib/components/Sidebar.svelte';
  import Dashboard from './routes/Dashboard.svelte';
  import Transactions from './routes/Transactions.svelte';
  import Categories from './routes/Categories.svelte';
  import Accounts from './routes/Accounts.svelte';
  import Settings from './routes/Settings.svelte';

  const routes = {
    '/':             Dashboard,
    '/transactions': Transactions,
    '/categories':   Categories,
    '/accounts':     Accounts,
    '/settings':     Settings,
    '*':             Dashboard,
  };
</script>

<div class="app-shell">
  <Sidebar />
  <main class="app-main">
    <Router {routes} />
  </main>
</div>
```

- [ ] **Step 3: Run svelte-check**

```bash
pnpm check
```

Expected: 0 errors.

- [ ] **Step 4: Smoke-test in the browser**

```bash
pnpm dev
```

Open `http://localhost:1420`. Expected:
- Sidebar appears on the left with 5 entries.
- Dashboard page is the initial view.
- Clicking each nav item routes to the matching placeholder page.

Stop the dev server.

- [ ] **Step 5: Update the existing E2E smoke test**

Read `tests/e2e/smoke.spec.ts`; if it asserts on the old `schema-version` text on the root page, update its assertion to look for `data-testid="page-dashboard"` (the dashboard placeholder) and the sidebar (`data-testid="nav-dashboard"`). Do not delete other assertions; just point them at the new shell.

- [ ] **Step 6: Run Playwright**

```bash
pnpm test:e2e
```

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add src/App.svelte src/routes tests/e2e/smoke.spec.ts
git commit -m "feat(ui): mount svelte-spa-router with five placeholder routes"
```

---

### Task 6: Scaffold Rust modules and `infra::events` helper

**Files:**
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/domain/mod.rs`
- Modify: `src-tauri/src/infra/mod.rs`
- Create: `src-tauri/src/infra/events.rs`
- Create: `src-tauri/src/infra/repo/mod.rs`
- Create: `src-tauri/src/commands/{categories,accounts,transactions,reports,backup,settings}.rs` (empty for now)
- Create: `src-tauri/src/domain/{category,account,ledger,report,seed}.rs` (empty for now)

- [ ] **Step 1: Create the `infra/events.rs` helper**

```rust
use serde::Serialize;
use tauri::{AppHandle, Emitter};

/// Domains that can be broadcast on the `data:changed` channel.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangedDomain {
    Categories,
    Accounts,
    Transactions,
    Meta,
}

#[derive(Debug, Serialize)]
struct ChangedEvent {
    domain: ChangedDomain,
}

/// Emit `data:changed { domain }` to all webview windows. Failures are logged
/// to stderr but never propagated — UI re-fetch is best-effort.
pub fn emit_changed(app: &AppHandle, domain: ChangedDomain) {
    if let Err(e) = app.emit("data:changed", ChangedEvent { domain }) {
        eprintln!("[events] failed to emit data:changed: {e}");
    }
}
```

- [ ] **Step 2: Create `infra/repo/mod.rs`**

```rust
//! SQL access split by table. Each repo takes `&rusqlite::Connection`
//! (or `&mut Connection` when starting a transaction) and returns
//! domain types defined in `crate::domain`.
//!
//! Repos do not lock the `Mutex<Connection>` themselves — the command layer
//! does that and passes a borrowed connection.
```

- [ ] **Step 3: Create six empty command files**

For each of `categories.rs`, `accounts.rs`, `transactions.rs`, `reports.rs`, `backup.rs`, `settings.rs` under `src-tauri/src/commands/`, write a one-line file:

```rust
//! Phase 2 — populated in slices 02..06.
```

- [ ] **Step 4: Create five empty domain files**

For each of `category.rs`, `account.rs`, `ledger.rs`, `report.rs`, `seed.rs` under `src-tauri/src/domain/`, write the same one-line stub:

```rust
//! Phase 2 — populated in slices 02..06.
```

- [ ] **Step 5: Wire `mod` declarations**

Replace `src-tauri/src/commands/mod.rs` with:

```rust
pub mod meta;
pub mod categories;
pub mod accounts;
pub mod transactions;
pub mod reports;
pub mod backup;
pub mod settings;
```

Replace `src-tauri/src/domain/mod.rs` with:

```rust
pub mod category;
pub mod account;
pub mod ledger;
pub mod report;
pub mod seed;
```

Replace `src-tauri/src/infra/mod.rs` with:

```rust
pub mod db;
pub mod events;
pub mod keychain;
pub mod migrations;
pub mod repo;
```

(Keep whatever visibility/order was there before for `db`, `keychain`, `migrations`; just add `events` and `repo`.)

- [ ] **Step 5b: Make the top-level modules public in `lib.rs`**

Integration tests under `src-tauri/tests/` import `budget_tracker_lib::domain::…`, `…::infra::repo::…`, etc. Change the four declarations at the top of `src-tauri/src/lib.rs`:

```rust
mod commands;
mod domain;
mod error;
mod infra;
```

to:

```rust
pub mod commands;
pub mod domain;
pub mod error;
pub mod infra;
```

Then run `cargo build` to verify nothing breaks.

- [ ] **Step 6: Build**

```bash
cargo build
```

Expected: PASS. If `emit` isn't found, the `Emitter` trait is missing — make sure `use tauri::{AppHandle, Emitter};` is present in `events.rs`.

- [ ] **Step 7: Run clippy and tests**

```bash
cargo clippy --all-targets -- -D warnings
cargo test
```

Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add src-tauri/src/commands src-tauri/src/domain src-tauri/src/infra src-tauri/src/lib.rs
git commit -m "chore(scaffold): add empty phase 2 module files, data:changed helper, expose modules"
```

---

### Task 7: Add `proptest` as a dev-dependency

**Files:**
- Modify: `src-tauri/Cargo.toml`

- [ ] **Step 1: Edit Cargo.toml**

Under `[dev-dependencies]` add:

```toml
proptest = "1"
```

- [ ] **Step 2: Build**

```bash
cargo build --tests
```

Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "chore(deps): add proptest dev-dep for ledger property tests"
```

---

### Foundation slice DoD

Before moving to slice 02, verify:

- [ ] `cargo clippy --all-targets -- -D warnings` is green.
- [ ] `cargo test` is green.
- [ ] `pnpm check` is green.
- [ ] `pnpm test:e2e` is green (the smoke test now hits the new shell).
- [ ] `pnpm tauri dev` opens, sidebar renders, all 5 routes navigate.
- [ ] Git log shows the seven commits above.
