# Phase 4 Review Fixes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the Phase 4 code review so calendar dates, key/decrypt recovery, and archived-master edits are correct, then harden error handling, command validation, and spec/CI drift before Phase 5.

**Architecture:** Keep the existing split (thin Tauri commands, domain functions, SQLite repos, Svelte display). Extract a testable boot engine so Keychain/SQLCipher setup no longer panics. Push remaining Svelte-side totals and top-N budget ranking into Rust. Do not add new product features (recurring, report tabs, Excel, `ends_on` carry-forward).

**Tech Stack:** Tauri 2.x, Rust + rusqlite (`bundled-sqlcipher-vendored-openssl`), `keyring`, Svelte 5 Runes + TypeScript, Vitest, Playwright, `chrono` Local for calendar dates.

**Spec:** [docs/superpowers/specs/2026-05-24-budget-tracker-design.md](../specs/2026-05-24-budget-tracker-design.md)

**Review (source of work):** [docs/superpowers/reviews/2026-08-29-phase4-code-review.md](../reviews/2026-08-29-phase4-code-review.md)

## Global Constraints

- Amounts are integer yen only (`i64` / `number` with no float math).
- Aggregation, budget evaluation, and ranking live in `src-tauri/src/domain/`. Svelte displays invoke results.
- `transactions.type = 'transfer'` is excluded from income/expense totals (`WHERE type IN ('income','expense')`).
- Do not hardcode bank, card, or category names. `accounts.kind` is the only fixed list: `cash` / `bank` / `credit_card` / `e_money` / `investment`.
- Categories and accounts are archived (`archived_at`), never physically deleted.
- Schema changes go through a new `src-tauri/migrations/V<NNN>__*.sql`. Do not edit existing migrations. This plan needs **no new migration**.
- SQLCipher has no plaintext fallback. Do not mint a new Keychain key beside an existing `data.db`.
- Calendar dates (`YYYY-MM-DD`, `YYYY-MM`) use the local timezone. RFC3339 timestamps (`created_at`, `exported_at`, `archived_at`) stay UTC.
- Conventional Commits. Each task ends with a commit.
- Phase completion commands after each wave: `cd src-tauri && cargo test --no-fail-fast`, `cd src-tauri && cargo clippy --all-targets -- -D warnings`, `pnpm test`, `pnpm check`. Playwright when the task touches E2E or a user-visible route.

## Locked decisions (do not reopen during implementation)

1. **Ship gate is Wave A (Bugs 1–3).** Waves B–E may follow in the same branch, but Wave A must be independently green.
2. **`ends_on` continuation is not implemented.** Phase 4 lookup stays `period = 'monthly' AND starts_on = <first of selected month>`. Parent spec is updated to say so (Task 16). Carry-forward is a later enhancement.
3. **Keychain service stays `jp.budget-tracker` / `db_key`.** Spec Windows example and DB path text are updated to match `app_data_dir()` (`jp.budget-tracker.app/data.db`). Never rename the service; that would orphan existing keys.
4. **Excel import/export stays deferred** (not Phase 4, not this plan). Parent spec is labeled accordingly.
5. **Report 4-tab commands (`report_yearly` / `report_by_category` / `report_net_worth_series`) stay Phase 5.** Document implemented commands as `monthly_summary` / `monthly_series`.
6. **JSON `schema_version` is a backup-format version**, currently `1`, independent of `app_meta.schema_version` (currently `3`).
7. **Archived FK on edit:** keep the row's current archived account/category. Reject archived IDs on **create**, and on **update that retargets** to a different archived master.
8. **Asset/liability split UI is out of scope.** `total_assets` is the sum of non-archived account balances (current Dashboard meaning), computed in Rust.
9. **`release.yml` and tauri-driver stay Phase 6.** This plan only corrects spec §8 wording and commits `Cargo.lock`.
10. **Out of scope:** Phase 5 recurring expansion, report tabs, Excel, Keychain rename, plaintext SQLCipher fallback.

## Waves

| Wave | Review items | Why this order |
|---|---|---|
| A | Bugs 1, 2, 3 | Review ship gate: wrong "today", unrecoverable decrypt, silent remap of history |
| B | Bugs 4–8, suggestion 22 | Errors and races that make the app look empty or stale |
| C | Suggestions 11, 15–19 | Command/domain hardening; FK ON before referential tests |
| D | Suggestions 20, 21, 24 | Modal a11y, missing filters, yen formatting |
| E | Suggestions 9, 10, 12–14, 23, 25 | Spec/CI/lockfile/mocks/nits |

## File Structure

### Create

- `src-tauri/src/infra/boot.rs` — testable DB boot: first launch vs recovery; never creates a key when `data.db` exists; owns `KEYCHAIN_SERVICE` / `KEYCHAIN_ACCOUNT` / `DB_FILENAME`
- `src-tauri/src/commands/recovery.rs` — `boot_status`, `recover_import_json`, `recover_start_empty`
- `src-tauri/tests/integration_boot.rs` — boot engine against tempfile + mock `KeyStore`
- `src-tauri/tests/integration_recovery.rs` — quarantine + import into a new encrypted file
- `src/lib/api/boot.ts` — typed wrappers for boot/recovery commands
- `src/lib/api/boot.test.ts` — invoke payload coverage
- `src/routes/Recovery.svelte` — decrypt-failure screen (JSON restore / start empty)
- `src/lib/components/ErrorBanner.svelte` — shared page error (`role="alert"`)
- `src/lib/utils/yearMonth.test.ts` — local `isoToday` regression
- `src/lib/utils/selectOptions.ts` / `src/lib/utils/selectOptions.test.ts` — keep current archived ids in select lists
- `src/lib/stores/transactions.test.ts` — stale `load()` ignored; error populated
- `src/lib/components/Modal.test.ts` — focus trap / Escape
- `tests/e2e/recovery-flow.spec.ts` — mocked `boot_status === recovery` UI
- `tests/e2e/tauriMock.ts` — shared Playwright `boot_status: ready` stub used by smoke and the flow specs

### Modify (by concern)

- Dates: `src/lib/utils/yearMonth.ts`, `src/routes/Budgets.svelte`, `src/routes/Transactions.svelte`, `src-tauri/src/commands/reports.rs`, `src-tauri/src/domain/report.rs`
- Boot: `src-tauri/src/infra/keychain.rs`, `src-tauri/src/infra/mod.rs`, `src-tauri/src/infra/db.rs` (`looks_like_decrypt_failure`), `src-tauri/src/error.rs`, `src-tauri/src/lib.rs`, `src-tauri/src/commands/meta.rs`, every command that locks `state.conn`
- Archive edit: `src/routes/Transactions.svelte`
- Errors/races: `src/lib/stores/*.svelte.ts`, `src/lib/stores/*.test.ts`, `src/routes/{Transactions,Categories,Accounts,Dashboard,Budgets}.svelte`
- Validation: `src-tauri/src/domain/ledger.rs`, `src-tauri/src/commands/transactions.rs`, `src-tauri/src/infra/repo/transaction_repo.rs`, `src-tauri/src/infra/migrations.rs`, `src-tauri/src/commands/backup.rs`
- Totals: `src-tauri/src/domain/balance.rs`, `src-tauri/src/domain/budget.rs`, `src-tauri/src/commands/balances.rs`, `src-tauri/src/commands/budgets.rs`, `src/lib/api/balances.ts`, `src/lib/api/budgets.ts`, `src/lib/stores/balances.svelte.ts`, `src/routes/Dashboard.svelte`
- UI: `src/lib/components/Modal.svelte`, `src/routes/Accounts.svelte`, `src/App.svelte`
- Docs/CI: `docs/superpowers/specs/2026-05-24-budget-tracker-design.md`, `.gitignore`, `.github/workflows/ci.yml`, `tests/e2e/*.spec.ts`, `AGENTS.md`, `CLAUDE.md`

### Do not create

- New SQL migration
- `release.yml`
- `domain/recurring.rs` / report 4-tab commands

## Interfaces

### Boot

```rust
pub const KEYCHAIN_SERVICE: &str = "jp.budget-tracker";
pub const KEYCHAIN_ACCOUNT: &str = "db_key";
pub const DB_FILENAME: &str = "data.db";
// Declare these in infra/boot.rs in Task 2 (not later).

pub trait KeyStore {
    fn get(&self) -> AppResult<Option<DbKey>>;
    fn create(&self) -> AppResult<DbKey>;
    fn delete(&self) -> AppResult<()>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryReason {
    KeyMissing,
    DecryptFailed,
    KeyCorrupt,
    KeychainError,
}

pub enum BootOutcome {
    Ready { conn: Connection, db_path: PathBuf },
    Recovery { reason: RecoveryReason, data_dir: PathBuf, db_path: PathBuf },
}

pub fn boot(data_dir: &Path, keys: &dyn KeyStore) -> AppResult<BootOutcome>;
```

### AppState (after Task 3)

```rust
pub struct AppState {
    pub inner: Mutex<AppInner>,
}

pub enum AppInner {
    Ready { conn: Connection, db_path: PathBuf },
    Recovery {
        reason: RecoveryReason,
        data_dir: PathBuf,
        db_path: PathBuf,
    },
}

impl AppState {
    pub fn db_path(&self) -> AppResult<PathBuf>;
    pub fn with_conn<R>(&self, f: impl FnOnce(&Connection) -> AppResult<R>) -> AppResult<R>;
    pub fn with_conn_mut<R>(&self, f: impl FnOnce(&mut Connection) -> AppResult<R>) -> AppResult<R>;
}
```

### Recovery commands

```ts
export type BootStatus = {
  state: 'ready' | 'recovery';
  recovery_reason: 'key_missing' | 'decrypt_failed' | 'key_corrupt' | 'keychain_error' | null;
  db_path: string;
};

bootStatus(): Promise<BootStatus>
recoverImportJson(payload: string): Promise<ImportResult>  // overwrite into a NEW db file
recoverStartEmpty(): Promise<void>
```

### Balance list (after Task 13)

```ts
export type BalanceList = {
  accounts: AccountBalance[];
  total_assets: number; // integer yen; sum of accounts where archived_at == null
};
```

### Top budgets (after Task 13)

```ts
listTopBudgetStatuses(yearMonth: string, limit: number): Promise<BudgetStatus[]>
// limit in 1..=10. Dashboard calls with 3.
```

### Ledger refs (after Task 11)

```rust
pub struct AllowedArchivedRefs {
    pub account_id: Option<i64>,
    pub category_id: Option<i64>,
    pub counter_account_id: Option<i64>,
}

pub fn assert_account_writable(
    account: &Account,
    allow_id: Option<i64>,
) -> AppResult<()>;

pub fn assert_category_matches_tx(
    category: &Category,
    tx_type: TxType,
    allow_id: Option<i64>,
) -> AppResult<()>;
```

---

## Wave A — Ship gate (Bugs 1–3)

### Task 1: Local calendar dates

**Files:**
- Create: `src/lib/utils/yearMonth.test.ts`
- Modify: `src/lib/utils/yearMonth.ts`
- Modify: `src-tauri/src/domain/report.rs` (add `year_month_from_date`)
- Modify: `src-tauri/src/commands/reports.rs:35` (`Utc` → `Local`)
- Test: existing `src-tauri/src/domain/report.rs` unit tests; `src/lib/utils/yearMonth.test.ts`

**Review:** Bug 1. `isoToday()` uses `toISOString()` (UTC). `monthly_series` uses `chrono::Utc`. Dashboard and `list_budget_statuses` already use local. JST 00:00–08:59 disagrees.

**Interfaces:**
- Consumes: none
- Produces: `isoToday(): string` as local `YYYY-MM-DD`; `year_month_from_date(NaiveDate) -> report::YearMonth`; `monthly_series` end month from `chrono::Local::now().date_naive()`

Do **not** change `chrono::Utc::now().to_rfc3339()` in `created_at` / `exported_at` / `archived_at`.

- [ ] **Step 1: Write the failing frontend test**

```ts
import { afterEach, describe, expect, it, vi } from 'vitest';
import { isoToday, monthRange } from './yearMonth';

describe('isoToday', () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  it('returns local YYYY-MM-DD and does not use toISOString for the date', () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date(2026, 5, 1, 0, 30, 0)); // local 2026-06-01 00:30
    const isoSpy = vi.spyOn(Date.prototype, 'toISOString');
    expect(isoToday()).toBe('2026-06-01');
    expect(isoSpy).not.toHaveBeenCalled();
  });
});

describe('monthRange', () => {
  it('returns inclusive local month bounds', () => {
    expect(monthRange(2026, 2)).toEqual({ from: '2026-02-01', to: '2026-02-28' });
    expect(monthRange(2024, 2)).toEqual({ from: '2024-02-01', to: '2024-02-29' });
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pnpm exec vitest run src/lib/utils/yearMonth.test.ts`

Expected: FAIL — `isoToday` still calls `toISOString`, so the spy assertion fails (and in JST the date may be `2026-05-31`).

- [ ] **Step 3: Implement local `isoToday`**

```ts
function pad2(n: number): string {
  return String(n).padStart(2, '0');
}

export function isoToday(): string {
  const now = new Date();
  return `${now.getFullYear()}-${pad2(now.getMonth() + 1)}-${pad2(now.getDate())}`;
}

export function monthRange(year: number, month: number): { from: string; to: string } {
  const m = pad2(month);
  const last = new Date(year, month, 0).getDate();
  return {
    from: `${year}-${m}-01`,
    to: `${year}-${m}-${pad2(last)}`,
  };
}
```

- [ ] **Step 4: Run frontend test**

Run: `pnpm exec vitest run src/lib/utils/yearMonth.test.ts`

Expected: PASS

- [ ] **Step 5: Write the failing Rust test for series end month**

Add to `src-tauri/src/domain/report.rs`:

```rust
pub fn year_month_from_date(date: chrono::NaiveDate) -> YearMonth {
    use chrono::Datelike;
    YearMonth {
        year: date.year(),
        month: date.month(),
    }
}
```

Test:

```rust
#[test]
fn year_month_from_date_uses_the_naive_calendar_date() {
    let d = chrono::NaiveDate::from_ymd_opt(2026, 6, 1).unwrap();
    let ym = year_month_from_date(d);
    assert_eq!(ym.key(), "2026-06");
}
```

Also add a unit test that `fill_monthly_series` with `end = year_month_from_date(2026-06-01)` and `months = 12` has last bucket `2026-06` (not a UTC-shifted month). If `year_month_from_date` is missing, the test fails to compile — add the test in the same file as the function.

- [ ] **Step 6: Switch `monthly_series` to Local**

In `src-tauri/src/commands/reports.rs`:

```rust
let today = chrono::Local::now().date_naive();
let end = crate::domain::report::year_month_from_date(today);
```

Keep `months` validation `1..=60`. Do not use `chrono::Utc` anywhere this command computes the calendar month. RFC3339 timestamps in other commands stay UTC.

- [ ] **Step 7: Run Rust tests**

Run: `cd src-tauri && cargo test --lib report -- --nocapture`

Expected: PASS, including `year_month_from_date_uses_the_naive_calendar_date`.

- [ ] **Step 8: Commit**

```bash
git add src/lib/utils/yearMonth.ts src/lib/utils/yearMonth.test.ts \
  src-tauri/src/domain/report.rs src-tauri/src/commands/reports.rs
git commit -m "$(cat <<'EOF'
fix: use local calendar dates for today and monthly series

UTC toISOString and chrono::Utc shifted JST early-morning transaction
dates and the dashboard chart's last bar relative to budget/local month.
EOF
)"
```

---

### Task 2: Boot engine that never mints a key beside an existing DB

**Files:**
- Create: `src-tauri/src/infra/boot.rs`
- Create: `src-tauri/tests/integration_boot.rs`
- Modify: `src-tauri/src/infra/keychain.rs` (split get/create)
- Modify: `src-tauri/src/infra/mod.rs` (`pub mod boot;`)
- Modify: `src-tauri/src/infra/db.rs` (`looks_like_decrypt_failure`)
- Modify: `src-tauri/src/error.rs` if a new variant is needed (`Unavailable` is Task 3; this task can stay with existing `Corrupt` / `Keychain` / `Io`)

**Review:** Bug 2 (backend half). `lib.rs` `setup` uses `get_or_create_key` then `.expect()`. If `data.db` exists and Keychain has `NoEntry`, a **new** key is stored and decrypt fails → panic. Spec §6.2 requires a JSON restore path.

**Interfaces:**
- Consumes: `keychain::DbKey`, `db::open_encrypted`, `migrations::run`, `domain::seed::seed_default_categories_if_needed`
- Produces: `KeyStore`, `boot()`, `BootOutcome`, `RecoveryReason`

Keep `get_or_create_key` as a thin wrapper **only for tests that still call it**, or replace those tests with `get`/`create`. Production boot must not call `get_or_create_key`.

**Decrypt vs other failures:** `BootOutcome::Recovery { DecryptFailed }` is **only** for `db::open_encrypted` failing in a way that looks like a SQLCipher key mismatch (see `looks_like_decrypt_failure`). `migrations::run` and `seed_default_categories_if_needed` must use `?` and propagate. A readable DB with a broken migration must **not** open the Recovery UI (that path can quarantine a healthy file).

- [ ] **Step 1: Write failing keychain tests for get vs create**

Add to `src-tauri/src/infra/keychain.rs` tests (reuse the existing mock builder):

```rust
#[test]
fn get_returns_none_when_absent() {
    ensure_mock();
    let account = unique_account("get-none");
    let got = get_key("test", &account).unwrap();
    assert!(got.is_none());
}

#[test]
fn get_does_not_create_an_entry() {
    ensure_mock();
    let account = unique_account("get-no-create");
    let _ = get_key("test", &account).unwrap();
    let again = get_key("test", &account).unwrap();
    assert!(again.is_none());
}

#[test]
fn create_persists_and_get_returns_it() {
    ensure_mock();
    let account = unique_account("create-get");
    let created = create_key("test", &account).unwrap();
    let got = get_key("test", &account).unwrap().unwrap();
    assert_eq!(created, got);
    delete_key("test", &account).unwrap();
}

#[test]
fn create_fails_if_entry_exists() {
    ensure_mock();
    let account = unique_account("create-exists");
    create_key("test", &account).unwrap();
    let err = create_key("test", &account).unwrap_err();
    assert!(matches!(err, AppError::Conflict(_)));
    delete_key("test", &account).unwrap();
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd src-tauri && cargo test --lib keychain::tests::get_returns_none -- --nocapture`

Expected: FAIL — `get_key` / `create_key` not found.

- [ ] **Step 3: Implement `get_key` / `create_key`**

```rust
pub fn get_key(service: &str, account: &str) -> AppResult<Option<DbKey>> {
    let entry = keyring::Entry::new(service, account)?;
    match entry.get_password() {
        Ok(b64) => Ok(Some(decode_key(&b64)?)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

pub fn create_key(service: &str, account: &str) -> AppResult<DbKey> {
    if get_key(service, account)?.is_some() {
        return Err(AppError::Conflict("keychain entry already exists".into()));
    }
    let mut key = [0u8; KEY_LEN];
    rand::thread_rng().fill_bytes(&mut key);
    let entry = keyring::Entry::new(service, account)?;
    entry.set_password(&STANDARD_NO_PAD.encode(key))?;
    Ok(key)
}

fn decode_key(b64: &str) -> AppResult<DbKey> {
    let raw = STANDARD_NO_PAD.decode(b64.as_bytes())?;
    if raw.len() != KEY_LEN {
        return Err(AppError::Corrupt(format!(
            "corrupt keychain entry: stored key has wrong length: {}",
            raw.len()
        )));
    }
    let mut out = [0u8; KEY_LEN];
    out.copy_from_slice(&raw);
    Ok(out)
}
```

Keep the existing `get_or_create_key` tests green by rewriting `get_or_create_key` as:

```rust
pub fn get_or_create_key(service: &str, account: &str) -> AppResult<DbKey> {
    match get_key(service, account)? {
        Some(key) => Ok(key),
        None => create_key(service, account),
    }
}
```

This wrapper is **test-only / legacy**. `boot()` must call `get` / `create` explicitly.

- [ ] **Step 4: Run keychain tests**

Run: `cd src-tauri && cargo test --lib keychain -- --nocapture`

Expected: PASS (including old tests).

- [ ] **Step 5: Write failing boot integration tests**

Create `src-tauri/tests/integration_boot.rs`:

```rust
use budget_tracker_lib::error::AppError;
use budget_tracker_lib::infra::boot::{boot, BootOutcome, KeyStore, RecoveryReason, DB_FILENAME};
use budget_tracker_lib::infra::keychain::{DbKey, KEY_LEN};
use std::path::PathBuf;
use std::sync::Mutex;

struct MemKeys {
    key: Mutex<Option<DbKey>>,
    create_calls: Mutex<u32>,
}

impl KeyStore for MemKeys {
    fn get(&self) -> budget_tracker_lib::error::AppResult<Option<DbKey>> {
        Ok(*self.key.lock().unwrap())
    }
    fn create(&self) -> budget_tracker_lib::error::AppResult<DbKey> {
        *self.create_calls.lock().unwrap() += 1;
        if self.key.lock().unwrap().is_some() {
            return Err(AppError::Conflict("exists".into()));
        }
        let key = [7u8; KEY_LEN];
        *self.key.lock().unwrap() = Some(key);
        Ok(key)
    }
    fn delete(&self) -> budget_tracker_lib::error::AppResult<()> {
        *self.key.lock().unwrap() = None;
        Ok(())
    }
}

fn data_dir() -> tempfile::TempDir {
    tempfile::TempDir::new().unwrap()
}

#[test]
fn first_launch_creates_key_and_db() {
    let dir = data_dir();
    let keys = MemKeys { key: Mutex::new(None), create_calls: Mutex::new(0) };
    let outcome = boot(dir.path(), &keys).unwrap();
    assert!(matches!(outcome, BootOutcome::Ready { .. }));
    assert_eq!(*keys.create_calls.lock().unwrap(), 1);
    assert!(dir.path().join(DB_FILENAME).exists());
}

#[test]
fn existing_db_and_missing_key_is_recovery_and_does_not_create_key() {
    let dir = data_dir();
    std::fs::write(dir.path().join(DB_FILENAME), b"ciphertext-placeholder").unwrap();
    let keys = MemKeys { key: Mutex::new(None), create_calls: Mutex::new(0) };
    let outcome = boot(dir.path(), &keys).unwrap();
    assert!(matches!(
        outcome,
        BootOutcome::Recovery { reason: RecoveryReason::KeyMissing, .. }
    ));
    assert_eq!(*keys.create_calls.lock().unwrap(), 0);
}

#[test]
fn existing_db_and_wrong_key_is_decrypt_failed_and_does_not_create_key() {
    let dir = data_dir();
    let right = [1u8; KEY_LEN];
    let wrong = [2u8; KEY_LEN];
    {
        let path = dir.path().join(DB_FILENAME);
        let mut conn = budget_tracker_lib::infra::db::open_encrypted(&path, &right).unwrap();
        // SQLCipher can treat an empty file as a fresh DB. Write schema under
        // the right key so the wrong key actually fails to decrypt.
        budget_tracker_lib::infra::migrations::run(&mut conn).unwrap();
        drop(conn);
    }
    let keys = MemKeys { key: Mutex::new(Some(wrong)), create_calls: Mutex::new(0) };
    let outcome = boot(dir.path(), &keys).unwrap();
    assert!(matches!(
        outcome,
        BootOutcome::Recovery { reason: RecoveryReason::DecryptFailed, .. }
    ));
    assert_eq!(*keys.create_calls.lock().unwrap(), 0);
}

#[test]
fn existing_db_and_matching_key_is_ready() {
    let dir = data_dir();
    let key = [1u8; KEY_LEN];
    {
        let path = dir.path().join(DB_FILENAME);
        let mut conn = budget_tracker_lib::infra::db::open_encrypted(&path, &key).unwrap();
        budget_tracker_lib::infra::migrations::run(&mut conn).unwrap();
        drop(conn);
    }
    let keys = MemKeys { key: Mutex::new(Some(key)), create_calls: Mutex::new(0) };
    let outcome = boot(dir.path(), &keys).unwrap();
    assert!(matches!(outcome, BootOutcome::Ready { .. }));
    assert_eq!(*keys.create_calls.lock().unwrap(), 0);
}
```

- [ ] **Step 6: Run boot tests to verify they fail**

Run: `cd src-tauri && cargo test --test integration_boot -- --nocapture`

Expected: FAIL — `boot` module missing.

- [ ] **Step 7: Implement `infra/boot.rs`**

```rust
use std::path::{Path, PathBuf};
use rusqlite::Connection;
use serde::Serialize;
use crate::domain::seed;
use crate::error::{AppError, AppResult};
use crate::infra::{db, migrations};
use crate::infra::keychain::DbKey;

pub const KEYCHAIN_SERVICE: &str = "jp.budget-tracker";
pub const KEYCHAIN_ACCOUNT: &str = "db_key";
pub const DB_FILENAME: &str = "data.db";

pub trait KeyStore: Send + Sync {
    fn get(&self) -> AppResult<Option<DbKey>>;
    fn create(&self) -> AppResult<DbKey>;
    fn delete(&self) -> AppResult<()>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryReason {
    KeyMissing,
    DecryptFailed,
    KeyCorrupt,
    KeychainError,
}

pub enum BootOutcome {
    Ready { conn: Connection, db_path: PathBuf },
    Recovery { reason: RecoveryReason, data_dir: PathBuf, db_path: PathBuf },
}

fn open_and_prepare(db_path: &Path, key: &DbKey) -> AppResult<Connection> {
    let mut conn = db::open_encrypted(db_path, key)?;
    migrations::run(&mut conn)?;
    seed::seed_default_categories_if_needed(&mut conn)?;
    Ok(conn)
}

pub fn boot(data_dir: &Path, keys: &dyn KeyStore) -> AppResult<BootOutcome> {
    std::fs::create_dir_all(data_dir)?;
    let db_path = data_dir.join(DB_FILENAME);
    let db_exists = db_path.exists();

    let key = match keys.get() {
        Ok(v) => v,
        Err(AppError::Corrupt(_)) => {
            return Ok(BootOutcome::Recovery {
                reason: RecoveryReason::KeyCorrupt,
                data_dir: data_dir.to_path_buf(),
                db_path,
            });
        }
        Err(_) => {
            return Ok(BootOutcome::Recovery {
                reason: RecoveryReason::KeychainError,
                data_dir: data_dir.to_path_buf(),
                db_path,
            });
        }
    };

    match (db_exists, key) {
        (false, None) => {
            let key = keys.create()?;
            let conn = open_and_prepare(&db_path, &key)?;
            Ok(BootOutcome::Ready { conn, db_path })
        }
        (false, Some(key)) => {
            let conn = open_and_prepare(&db_path, &key)?;
            Ok(BootOutcome::Ready { conn, db_path })
        }
        (true, None) => Ok(BootOutcome::Recovery {
            reason: RecoveryReason::KeyMissing,
            data_dir: data_dir.to_path_buf(),
            db_path,
        }),
        (true, Some(key)) => match db::open_encrypted(&db_path, &key) {
            Ok(mut conn) => {
                // Readable DB: migration/seed failures must propagate, not
                // become Recovery (that would let recover_* quarantine it).
                migrations::run(&mut conn)?;
                seed::seed_default_categories_if_needed(&mut conn)?;
                Ok(BootOutcome::Ready { conn, db_path })
            }
            Err(err) if db::looks_like_decrypt_failure(&err) => {
                Ok(BootOutcome::Recovery {
                    reason: RecoveryReason::DecryptFailed,
                    data_dir: data_dir.to_path_buf(),
                    db_path,
                })
            }
            Err(err) => Err(err),
        },
    }
}

pub struct OsKeyStore {
    pub service: String,
    pub account: String,
}

impl KeyStore for OsKeyStore {
    fn get(&self) -> AppResult<Option<DbKey>> {
        crate::infra::keychain::get_key(&self.service, &self.account)
    }
    fn create(&self) -> AppResult<DbKey> {
        crate::infra::keychain::create_key(&self.service, &self.account)
    }
    fn delete(&self) -> AppResult<()> {
        crate::infra::keychain::delete_key(&self.service, &self.account)
    }
}
```

Add to `src-tauri/src/infra/db.rs` next to `open_encrypted` (import `AppError` alongside `AppResult`):

```rust
pub fn looks_like_decrypt_failure(err: &AppError) -> bool {
    let text = err.to_string().to_lowercase();
    text.contains("not a database")
        || text.contains("file is encrypted")
        || text.contains("hmac check failed")
}
```

Add a unit test in `db.rs` that `open_encrypted(&path, &wrong)` returns Err and `looks_like_decrypt_failure` is true for that error (reuse `wrong_key_fails_to_decrypt`).

`delete_key` is currently `#[cfg(test)]`. Export it for recovery of `KeyCorrupt` (Task 3 will call `keys.delete()` then `keys.create()` **after** the old DB is quarantined). In this task, make `delete_key` available in all builds:

```rust
pub fn delete_key(service: &str, account: &str) -> AppResult<()> {
    let entry = keyring::Entry::new(service, account)?;
    match entry.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e.into()),
    }
}
```

- [ ] **Step 8: Run boot + keychain tests**

Run: `cd src-tauri && cargo test --lib keychain && cargo test --test integration_boot`

Expected: PASS

- [ ] **Step 9: Commit**

```bash
git add src-tauri/src/infra/boot.rs src-tauri/src/infra/mod.rs \
  src-tauri/src/infra/keychain.rs src-tauri/src/infra/db.rs \
  src-tauri/tests/integration_boot.rs
git commit -m "$(cat <<'EOF'
fix: refuse to mint a DB key when data.db already exists

A missing Keychain entry next to an encrypted file used to create a
new key and then panic on decrypt, destroying the restore path.
EOF
)"
```

---

### Task 3: Recovery AppState, commands, and Recovery UI

**Files:**
- Create: `src-tauri/src/commands/recovery.rs`
- Create: `src-tauri/tests/integration_recovery.rs`
- Create: `src/lib/api/boot.ts`, `src/lib/api/boot.test.ts`
- Create: `src/routes/Recovery.svelte`
- Create: `tests/e2e/recovery-flow.spec.ts`
- Modify: `src-tauri/src/error.rs` (`Unavailable`)
- Modify: `src-tauri/src/commands/meta.rs` (`AppState` → `Mutex<AppInner>`)
- Modify: `src-tauri/src/commands/mod.rs`, `src-tauri/src/lib.rs`
- Modify: every command that currently does `state.conn.lock()` — switch to `state.with_conn` / `with_conn_mut`
- Modify: `src/App.svelte` — branch on `bootStatus()`
- Modify: `src/lib/api/index.ts` — export boot API
- Modify: `src-tauri/src/commands/settings.rs` `get_db_path` — works in recovery via `AppState::db_path`

**Review:** Bug 2 (product half). Spec §6.2 / §11: show “JSON エクスポートから復元してください”. `import_json` today requires an open DB.

**Interfaces:**
- Consumes: `boot()`, `backup::import_snapshot_json`, `ImportResult`
- Produces: `boot_status`, `recover_import_json`, `recover_start_empty`; frontend Recovery route

**Quarantine / new file protocol (must follow exactly):**

`recover_import_json`:
1. Inner must be `Recovery`. Else `AppError::InvalidArgument`.
2. Write a new file `data.db.new` in `data_dir`. Do **not** move `data.db` yet.
3. Obtain a key: `get()` if `Some` and 32 bytes, use it. If missing/corrupt: `delete()` then `create()`.
4. `open_encrypted(data.db.new)`, migrate, seed, `import_snapshot_json(..., "overwrite")`.
5. On import **failure**: delete `data.db.new`, leave `Recovery` unchanged, return the error. Original `data.db` is untouched.
6. On import **success**: rename `data.db` → `data.db.corrupt-<UTC stamp>` if it exists; rename `data.db.new` → `data.db`; set inner to `Ready`.

`recover_start_empty`:
1. Inner must be `Recovery`.
2. If `data.db` exists, rename to `data.db.corrupt-<UTC stamp>`.
3. Same key rules as step 3 above.
4. Create `data.db`, migrate, seed, `Ready`.

- [ ] **Step 1: Add `AppError::Unavailable` test**

```rust
#[error("unavailable: {0}")]
Unavailable(String),
```

Test: `assert_eq!(AppError::Unavailable("recovery".into()).to_string(), "unavailable: recovery");`

- [ ] **Step 2: Run to see it fail, then add the variant and pass**

Run: `cd src-tauri && cargo test --lib error`

- [ ] **Step 3: Replace `AppState` and add helpers**

In `commands/meta.rs`:

```rust
pub enum AppInner {
    Ready { conn: Connection, db_path: PathBuf },
    Recovery {
        reason: crate::infra::boot::RecoveryReason,
        data_dir: PathBuf,
        db_path: PathBuf,
    },
}

pub struct AppState {
    pub inner: Mutex<AppInner>,
}

impl AppState {
    pub fn db_path(&self) -> AppResult<PathBuf> {
        let inner = self.inner.lock().map_err(|_| {
            AppError::Corrupt("connection mutex poisoned".into())
        })?;
        match &*inner {
            AppInner::Ready { db_path, .. } | AppInner::Recovery { db_path, .. } => {
                Ok(db_path.clone())
            }
        }
    }

    pub fn with_conn<R>(&self, f: impl FnOnce(&Connection) -> AppResult<R>) -> AppResult<R> {
        let inner = self.inner.lock().map_err(|_| {
            AppError::Corrupt("connection mutex poisoned".into())
        })?;
        match &*inner {
            AppInner::Ready { conn, .. } => f(conn),
            AppInner::Recovery { reason, .. } => Err(AppError::Unavailable(format!(
                "database is in recovery ({reason:?})"
            ))),
        }
    }

    pub fn with_conn_mut<R>(
        &self,
        f: impl FnOnce(&mut Connection) -> AppResult<R>,
    ) -> AppResult<R> {
        let mut inner = self.inner.lock().map_err(|_| {
            AppError::Corrupt("connection mutex poisoned".into())
        })?;
        match &mut *inner {
            AppInner::Ready { conn, .. } => f(conn),
            AppInner::Recovery { reason, .. } => Err(AppError::Unavailable(format!(
                "database is in recovery ({reason:?})"
            ))),
        }
    }
}
```

Convert each command. Read-only example (`list_balances`):

```rust
state.with_conn(|conn| balance_repo::list_balances(conn))
```

Mutating example (`create_transaction` body after validation):

```rust
state.with_conn_mut(|conn| {
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let id = transaction_repo::insert(&tx, &input)?;
    let row = transaction_repo::find_by_id(&tx, id)?;
    tx.commit()?;
    Ok(row)
})
```

`get_db_path`: `Ok(state.db_path()?.to_string_lossy().into_owned())`.

`app_info`: if Recovery, return `schema_version: 0` and `db_path` without opening SQLite (or keep `app_info` Ready-only and let the UI call `boot_status` first — **do the latter**). `app_info` uses `with_conn`.

Files that lock `state.conn` today (replace all): `commands/{meta,categories,accounts,transactions,reports,backup,settings,balances,budgets}.rs`.

- [ ] **Step 4: Wire `lib.rs` setup without `expect` on decrypt**

```rust
.setup(|app| {
    let data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let keys = crate::infra::boot::OsKeyStore {
        service: crate::infra::boot::KEYCHAIN_SERVICE.to_string(),
        account: crate::infra::boot::KEYCHAIN_ACCOUNT.to_string(),
    };
    let outcome = crate::infra::boot::boot(&data_dir, &keys).map_err(|e| e.to_string())?;
    let inner = match outcome {
        crate::infra::boot::BootOutcome::Ready { conn, db_path } => {
            AppInner::Ready { conn, db_path }
        }
        crate::infra::boot::BootOutcome::Recovery { reason, data_dir, db_path } => {
            AppInner::Recovery { reason, data_dir, db_path }
        }
    };
    app.manage(AppState { inner: Mutex::new(inner) });
    Ok(())
})
```

`KEYCHAIN_SERVICE` / `KEYCHAIN_ACCOUNT` / `DB_FILENAME` already live in `infra/boot.rs` from Task 2. `lib.rs` must import those constants; do not re-declare them.

Register `boot_status`, `recover_import_json`, `recover_start_empty`.

Setup still returns `Err` (Tauri will not show a window) only for I/O that prevents creating `data_dir`. Decrypt/key failures must **not** fail `setup`.

- [ ] **Step 5: Write recovery command tests (integration)**

`src-tauri/tests/integration_recovery.rs` should call a **library function**, not the Tauri command:

```rust
pub fn recover_import(
    inner: &Mutex<AppInner>,
    keys: &dyn KeyStore,
    payload: &str,
) -> AppResult<ImportResult>;

pub fn recover_start_empty(
    inner: &Mutex<AppInner>,
    keys: &dyn KeyStore,
) -> AppResult<()>;
```

Tests:
1. DecryptFailed + valid JSON → Ready, original file renamed `data.db.corrupt-*`, imported accounts present.
2. DecryptFailed + invalid JSON → still Recovery, `data.db` byte-identical, no `data.db.new` left.
3. KeyMissing + existing placeholder file → `recover_start_empty` creates a key (create_calls == 1) and Ready.
4. `import_json` while Recovery → `Unavailable`.

Reuse `export_snapshot_json` from a throwaway in-memory DB to build a valid payload (see `integration_backup.rs`).

- [ ] **Step 6: Run recovery tests RED, then implement `commands/recovery.rs`**

Run: `cd src-tauri && cargo test --test integration_recovery -- --nocapture`

Implement quarantine names with UTC stamp `data.db.corrupt-20260829T123045Z`. If the target exists, append `-2`, `-3`.

- [ ] **Step 7: Frontend API + Recovery screen**

`src/lib/api/boot.ts`:

```ts
import { invoke } from '@tauri-apps/api/core';
import type { ImportResult } from './backup';

export type RecoveryReason =
  | 'key_missing'
  | 'decrypt_failed'
  | 'key_corrupt'
  | 'keychain_error';

export type BootStatus = {
  state: 'ready' | 'recovery';
  recovery_reason: RecoveryReason | null;
  db_path: string;
};

export function bootStatus(): Promise<BootStatus> {
  return invoke('boot_status');
}

export function recoverImportJson(payload: string): Promise<ImportResult> {
  return invoke('recover_import_json', { payload });
}

export function recoverStartEmpty(): Promise<void> {
  return invoke('recover_start_empty');
}
```

Vitest: assert invoke names and payloads.

`Recovery.svelte` copy (Japanese, matching spec §6.2):

- Title: `データベースを開けません`
- Body: `保存データと鍵が一致しないか、鍵が見つかりません。壊れたファイルは残します。JSON バックアップから復元するか、空の家計簿でやり直してください。`
- Show `db_path`
- File input → `recoverImportJson`
- Button `空の家計簿で始める` → `confirm(...)` then `recoverStartEmpty`
- On success: `window.location.reload()` (simplest way to remount stores against Ready)

`App.svelte`:

```svelte
let boot = $state<BootStatus | null>(null);
let bootError = $state<string | null>(null);

onMount(() => {
  void bootStatus()
    .then((status) => { boot = status; })
    .catch((e) => { bootError = e instanceof Error ? e.message : String(e); });
});
```

If `bootError`, show it. If `boot?.state === 'recovery'`, render `<Recovery reason={boot.recovery_reason} dbPath={boot.db_path} />` **without** the normal sidebar shell (no live queries). If `ready`, current shell.

Browser E2E has no real boot. `App.svelte` will call `boot_status` on mount. If the mock is missing, `boot` stays `null` and smoke never sees `page-dashboard`.

Create `tests/e2e/tauriMock.ts` and use it from **every** spec, including `smoke.spec.ts` (that file has **no** `addInitScript` today):

```ts
// tests/e2e/tauriMock.ts
import type { Page } from '@playwright/test';

export async function installReadyBootMock(page: Page): Promise<void> {
  await page.addInitScript(() => {
    const internals = (window as any).__TAURI_INTERNALS__ ?? {};
    const previous = internals.invoke;
    internals.invoke = async (command: string, args: any) => {
      if (command === 'boot_status') {
        return { state: 'ready', recovery_reason: null, db_path: '/tmp/data.db' };
      }
      // Dashboard mounts on `/` and `/` is the smoke landing page.
      // Task 3 runs before Task 13. Current createBalancesStore assigns the
      // invoke result to `items` and calls `items.filter(...)`. Return an
      // array until Task 13 changes list_balances to `{ accounts, total_assets }`.
      if (command === 'list_balances') {
        return [];
      }
      if (command === 'list_budget_statuses' || command === 'list_top_budget_statuses') {
        return [];
      }
      if (command === 'monthly_summary') {
        return { income: 0, expense: 0, net: 0, by_category: [] };
      }
      if (command === 'monthly_series') {
        return [];
      }
      if (command === 'list_transactions') {
        return { items: [], total: 0 };
      }
      if (command === 'list_categories' || command === 'list_accounts') {
        return [];
      }
      if (typeof previous === 'function') return previous(command, args);
      return null;
    };
    (window as any).__TAURI_INTERNALS__ = internals;
  });
}
```

`smoke.spec.ts`:

```ts
test('app shell renders with dashboard route', async ({ page }) => {
  await installReadyBootMock(page);
  // existing assertions
});
```

Also add `case 'boot_status':` to the existing `invoke` switches in `transaction-flow.spec.ts`, `budget-flow.spec.ts`, and `transfer-flow.spec.ts` (those files replace `__TAURI_INTERNALS__` wholesale, so the helper above is not enough by itself). Return `{ state: 'ready', recovery_reason: null, db_path: '/tmp/data.db' }`.

Default `null` from `default:` currently returns `null` for unknown commands — after this task, a missing `boot_status` case leaves the shell blank.

- [ ] **Step 8: Recovery E2E (mocked)**

`tests/e2e/recovery-flow.spec.ts`: init script `boot_status` returns recovery; assert heading `データベースを開けません`; assert JSON file input and empty-start button exist; stub `recover_import_json` / `recover_start_empty`.

- [ ] **Step 9: Run verification**

```bash
cd src-tauri && cargo test --no-fail-fast && cargo clippy --all-targets -- -D warnings
pnpm test && pnpm check && pnpm test:e2e
```

Expected: all green.

- [ ] **Step 10: Commit**

```bash
git add src-tauri/src/lib.rs src-tauri/src/error.rs src-tauri/src/commands \
  src-tauri/tests/integration_recovery.rs src/App.svelte src/lib/api \
  src/routes/Recovery.svelte tests/e2e tests/e2e/tauriMock.ts tests/e2e/smoke.spec.ts
git commit -m "$(cat <<'EOF'
feat: recover from decrypt failure via JSON import without minting a silent key

Setup no longer panics before the window exists. A mismatched data.db is
quarantined only after a new encrypted file imports successfully.
EOF
)"
```

---

### Task 4: Keep archived account/category IDs when editing history

**Files:**
- Modify: `src/routes/Transactions.svelte` (stores, options, `$effect`, `labelForRow`)
- Modify: `tests/e2e/transaction-flow.spec.ts` only if the create-path still works (no remap on create)

**Review:** Bug 3. `createCategoriesStore({ include_archived: false })` + `$effect` remaps missing select values to the first visible option. Saving persists the wrong master. List labels become `-`.

**Interfaces:**
- Consumes: `list_categories` / `list_accounts` already support include-archived
- Produces: no API change. Create path still offers active masters only.

- [ ] **Step 1: Add a focused Vitest if a helper is extracted; otherwise cover via E2E + the effect guard**

Extract a pure helper so TDD is possible:

`src/lib/utils/selectOptions.ts`:

```ts
export type Named = { id: number; name: string; archived_at?: string | null };

export function optionsWithCurrent(
  items: Named[],
  currentId: string,
  includeArchived: boolean,
): { value: string; label: string }[] {
  const visible = items.filter((item) => includeArchived || !item.archived_at);
  const options = visible.map((item) => ({
    value: String(item.id),
    label: item.archived_at ? `${item.name} (アーカイブ済)` : item.name,
  }));
  if (currentId && !options.some((option) => option.value === currentId)) {
    const missing = items.find((item) => String(item.id) === currentId);
    if (missing) {
      options.unshift({
        value: String(missing.id),
        label: `${missing.name} (アーカイブ済)`,
      });
    }
  }
  return options;
}
```

Test:

```ts
it('keeps the current archived id and does not replace it with the first active', () => {
  const items = [
    { id: 1, name: '現金', archived_at: null },
    { id: 2, name: '旧口座', archived_at: '2026-01-01T00:00:00Z' },
  ];
  const options = optionsWithCurrent(items, '2', false);
  expect(options.map((o) => o.value)).toEqual(['2', '1']);
  expect(options[0].label).toContain('アーカイブ済');
});

it('create path (empty current) lists only active', () => {
  const items = [
    { id: 1, name: '現金', archived_at: null },
    { id: 2, name: '旧口座', archived_at: '2026-01-01T00:00:00Z' },
  ];
  expect(optionsWithCurrent(items, '', false).map((o) => o.value)).toEqual(['1']);
});
```

- [ ] **Step 2: Run helper test RED, then implement helper GREEN**

- [ ] **Step 3: Wire Transactions.svelte**

Changes:
1. `createCategoriesStore({ include_archived: true })` and `createAccountsStore(true)` (accounts store already true — keep it). Categories currently `include_archived: false` — switch to `true`.
2. Build `categoryOptions` / `accountOptions` / `counterAccountOptions` with `optionsWithCurrent` using `formCategory` / `formAccount` / `formCounterAccount`. Filter categories by `formType` **before** the helper. For create (`editing === null`) pass `currentId = ''` so archived are omitted. For edit, pass the form field.
3. Replace the `$effect` so it **returns immediately when `editing` is set**. Remap first-option only for create, and when `formType` changes on create:

```ts
$effect(() => {
  if (!modalOpen || editing) return;
  if (formType !== 'transfer' && !categoryOptions.some((option) => option.value === formCategory)) {
    formCategory = firstCategoryFor(formType);
  }
  if (!accountOptions.some((option) => option.value === formAccount)) {
    formAccount = accountOptions[0]?.value ?? '';
  }
  if (
    formType === 'transfer' &&
    !counterAccountOptions.some((option) => option.value === formCounterAccount)
  ) {
    formCounterAccount = counterAccountOptions[0]?.value ?? '';
  }
});
```

4. `labelForRow` already uses `categoryById` / `accountById` from store items — with `include_archived: true` the names resolve. Keep `'?'` / `'-'` only when the id is truly missing.

- [ ] **Step 4: `pnpm test` && `pnpm check`**

Expected: PASS. Create-transaction E2E still adds a row.

- [ ] **Step 5: Commit**

```bash
git add src/lib/utils/selectOptions.ts src/lib/utils/selectOptions.test.ts \
  src/routes/Transactions.svelte
git commit -m "$(cat <<'EOF'
fix: keep archived account and category ids when editing history

The edit modal remapped missing select values to the first active
master, so saving silently rewrote past transactions.
EOF
)"
```

---

## Wave B — Error surfaces and races (Bugs 4–8, 22)

### Task 5: Show list and mutation errors instead of empty ledgers

**Files:**
- Create: `src/lib/components/ErrorBanner.svelte`
- Modify: `src/routes/Transactions.svelte` (`txStore.error`, `remove` try/catch, empty state guard)
- Modify: `src/routes/Categories.svelte` (`store.error`, `toggleArchive` try/catch)
- Modify: `src/routes/Accounts.svelte` (same)
- Modify: `src/routes/Budgets.svelte` (already shows `store.error` — reuse banner)

**Review:** Bugs 4 and 5.

**Empty-state rule:** show `EmptyState` only when `!loading && !error && items.length === 0`.

- [ ] **Step 1: Add `ErrorBanner.svelte`**

```svelte
<script lang="ts">
  let { message }: { message: string } = $props();
</script>

<p class="error" role="alert" data-testid="page-error">{message}</p>

<style>
  .error {
    color: var(--danger);
    font-weight: 700;
  }
</style>
```

- [ ] **Step 2: Transactions list + delete**

```svelte
{#if txStore.error}
  <ErrorBanner message={txStore.error} />
{:else if txStore.items.length === 0 && !txStore.loading}
  <EmptyState title="該当する取引がありません" hint="右上から取引を追加できます" />
{:else}
  <!-- table -->
{/if}
```

Pager buttons: `disabled={txStore.loading || txStore.page === 0}` (and next similarly). Full pager disable-while-loading is completed in Task 6; here at least do not treat error as empty.

```ts
let actionError = $state<string | null>(null);

async function remove(transaction: Transaction) {
  actionError = null;
  if (!confirm(`「${transaction.description || '取引'}」を削除しますか?`)) return;
  try {
    await deleteTransaction(transaction.id);
  } catch (e) {
    actionError = e instanceof Error ? e.message : String(e);
  }
}
```

Render `actionError` with `ErrorBanner` above the table. Confirm stays confirmation-only.

- [ ] **Step 3: Categories and Accounts**

Same pattern for `store.error` vs empty, and:

Categories (`src/routes/Categories.svelte`):

```ts
let actionError = $state<string | null>(null);
async function toggleArchive(category: Category) {
  actionError = null;
  try {
    if (category.archived_at) await unarchiveCategory(category.id);
    else await archiveCategory(category.id);
  } catch (e) {
    actionError = e instanceof Error ? e.message : String(e);
  }
}
```

Accounts (`src/routes/Accounts.svelte`):

```ts
let actionError = $state<string | null>(null);
async function toggleArchive(account: Account) {
  actionError = null;
  try {
    if (account.archived_at) await unarchiveAccount(account.id);
    else await archiveAccount(account.id);
  } catch (e) {
    actionError = e instanceof Error ? e.message : String(e);
  }
}
```

Categories currently shows EmptyState when `visible.length === 0` even if `store.error` is set — fix that.

- [ ] **Step 4: `pnpm check`**

- [ ] **Step 5: Commit**

```bash
git commit -m "$(cat <<'EOF'
fix: surface list and archive errors instead of empty ledgers

Failed list_transactions left items [] so the UI looked like a blank
book. Delete and archive rejections were unhandled.
EOF
)"
```

---

### Task 6: Ignore stale store responses

**Files:**
- Modify: `src/lib/stores/transactions.svelte.ts`
- Modify: `src/lib/stores/accounts.svelte.ts`
- Modify: `src/lib/stores/categories.svelte.ts`
- Modify: `src/lib/stores/budgets.svelte.ts`
- Modify: `src/lib/stores/balances.svelte.ts`
- Create: `src/lib/stores/transactions.test.ts` (and extend `budgets.test.ts` / `categories.test.ts`)

**Review:** Bug 6. Parallel `void load()` from `setFilter` / `setPage` / `data:changed` can apply an older response last.

**Pattern (copy into each store `load`):**

```ts
let requestId = 0;

async function load() {
  const id = ++requestId;
  loading = true;
  error = null;
  try {
    const res = await listTransactions(filter, page, pageSize);
    if (id !== requestId) return;
    items = res.items;
    total = res.total;
  } catch (e) {
    if (id !== requestId) return;
    error = e instanceof Error ? e.message : String(e);
  } finally {
    if (id === requestId) loading = false;
  }
}
```

- [ ] **Step 1: Failing transactions store test**

```ts
it('ignores a stale list response', async () => {
  let resolveOld: (value: unknown) => void = () => {};
  const oldPromise = new Promise((resolve) => {
    resolveOld = resolve;
  });
  listMock.mockImplementationOnce(() => oldPromise);
  listMock.mockResolvedValueOnce({
    items: [{ id: 2, occurred_on: '2026-05-02', type: 'expense', amount: 2, account_id: 1, counter_account_id: null, category_id: 1, description: 'new', recurring_id: null, created_at: '', updated_at: '' }],
    total: 1,
  });

  const store = createTransactionsStore({}, 50);
  await vi.waitFor(() => expect(listMock).toHaveBeenCalledTimes(1));

  store.setPage(1);
  await vi.waitFor(() => expect(store.items[0]?.id).toBe(2));

  resolveOld({
    items: [{ id: 1, occurred_on: '2026-05-01', type: 'expense', amount: 1, account_id: 1, counter_account_id: null, category_id: 1, description: 'old', recurring_id: null, created_at: '', updated_at: '' }],
    total: 1,
  });
  await new Promise((r) => setTimeout(r, 20));
  expect(store.items[0]?.id).toBe(2);
  await store.dispose();
});
```

- [ ] **Step 2: Run RED, implement requestId in all five stores, GREEN**

Run: `pnpm exec vitest run src/lib/stores/transactions.test.ts`

Apply the same `requestId` to accounts, categories, budgets, balances. Add one stale-response test to `budgets.test.ts` (two overlapping `setYearMonth` calls) so budgets is not left untested.

Disable pager while `txStore.loading` in `Transactions.svelte`.

- [ ] **Step 3: Commit**

```bash
git commit -m "$(cat <<'EOF'
fix: drop stale store responses so paging cannot rewind the ledger

setFilter, setPage, and data:changed all called load() without a
generation counter, so an older list could overwrite the current page.
EOF
)"
```

---

### Task 7: Dashboard reload is atomic and disposed on unmount

**Files:**
- Modify: `src/routes/Dashboard.svelte`

**Review:** Bugs 7 and 22.

- [ ] **Step 1: Add a `disposed` flag and snapshot on failure**

```ts
let disposed = false;
let reloadId = 0;

async function reload() {
  const id = ++reloadId;
  error = null;
  try {
    const [nextSummary, nextSeries, nextRecent, nextBudgets] = await Promise.all([
      monthlySummary(currentYear, currentMonth),
      monthlySeries(12),
      listTransactions({}, 0, 10),
      listBudgetStatuses(currentYearMonth), // replaced in Task 13 with listTopBudgetStatuses
    ]);
    if (disposed || id !== reloadId) return;
    summary = nextSummary;
    series = nextSeries;
    recent = nextRecent.items;
    budgetStatuses = nextBudgets;
    drawChart();
  } catch (e) {
    if (disposed || id !== reloadId) return;
    error = e instanceof Error ? e.message : String(e);
    summary = null;
    series = [];
    recent = [];
    budgetStatuses = [];
    drawChart();
  }
}

function drawChart() {
  if (disposed || !canvas) return;
  // existing Chart.js body
}
```

`onMount`:

```ts
onMount(() => {
  void (async () => {
    await reload();
    if (disposed) return;
    try {
      const nextUnlisten = await onDataChanged((domain) => {
        if (disposed) return;
        if (
          domain === 'transactions' ||
          domain === 'categories' ||
          domain === 'accounts' ||
          domain === 'budgets'
        ) {
          void reload();
        }
      });
      if (disposed) nextUnlisten();
      else unlisten = nextUnlisten;
    } catch {
      // Browser-only E2E has no Tauri event bus.
    }
  })();
});

onDestroy(() => {
  disposed = true;
  chart?.destroy();
  chart = null;
  unlisten?.();
  unlisten = null;
  void catStore.dispose();
  void balancesStore.dispose();
});
```

Cards already show `---` when `summary` is null — after this catch, a failed refresh must not keep the previous income/expense/net.

- [ ] **Step 2: `pnpm check`**

- [ ] **Step 3: Commit**

```bash
git commit -m "$(cat <<'EOF'
fix: clear dashboard summary on reload failure and ignore unmount races

A partial Promise.all catch left the previous P/L cards visible under
the error banner, and reload/drawChart could run after destroy.
EOF
)"
```

---

### Task 8: Archived accounts show computed balance

**Files:**
- Modify: `src/routes/Accounts.svelte:161`

**Review:** Bug 8. Archived list uses `account.initial_balance`. `balanceById` already has computed balances from `list_balances` (includes archived).

- [ ] **Step 1: Reuse the active-row balance cell for archived rows**

Replace:

```svelte
<span class="balance"><strong>{yen.format(account.initial_balance)}</strong></span>
```

with the same error / computed-balance block used for visible accounts (`balances.error` → `取得できません`, else `balanceById.get(account.id)`). **Do not** fall back to `initial_balance` when the map is missing: if `!balances.loading && !balances.error && !balanceById.has(account.id)` show `取得できません`. While `balances.loading`, show a muted `…` (not the opening balance).

Add `data-testid={`account-balance-${account.id}`}` on archived rows too.

- [ ] **Step 2: `pnpm check`**

- [ ] **Step 3: Commit**

```bash
git commit -m "$(cat <<'EOF'
fix: show computed balance for archived accounts

The archived list printed initial_balance, which diverged after
income, expense, or transfer activity.
EOF
)"
```

---

## Wave C — Domain and command hardening

### Task 9: Turn FOREIGN KEY on in `migrations::run`

**Files:**
- Modify: `src-tauri/src/infra/migrations.rs:91` (start of `run`)
- Modify: `src-tauri/tests/integration_transactions.rs` (add a test that missing parent fails)

**Review:** Suggestion 19. V001 `PRAGMA foreign_keys = ON` inside a migration transaction is a no-op. Production `open_encrypted` sets it; in-memory tests do not.

- [ ] **Step 1: Write a failing integration test**

In `integration_transactions.rs`:

```rust
#[test]
fn insert_rejects_missing_account_fk() {
    let (conn, _acc, cat) = seeded_db();
    let err = transaction_repo::insert(
        &conn,
        &transaction_repo::InsertInput {
            occurred_on: "2026-05-25",
            type_: TxType::Expense,
            amount: 100,
            account_id: 9_999_999,
            category_id: cat,
            description: "ghost",
            now: NOW,
        },
    )
    .unwrap_err();
    assert!(
        matches!(err, budget_tracker_lib::error::AppError::Db(_)),
        "got {err:?}"
    );
}
```

`seeded_db` already calls `migrations::run`. Without FK ON, this insert **succeeds**.

- [ ] **Step 2: Run RED**

Run: `cd src-tauri && cargo test --test integration_transactions insert_rejects_missing_account_fk -- --nocapture`

Expected: FAIL (insert Ok).

- [ ] **Step 3: Enable FK at the start of `migrations::run`**

```rust
pub fn run(conn: &mut Connection) -> AppResult<u32> {
    conn.pragma_update(None, "foreign_keys", "ON")?;
    let mut version = current_version(conn)?;
    // existing loop
}
```

This is outside any migration transaction, so it takes effect. Production `open_encrypted` already sets the same pragma; setting it twice is fine.

- [ ] **Step 4: Run `cargo test --no-fail-fast`**

Expected: PASS. If an existing test inserted orphan rows, fix the test data (do not disable FK).

- [ ] **Step 5: Commit**

```bash
git commit -m "$(cat <<'EOF'
test: enable SQLite foreign keys in migrations::run

In-memory integration tests inherited FK-off, so missing parents
were not caught. Production already sets the pragma in open_encrypted.
EOF
)"
```

---

### Task 10: Escape LIKE wildcards and cap `page_size`

**Files:**
- Modify: `src-tauri/src/infra/repo/transaction_repo.rs` (`build_where`, `list`)
- Modify: `src-tauri/src/commands/transactions.rs` (`list_transactions`)
- Modify: `src-tauri/tests/integration_transactions.rs`

**Review:** Suggestion 18.

- [ ] **Step 1: Failing tests**

```rust
#[test]
fn search_treats_percent_as_literal() {
    let (conn, acc, cat) = seeded_db();
    transaction_repo::insert(&conn, &transaction_repo::InsertInput {
        occurred_on: "2026-05-10",
        type_: TxType::Expense,
        amount: 100,
        account_id: acc,
        category_id: cat,
        description: "100%オフ",
        now: NOW,
    }).unwrap();
    transaction_repo::insert(&conn, &transaction_repo::InsertInput {
        occurred_on: "2026-05-11",
        type_: TxType::Expense,
        amount: 100,
        account_id: acc,
        category_id: cat,
        description: "ランチ",
        now: NOW,
    }).unwrap();
    let filter = transaction_repo::ListFilter {
        search: Some("%".into()),
        ..Default::default()
    };
    let (items, total) = transaction_repo::list(&conn, &filter, 0, 50).unwrap();
    assert_eq!(total, 1);
    assert_eq!(items[0].description, "100%オフ");
}

#[test]
fn list_transactions_command_rejects_page_size_over_200() {
    // If you do not want to construct AppState, test a helper:
    // crate::commands::transactions::clamp_page_size(201) -> Err
}
```

Add `pub fn parse_page_size(page_size: u32) -> AppResult<u32>` in `commands/transactions.rs`:

```rust
pub fn parse_page_size(page_size: u32) -> AppResult<u32> {
    if !(1..=200).contains(&page_size) {
        return Err(AppError::InvalidArgument(format!(
            "page_size must be 1..=200, got {page_size}"
        )));
    }
    Ok(page_size)
}
```

Unit test in `commands/transactions.rs` `mod tests`: `parse_page_size(0)` and `parse_page_size(201)` err; `50` ok.

- [ ] **Step 2: RED, then implement**

LIKE:

```rust
fn escape_like(raw: &str) -> String {
    raw.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_")
}

// in build_where:
clauses.push("description LIKE ? ESCAPE '\\'".into());
binds.push(Box::new(format!("%{}%", escape_like(q))));
```

LIMIT/OFFSET as bound parameters (not interpolated):

```rust
let list_sql = format!(
    "SELECT ... FROM transactions{where_sql}
      ORDER BY occurred_on DESC, id DESC
      LIMIT ? OFFSET ?"
);
let mut binds_with_page = binds;
binds_with_page.push(Box::new(limit as i64));
binds_with_page.push(Box::new(offset as i64));
```

`list_transactions` command:

```rust
let page_size = parse_page_size(page_size)?;
```

- [ ] **Step 3: `cargo test --test integration_transactions` + `cargo test --lib commands::transactions`**

- [ ] **Step 4: Commit**

```bash
git commit -m "$(cat <<'EOF'
fix: treat LIKE metacharacters as literals and cap transaction page size

Search bound the pattern but still honored % and _, and page_size had
no upper bound so a single invoke could materialize the whole ledger.
EOF
)"
```

---

### Task 11: Validate category type, existence, archive retarget, and transfer updates

**Files:**
- Modify: `src-tauri/src/domain/ledger.rs` (new `assert_*_refs`, transfer error copy)
- Modify: `src-tauri/src/commands/transactions.rs` (load account/category, call assert)
- Modify: `src-tauri/src/infra/repo/transaction_repo.rs` (`update` WHERE `type != 'transfer'`)
- Modify: `src-tauri/tests/integration_transactions.rs`, `integration_transfers.rs`

**Review:** Suggestions 16 and 17. Reconcile with Bug 3: **same** archived id on update is allowed; **new** archived id is not; missing FK → `NotFound` not raw constraint text.

- [ ] **Step 1: Domain tests for refs**

```rust
pub struct AllowedArchivedRefs {
    pub account_id: Option<i64>,
    pub category_id: Option<i64>,
    pub counter_account_id: Option<i64>,
}

impl AllowedArchivedRefs {
    pub fn none() -> Self {
        Self {
            account_id: None,
            category_id: None,
            counter_account_id: None,
        }
    }
}

pub fn assert_account_writable(
    account: &crate::domain::account::Account,
    allow_id: Option<i64>,
) -> AppResult<()> {
    if account.archived_at.is_some() && allow_id != Some(account.id) {
        return Err(AppError::InvalidArgument(format!(
            "account {} is archived",
            account.id
        )));
    }
    Ok(())
}

pub fn assert_category_matches_tx(
    category: &crate::domain::category::Category,
    tx_type: TxType,
    allow_id: Option<i64>,
) -> AppResult<()> {
    if category.archived_at.is_some() && allow_id != Some(category.id) {
        return Err(AppError::InvalidArgument(format!(
            "category {} is archived",
            category.id
        )));
    }
    let expected = match tx_type {
        TxType::Income => crate::domain::category::CategoryType::Income,
        TxType::Expense => crate::domain::category::CategoryType::Expense,
        TxType::Transfer => {
            return Err(AppError::InvalidArgument(
                "use create_transfer or update_transfer for transfer rows".into(),
            ));
        }
    };
    if category.type_ != expected {
        return Err(AppError::InvalidArgument(format!(
            "category {} type does not match transaction type",
            category.id
        )));
    }
    Ok(())
}
```

Tests:
- expense + income category → err
- archived account on create (`allow_id = None`) → err
- archived account with `allow_id = Some(same id)` → ok
- archived account with `allow_id = Some(other id)` → err

Change `validate_input` transfer message from `"transfer transactions are not supported in Phase 2"` to `"use create_transfer or update_transfer for transfer rows"`. Update `rejects_transfer_type` to assert the new substring.

- [ ] **Step 2: RED domain tests, implement, GREEN**

- [ ] **Step 3: Command integration tests**

```rust
#[test]
fn create_expense_with_income_category_is_rejected() { /* seed income cat, create expense */ }

#[test]
fn update_transaction_rejects_existing_transfer_row() { /* insert_transfer then update_transaction */ }

#[test]
fn create_with_missing_account_is_not_found() { /* account_id 999 after FK ON */ }
```

Command helper (in `commands/transactions.rs`):

```rust
fn load_account(conn: &Connection, id: i64) -> AppResult<crate::domain::account::Account> {
    account_repo::find_by_id(conn, id)
}

fn prepare_income_expense(
    conn: &Connection,
    validated: &ledger::ValidatedInput,
    allow: ledger::AllowedArchivedRefs,
) -> AppResult<()> {
    let account = account_repo::find_by_id(conn, validated.account_id)?;
    ledger::assert_account_writable(&account, allow.account_id)?;
    let category = category_repo::find_by_id(conn, validated.category_id)?;
    ledger::assert_category_matches_tx(&category, validated.type_, allow.category_id)?;
    Ok(())
}
```

`create_transaction`: `allow = AllowedArchivedRefs::none()`.
`update_transaction`: load existing row first; if `existing.type_ == Transfer`, return the `update_transfer` InvalidArgument; `allow` uses existing ids.

`create_transfer` / `update_transfer`: load both accounts; writable with allow on update.

`transaction_repo::update` SQL: add `AND type != 'transfer'` so a direct repo call cannot rewrite a transfer. If `n == 0`, `find_by_id`; if type is transfer → `InvalidArgument("use update_transfer ...")`; else `NotFound`.

- [ ] **Step 4: `cargo test --test integration_transactions --test integration_transfers`**

- [ ] **Step 5: Commit**

```bash
git commit -m "$(cat <<'EOF'
fix: reject mismatched categories, archived retargets, and transfer rewrites

Commands trusted the UI to send a live matching master, so an invoke
could attach income to an expense category or turn a transfer into
an expense. Updates may keep the row's current archived ids.
EOF
)"
```

---

### Task 12: Backup format version and row validation

**Files:**
- Modify: `src-tauri/src/commands/backup.rs`
- Modify: `src-tauri/src/error.rs` (`ImportValidation`)
- Modify: `src-tauri/tests/integration_backup.rs`
- Modify: parent spec in Task 16 (mention only; do not edit spec here except if you must — **defer spec text to Task 16**)

**Review:** Suggestion 11.

Rename `SUPPORTED_SCHEMA_VERSION` → `BACKUP_FORMAT_VERSION: u32 = 1`. JSON field remains `schema_version` (format version). Keep accepting `1`. Reject `3` with message `unsupported backup format version 3 (expected 1); this is not app_meta.schema_version`.

- [ ] **Step 1: Error type**

```rust
#[derive(Debug, Clone, Serialize)]
pub struct ImportRowError {
    pub index: u32,
    pub entity: String,
    pub message: String,
}

#[error("import validation failed: {0}")]
ImportValidation(String),
```

Helper: join errors as `transaction[2]: amount must be positive; transfer[4]: source and destination must differ`.

Serialize `AppError` still as a string (existing `Serialize` impl). Tests assert `to_string()` contains `transaction[`.

- [ ] **Step 2: Pre-validate before mutating**

After JSON parse and format-version check, **before** `DELETE` / inserts, walk arrays and collect errors:

- account: `account::validate_name`, `AccountKind::parse` (no default `"cash"`), `initial_balance` required integer (no `unwrap_or(0)` for required fields)
- category: `category::validate_name`, `CategoryType::parse`
- transaction income/expense: `ledger::validate_input`
- transaction transfer: `ledger::validate_transfer_input` (same-account transfer fails here)
- budget: `budget::validate_set_budget_input` with `year_month` derived from `starts_on` (`YYYY-MM-DD` → first 7 chars)

If `errors` is non-empty, return `Err(AppError::ImportValidation(joined))` without opening the write transaction.

Keep one Immediate transaction for the actual writes. Rollback on any insert error remains.

Do not invent missing names/kinds/amounts. Missing required fields → row error.

- [ ] **Step 3: Tests**

```rust
#[test]
fn import_rejects_backup_format_version_matching_db_schema() {
    // schema_version: 3 → error mentions backup format
}

#[test]
fn import_rejects_same_account_transfer_before_commit() {
    // build snapshot with a transfer account_id == counter_account_id
    // existing rows remain (append mode) / empty (overwrite on empty db)
}

#[test]
fn import_rejects_empty_account_name() { /* ... */ }
```

Official export still writes `"schema_version": 1`. Round-trip test in `integration_backup.rs` must stay green.

- [ ] **Step 4: GREEN + commit**

```bash
git commit -m "$(cat <<'EOF'
fix: validate backup rows with domain rules and separate format version

JSON schema_version 1 is the backup format, not app_meta.schema_version.
Import defaulted kind/amount and skipped transfer identity checks.
EOF
)"
```

---

### Task 13: Compute total assets and top budgets in Rust

**Files:**
- Modify: `src-tauri/src/domain/balance.rs` (`total_assets`)
- Modify: `src-tauri/src/infra/repo/balance_repo.rs` or `commands/balances.rs` (wrap list)
- Modify: `src-tauri/src/domain/budget.rs` (`select_top_statuses`)
- Modify: `src-tauri/src/commands/budgets.rs` (`list_top_budget_statuses`)
- Modify: `src-tauri/src/lib.rs` (register command)
- Modify: `src/lib/api/balances.ts`, `src/lib/api/balances.test.ts`
- Modify: `src/lib/api/budgets.ts`, `src/lib/api/budgets.test.ts`
- Modify: `src/lib/stores/balances.svelte.ts`
- Modify: `src/routes/Dashboard.svelte` (use `total_assets` and `listTopBudgetStatuses`)
- Modify: E2E mocks that return `list_balances` as an array, including `tests/e2e/tauriMock.ts`

**Review:** Suggestion 15 / AGENTS.md rule 2.

- [ ] **Step 1: Domain tests**

```rust
#[test]
fn total_assets_sums_non_archived_only() {
    // two AccountBalance-like tuples, one archived
}

#[test]
fn select_top_statuses_orders_by_percent_then_projected_then_name() {
    // three statuses, limit 2
}
```

```rust
pub fn total_assets<'a, I>(rows: I) -> i64
where
    I: IntoIterator<Item = &'a (i64, Option<String>)>, // better: take &[AccountBalance]
```

Use `balance_repo::AccountBalance` in domain? Repos already depend on domain. Prefer a tiny input:

```rust
pub fn total_assets(balances: &[(Option<String>, i64)]) -> i64 {
    balances
        .iter()
        .filter(|(archived_at, _)| archived_at.is_none())
        .fold(0i64, |sum, (_, bal)| sum.saturating_add(*bal))
}
```

Or pass `&[AccountBalance]` from `commands/balances.rs` after moving `AccountBalance` to `domain/balance.rs`. **Do not move the struct in this task** unless it is already painful — compute in `commands/balances.rs` via a domain fn that takes `(archived: bool, balance: i64)`:

```rust
pub fn total_assets<I>(rows: I) -> i64
where
    I: IntoIterator<Item = (bool, i64)>, // archived, balance
{
    rows.into_iter()
        .filter(|(archived, _)| !*archived)
        .fold(0i64, |sum, (_, bal)| sum.saturating_add(bal))
}
```

`select_top_statuses(mut items: Vec<BudgetStatus>, n: usize) -> Vec<BudgetStatus>`:

Sort by `percent` desc, `projected_over_budget` desc, `category_name` with `cmp`. Filter `budget_id.is_some()` first (Dashboard today drops unbudgeted). Then `truncate(n)`.

- [ ] **Step 2: RED, implement domain, GREEN**

- [ ] **Step 3: Command + TS**

```rust
#[derive(Serialize)]
pub struct BalanceList {
    pub accounts: Vec<AccountBalance>,
    pub total_assets: i64,
}

pub fn list_balances(...) -> AppResult<BalanceList> {
    state.with_conn(|conn| {
        let accounts = balance_repo::list_balances(conn)?;
        let total_assets = crate::domain::balance::total_assets(
            accounts
                .iter()
                .map(|row| (row.archived_at.is_some(), row.balance)),
        );
        Ok(BalanceList { accounts, total_assets })
    })
}
```

```rust
#[tauri::command]
pub fn list_top_budget_statuses(
    state: State<'_, AppState>,
    year_month: String,
    limit: u32,
) -> AppResult<Vec<BudgetStatus>> {
    if !(1..=10).contains(&limit) {
        return Err(AppError::InvalidArgument(format!(
            "limit must be 1..=10, got {limit}"
        )));
    }
    let today = chrono::Local::now().date_naive();
    state.with_conn(|conn| {
        let all = list_budget_statuses_for_conn(conn, &year_month, today)?;
        Ok(budget::select_top_statuses(all, limit as usize))
    })
}
```

Frontend:
- `listBalances(): Promise<BalanceList>`
- store: `items` from `res.accounts`, `totalAssets` from `res.total_assets` (no filter/reduce)
- Dashboard: `yen.format(balancesStore.totalAssets)` unchanged from the call site; implementation no longer reduces
- Dashboard widget: `listTopBudgetStatuses(currentYearMonth, 3)` instead of client sort. Remove `topBudgetStatuses` `$derived` sort.

Update `src/lib/api/balances.test.ts` expected invoke result shape.

Update Playwright `list_balances` to return `{ accounts: [...], total_assets: 0 }` in `tests/e2e/tauriMock.ts`, `transaction-flow`, `transfer-flow`, `budget-flow`. Add `list_top_budget_statuses` in `budget-flow` returning the first 3 of the existing fixture **without** recomputing percent in JS if the test already has `list_budget_statuses` — Dashboard will call the top command; stub it as `[]` or a precomputed array.

- [ ] **Step 4: `cargo test`, `pnpm test`, `pnpm check`, `pnpm test:e2e`**

- [ ] **Step 5: Commit**

```bash
git commit -m "$(cat <<'EOF'
refactor: compute total assets and top budget ranks in Rust

Svelte reduced balances and sorted budget percent, which violates the
domain-in-Rust rule and drifted from command output.
EOF
)"
```

---

## Wave D — UI completeness

### Task 14: Modal focus trap and Escape

**Files:**
- Modify: `src/lib/components/Modal.svelte`
- Create: `src/lib/components/Modal.test.ts` (Vitest + `@testing-library/svelte` if already in package.json; if not, add a lightweight `document` test via rendering in vitest)

**Review:** Suggestion 20. Spec §9.

- [ ] **Step 1: Check whether `@testing-library/svelte` is a dependency**

Run: `rg "@testing-library/svelte" package.json`

If missing, test via a small harness: export `getFocusable(root: HTMLElement): HTMLElement[]` and unit-test that. Keep DOM behavior in the component.

```ts
import type { Snippet } from 'svelte';

let {
  open = false,
  title,
  onclose,
  children,
  footer,
}: {
  open?: boolean;
  title: string;
  onclose?: () => void;
  children?: Snippet;
  footer?: Snippet;
} = $props();

let dialogEl = $state<HTMLDivElement | null>(null);
let lastFocus: HTMLElement | null = null;

function focusable(root: HTMLElement): HTMLElement[] {
  return Array.from(
    root.querySelectorAll<HTMLElement>(
      'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])',
    ),
  ).filter((el) => !el.hasAttribute('disabled'));
}

$effect(() => {
  if (!open) return;
  lastFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;
  const frame = requestAnimationFrame(() => {
    const nodes = dialogEl ? focusable(dialogEl) : [];
    nodes[0]?.focus();
  });
  function onKey(event: KeyboardEvent) {
    if (event.key === 'Escape') {
      event.preventDefault();
      onclose?.();
      return;
    }
    if (event.key !== 'Tab' || !dialogEl) return;
    const nodes = focusable(dialogEl);
    if (nodes.length === 0) return;
    const first = nodes[0];
    const last = nodes[nodes.length - 1];
    if (event.shiftKey && document.activeElement === first) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault();
      first.focus();
    }
  }
  document.addEventListener('keydown', onKey);
  return () => {
    cancelAnimationFrame(frame);
    document.removeEventListener('keydown', onKey);
    lastFocus?.focus();
  };
});
```

Bind `bind:this={dialogEl}` on `.dialog`. Remove the backdrop `onkeydown` Escape (document listener covers it). Keep backdrop click-to-close.

Test `focusable()` with a stub DOM or test Escape calls `onclose` if the component can be mounted. `@testing-library/svelte` is already in `package.json`. Mount `Modal` with `open={true}`, dispatch `Escape` on `document`, and assert `onclose` was called. Also assert the first `input` inside the dialog receives focus.

- [ ] **Step 2: `pnpm test` && `pnpm check`**

- [ ] **Step 3: Commit**

```bash
git commit -m "$(cat <<'EOF'
fix: trap focus in modals and handle Escape on document

Escape was bound on the backdrop, so it did nothing while focus stayed
on the opener behind the overlay.
EOF
)"
```

---

### Task 15: Transaction filters for category, account, and month; unify yen

**Files:**
- Modify: `src/routes/Transactions.svelte` (filter row)
- Modify: `src/routes/Dashboard.svelte` (`formatCurrency` instead of `Intl.NumberFormat`)
- Modify: `src/routes/Accounts.svelte` (same)
- Modify: `src/lib/utils/yearMonth.ts` — `monthRange` is now used (no delete)

**Review:** Suggestions 21 and 24. API already accepts `category_id` / `account_id`. Do **not** filter `txStore.items` in the client.

- [ ] **Step 1: Add filter state and pass through `setFilter`**

```ts
let filterCategory = $state('');
let filterAccount = $state('');
let filterMonth = $state(''); // YYYY-MM or ''

function applyFilter() {
  let from = filterFrom || undefined;
  let to = filterTo || undefined;
  if (filterMonth) {
    const [y, m] = filterMonth.split('-').map((p) => Number.parseInt(p, 10));
    const range = monthRange(y, m);
    from = range.from;
    to = range.to;
  }
  txStore.setFilter({
    type: filterType === 'all' ? undefined : (filterType as TxType),
    from,
    to,
    search: filterSearch || undefined,
    category_id: filterCategory ? Number.parseInt(filterCategory, 10) : undefined,
    account_id: filterAccount ? Number.parseInt(filterAccount, 10) : undefined,
  });
}
```

Selects:
- カテゴリ: `{ value: '', label: 'すべて' }` plus active categories (`!archived_at`). Include type in label (`食費（支出）`) so income/expense are distinct.
- 口座: `{ value: '', label: 'すべて' }` plus active accounts.
- 月: `<input type="month" bind:value={filterMonth} data-testid="tx-filter-month" />`. When set, it overrides from/to as above. Clear month to use the date pickers again.

`applyFilter` is invoked from 適用. Do not slice `txStore.items`.

Replace `const yen = new Intl.NumberFormat(...)` in Dashboard and Accounts with `formatCurrency` from `src/lib/utils/formatCurrency.ts` (already used on Budgets). That is `¥` not `￥`.

- [ ] **Step 2: `pnpm check` && `pnpm test:e2e` (transaction-flow still passes)**

- [ ] **Step 3: Commit**

```bash
git commit -m "$(cat <<'EOF'
feat: filter transactions by category, account, and month on the server

The list API already accepted those fields but the form only sent type,
dates, and search. Yen display now uses formatCurrency everywhere.
EOF
)"
```

---

## Wave E — Spec, lockfile, mocks, nits

### Task 16: Sync parent spec and product copy

**Files:**
- Modify: `docs/superpowers/specs/2026-05-24-budget-tracker-design.md` §§5.2, 5.3, 5.6, 6.1, 8, 改訂履歴
- Modify: `src-tauri/src/domain/ledger.rs` comments (if any Phase 2 leftovers remain after Task 11)
- Modify: `src-tauri/src/domain/report.rs` + `src-tauri/src/domain/budget.rs` — single `YearMonth`
- Modify: `src-tauri/src/domain/mod.rs`
- Modify: `src-tauri/src/domain/balance.rs` comment (`AGENTS.md` not `CLAUDE.md`)
- Modify: `src-tauri/src/infra/repo/balance_repo.rs` — keep one copy of the V002 comment (SQL const **or** fn, not both)

**Review:** Suggestions 9, 10, 13 (wording only), 23, 25.

- [ ] **Step 1: Spec edits (exact intent)**

§5.2 既存HTML機能の継承: replace Excel/JSON as current with:

> JSON エクスポート/インポート（上書き/追記、検証エラーでロールバック）は現行。Excel インポート/エクスポートは Phase 4 では未実装で、Phase 6 以降に先送りする。

§5.3 適用期間: add:

> Phase 4 の月別 UI はカテゴリ×月の1行（`starts_on = YYYY-MM-01`, `ends_on` 未使用）。lookup は `period = 'monthly' AND starts_on = 選択月の1日`。`ends_on` NULL を翌月へ継続するルールは未実装（将来）。

§5.6: label the 4-tab commands and Recurring/Reports routes as **Phase 5**. Document implemented commands: `monthly_summary(year, month)`, `monthly_series(months)`.

§6.1 Windows example: `jp.budget-tracker / db_key` (same as macOS, matching implementation). Storage path: `app_data_dir()/data.db` which is `.../jp.budget-tracker.app/data.db` (Tauri `identifier`). Do not tell implementers to rename Keychain.

§8 E2E row: Playwright against Vite (`pnpm dev`) + Tauri invoke mocks until `tauri-driver` (Phase 6). PR CI remains `clippy` / `cargo test` / `pnpm test` / `svelte-check` / Playwright smoke. `release.yml` is Phase 6.

JSON backup: one sentence that snapshot `schema_version` is **backup format version 1**, not `app_meta.schema_version`.

改訂履歴: `2026-08-29: Phase 4 レビューに合わせ、未実装機能とパス/鍵の現行実装を明記`.

- [ ] **Step 2: Unify `YearMonth`**

Create `src-tauri/src/domain/year_month.rs` with the **report** version (`key`, `parse_key`, `step_back`) plus `year_month_from_date` from Task 1. Re-export:

```rust
// domain/mod.rs
pub mod year_month;
pub use year_month::YearMonth;
```

`domain/budget.rs`: delete its struct; `use crate::domain::year_month::{YearMonth, parse_year_month}` or keep `parse_year_month` as a wrapper around `YearMonth::parse_key` so existing `budget::parse_year_month` call sites compile. Prefer `YearMonth::parse_key` and update call sites in `budget.rs`, `budget_repo.rs`, `commands/budgets.rs`, `integration_budgets.rs`.

`domain/report.rs`: delete duplicate; `use crate::domain::YearMonth`.

- [ ] **Step 3: `cargo test` + `cargo clippy --all-targets -- -D warnings`**

- [ ] **Step 4: Commit**

```bash
git commit -m "$(cat <<'EOF'
docs: align parent spec with Phase 4 implementation and unify YearMonth

The spec still described Excel, four report commands, and Keychain
paths that the tree does not implement. YearMonth was defined twice.
EOF
)"
```

---

### Task 17: Commit `Cargo.lock` and fail CI on drift

**Files:**
- Modify: `.gitignore` — delete `src-tauri/Cargo.lock`
- Create (git): `src-tauri/Cargo.lock` (generate if missing)
- Modify: `.github/workflows/ci.yml` — `cargo clippy --locked`, `cargo test --locked`; pin actions to SHAs

**Review:** Suggestion 12, part of 13.

- [ ] **Step 1: Generate and inspect lockfile**

```bash
cd src-tauri && cargo generate-lockfile
```

Confirm `src-tauri/Cargo.lock` exists and lists `rusqlite` / `tauri` / `keyring`.

- [ ] **Step 2: `.gitignore`**

Remove the line `src-tauri/Cargo.lock`. Keep `src-tauri/target/` and `src-tauri/gen/`.

- [ ] **Step 3: CI**

```yaml
- name: Cargo clippy
  run: cargo clippy --locked --all-targets -- -D warnings
  working-directory: src-tauri

- name: Cargo test
  run: cargo test --locked --no-fail-fast
  working-directory: src-tauri
```

Pin to SHAs (current tags as of implementation day — look up via `git ls-remote` or GitHub UI, do not invent):

- `actions/checkout`
- `pnpm/action-setup`
- `actions/setup-node`
- `ilammy/setup-nasm`
- `dtolnay/rust-toolchain`
- `Swatinem/rust-cache`

Use `uses: actions/checkout@<40-char-sha>  # v4.x.x` with the version in a comment.

- [ ] **Step 4: `cd src-tauri && cargo test --locked --no-fail-fast` locally**

- [ ] **Step 5: Commit**

```bash
git add .gitignore src-tauri/Cargo.lock .github/workflows/ci.yml
git commit -m "$(cat <<'EOF'
chore: track Cargo.lock and fail CI on dependency drift

This is an application crate; pnpm-lock.yaml was already frozen.
Rust builds were not reproducible without the lockfile.
EOF
)"
```

---

### Task 18: Playwright mocks must not reimplement budget math

**Files:**
- Modify: `tests/e2e/budget-flow.spec.ts`
- Modify: `tests/e2e/transaction-flow.spec.ts`
- Modify: `tests/e2e/transfer-flow.spec.ts` (if `list_budget_statuses` is missing)

**Review:** Suggestion 14. Domain tests remain the source of truth for `projected` / transfer exclusion.

- [ ] **Step 1: Stop returning `null` for dashboard commands**

In `transaction-flow.spec.ts` `default:` currently returns `null` for `list_balances` / `list_budget_statuses` / `list_top_budget_statuses`. After Task 3, `boot_status` is stubbed. After Task 13, Dashboard calls `list_balances` (object) and `list_top_budget_statuses`.

Add explicit stubs:

```js
case 'boot_status':
  return { state: 'ready', recovery_reason: null, db_path: '/tmp/data.db' };
case 'list_balances':
  return { accounts: [], total_assets: 0 };
case 'list_budget_statuses':
  return [];
case 'list_top_budget_statuses':
  return [];
```

Do this in **every** e2e init switch, including `smoke.spec.ts` if it loads Dashboard.

- [ ] **Step 2: budget-flow — do not recompute projection**

Replace `budgetStatuses()` JS percent/projected engine with a **fixture table** keyed by yearMonth:

When the test sets a budget of X and one expense of Y, return a **literal** `BudgetStatus` object with fields copied from what the test assertions need (warning badge). Comment: `// fixture; Rust domain tests own projected math`.

Do not implement `projected = spent`. If the UI assertion is “警告” based on `threshold_reached`, set `threshold_reached: true` in the fixture after `set_budget` + expense, rather than calculating it.

Keep the mock as a UI wiring test.

- [ ] **Step 3: `pnpm test:e2e`**

Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git commit -m "$(cat <<'EOF'
test: stub dashboard commands in Playwright instead of reimplementing Rust

Mocks returning null crashed spreads, and JS projected/percent cloned
domain logic so a Rust regression could still leave E2E green.
EOF
)"
```

---

## Wave verification (after Task 18)

```bash
cd src-tauri && cargo clippy --all-targets -- -D warnings
cd src-tauri && cargo test --no-fail-fast
pnpm test
pnpm check
pnpm test:e2e
```

All must be green before declaring the review closed.

Update `AGENTS.md` / `CLAUDE.md` “現在の状態” only if you want a pointer: Phase 4 complete + review fixes; next remains Phase 5. Optional one-line; include in Task 16 if you touch those files.

---

## Self-review

### Spec / review coverage

| Review item | Task |
|---|---|
| Bug 1 dates | 1 |
| Bug 2 decrypt panic | 2, 3 |
| Bug 3 archived remap | 4 (command side in 11) |
| Bug 4 empty on error | 5 |
| Bug 5 mutation errors | 5 |
| Bug 6 stale load | 6 |
| Bug 7 stale summary | 7 |
| Bug 8 archived balance | 8 |
| Suggestion 9 spec Excel/reports | 16 |
| Suggestion 10 ends_on | 16 (spec only; no code) |
| Suggestion 11 backup version | 12 |
| Suggestion 12 Cargo.lock | 17 |
| Suggestion 13 CI/E2E wording | 16, 17 (no release.yml) |
| Suggestion 14 Playwright math | 18 |
| Suggestion 15 Svelte totals | 13 |
| Suggestion 16 category/archive FK | 11 |
| Suggestion 17 transfer update | 11 |
| Suggestion 18 LIKE / page_size | 10 |
| Suggestion 19 FK in tests | 9 |
| Suggestion 20 modal a11y | 14 |
| Suggestion 21 filters | 15 |
| Suggestion 22 dashboard unmount | 7 |
| Suggestion 23 keychain path spec | 16 |
| Nit 24 monthRange / yen | 15 |
| Nit 25 YearMonth / comments | 16 |

### Placeholder scan

No TBD/TODO left. Each task has commands, expected FAIL/PASS, and a commit message.

### Type consistency

- `BootStatus.state` is `'ready' | 'recovery'` in Rust serde and TS.
- `RecoveryReason` snake_case: `key_missing`, `decrypt_failed`, `key_corrupt`, `keychain_error`.
- `BalanceList = { accounts, total_assets }` after Task 13; E2E stubs updated in Tasks 13 and 18.
- `list_top_budget_statuses(year_month, limit)` with `limit` 1..=10.
- `BACKUP_FORMAT_VERSION = 1` is the JSON `schema_version` field.
- `AllowedArchivedRefs` used by create (`none`) and update (existing ids).

### Execution notes

- Work in the current repo unless starting a long-lived feature branch; `using-git-worktrees` is optional here because this is review follow-up on `main`.
- Do not implement `ends_on` continuation if a test “feels” like the parent spec wants it — Task 16 already records the Phase 4 rule.
- After Task 3, if `pnpm test:e2e` fails on Dashboard, the first suspect is a missing `boot_status` / `list_balances` stub (`null`).
