# Phase 2 — Slice 03: Accounts

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this slice task-by-task.

**Goal:** Implement account CRUD end-to-end. Reuses the patterns established in slice 02. No transfer UI (Phase 3).

**Prerequisite:** Slice 02 (categories) completed.

**Spec:** sections 4.2, 5.5 of `docs/superpowers/specs/2026-05-25-phase2-transactions-design.md`.

---

### Task 1: `domain/account.rs` types and validators

**Files:**
- Modify: `src-tauri/src/domain/account.rs`

- [ ] **Step 1: Implement with co-located tests**

```rust
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccountKind {
    Cash,
    Bank,
    CreditCard,
    EMoney,
    Investment,
}

impl AccountKind {
    pub fn as_sql(self) -> &'static str {
        match self {
            AccountKind::Cash => "cash",
            AccountKind::Bank => "bank",
            AccountKind::CreditCard => "credit_card",
            AccountKind::EMoney => "e_money",
            AccountKind::Investment => "investment",
        }
    }

    pub fn parse(raw: &str) -> AppResult<Self> {
        match raw {
            "cash" => Ok(Self::Cash),
            "bank" => Ok(Self::Bank),
            "credit_card" => Ok(Self::CreditCard),
            "e_money" => Ok(Self::EMoney),
            "investment" => Ok(Self::Investment),
            other => Err(AppError::InvalidArgument(format!(
                "account kind must be cash|bank|credit_card|e_money|investment, got '{other}'"
            ))),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub id: i64,
    pub name: String,
    pub kind: AccountKind,
    pub currency: String,
    pub initial_balance: i64,
    pub display_order: i64,
    pub note: String,
    pub archived_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

const MAX_NAME_LEN: usize = 40;
const MAX_NOTE_LEN: usize = 200;

pub fn validate_name(raw: &str) -> AppResult<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(AppError::InvalidArgument("account name is empty".into()));
    }
    if trimmed.chars().count() > MAX_NAME_LEN {
        return Err(AppError::InvalidArgument(format!(
            "account name must be {MAX_NAME_LEN} chars or fewer"
        )));
    }
    Ok(trimmed.to_string())
}

pub fn validate_note(raw: &str) -> AppResult<String> {
    if raw.chars().count() > MAX_NOTE_LEN {
        return Err(AppError::InvalidArgument(format!(
            "note must be {MAX_NOTE_LEN} chars or fewer"
        )));
    }
    Ok(raw.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_all_known_kinds() {
        for s in ["cash", "bank", "credit_card", "e_money", "investment"] {
            assert!(AccountKind::parse(s).is_ok(), "{s} should parse");
        }
    }

    #[test]
    fn rejects_unknown_kind() {
        assert!(matches!(
            AccountKind::parse("crypto").unwrap_err(),
            AppError::InvalidArgument(_)
        ));
    }

    #[test]
    fn validate_name_trims() {
        assert_eq!(validate_name("  楽天銀行  ").unwrap(), "楽天銀行");
    }

    #[test]
    fn validate_name_rejects_empty() {
        assert!(validate_name("").is_err());
        assert!(validate_name("   ").is_err());
    }

    #[test]
    fn validate_note_accepts_empty_and_200_chars() {
        assert!(validate_note("").is_ok());
        assert!(validate_note(&"あ".repeat(200)).is_ok());
        assert!(validate_note(&"あ".repeat(201)).is_err());
    }
}
```

- [ ] **Step 2: Test & clippy**

```bash
cargo test --lib domain::account
cargo clippy --all-targets -- -D warnings
```

Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/domain/account.rs
git commit -m "feat(domain): account kind enum and name/note validators"
```

---

### Task 2: `infra/repo/account_repo.rs`

**Files:**
- Create: `src-tauri/src/infra/repo/account_repo.rs`
- Modify: `src-tauri/src/infra/repo/mod.rs`

- [ ] **Step 1: Wire the module**

In `src-tauri/src/infra/repo/mod.rs`, add:

```rust
pub mod account_repo;
```

- [ ] **Step 2: Implement**

```rust
// src-tauri/src/infra/repo/account_repo.rs
use rusqlite::{Connection, OptionalExtension, params};

use crate::domain::account::{Account, AccountKind};
use crate::error::{AppError, AppResult};

fn row_to_account(row: &rusqlite::Row<'_>) -> rusqlite::Result<Account> {
    let kind_raw: String = row.get("kind")?;
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
    Ok(Account {
        id: row.get("id")?,
        name: row.get("name")?,
        kind,
        currency: row.get("currency")?,
        initial_balance: row.get("initial_balance")?,
        display_order: row.get("display_order")?,
        note: row.get("note")?,
        archived_at: row.get("archived_at")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

pub fn list(conn: &Connection, include_archived: bool) -> AppResult<Vec<Account>> {
    let sql = if include_archived {
        "SELECT id, name, kind, currency, initial_balance, display_order, note,
                archived_at, created_at, updated_at
           FROM accounts
          ORDER BY display_order ASC, id ASC"
    } else {
        "SELECT id, name, kind, currency, initial_balance, display_order, note,
                archived_at, created_at, updated_at
           FROM accounts
          WHERE archived_at IS NULL
          ORDER BY display_order ASC, id ASC"
    };
    let mut stmt = conn.prepare(sql)?;
    Ok(stmt
        .query_map([], row_to_account)?
        .collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn find_by_id(conn: &Connection, id: i64) -> AppResult<Account> {
    conn.query_row(
        "SELECT id, name, kind, currency, initial_balance, display_order, note,
                archived_at, created_at, updated_at
           FROM accounts WHERE id = ?1",
        params![id],
        row_to_account,
    )
    .optional()?
    .ok_or_else(|| AppError::NotFound(format!("account {id}")))
}

pub fn next_display_order(conn: &Connection) -> AppResult<i64> {
    let max: Option<i64> = conn.query_row(
        "SELECT MAX(display_order) FROM accounts",
        [],
        |r| r.get(0),
    )?;
    Ok(max.unwrap_or(-1) + 1)
}

pub struct InsertInput<'a> {
    pub name: &'a str,
    pub kind: AccountKind,
    pub currency: &'a str,
    pub initial_balance: i64,
    pub display_order: i64,
    pub note: &'a str,
    pub now: &'a str,
}

pub fn insert(conn: &Connection, input: &InsertInput<'_>) -> AppResult<i64> {
    conn.execute(
        "INSERT INTO accounts(name, kind, currency, initial_balance, display_order, note,
                              created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)",
        params![
            input.name,
            input.kind.as_sql(),
            input.currency,
            input.initial_balance,
            input.display_order,
            input.note,
            input.now,
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

#[derive(Debug, Default)]
pub struct UpdatePatch<'a> {
    pub name: Option<&'a str>,
    pub kind: Option<AccountKind>,
    pub initial_balance: Option<i64>,
    pub note: Option<&'a str>,
    pub display_order: Option<i64>,
}

pub fn update(conn: &Connection, id: i64, patch: &UpdatePatch<'_>, now: &str) -> AppResult<()> {
    if let Some(name) = patch.name {
        conn.execute(
            "UPDATE accounts SET name = ?1, updated_at = ?2 WHERE id = ?3",
            params![name, now, id],
        )?;
    }
    if let Some(kind) = patch.kind {
        conn.execute(
            "UPDATE accounts SET kind = ?1, updated_at = ?2 WHERE id = ?3",
            params![kind.as_sql(), now, id],
        )?;
    }
    if let Some(balance) = patch.initial_balance {
        conn.execute(
            "UPDATE accounts SET initial_balance = ?1, updated_at = ?2 WHERE id = ?3",
            params![balance, now, id],
        )?;
    }
    if let Some(note) = patch.note {
        conn.execute(
            "UPDATE accounts SET note = ?1, updated_at = ?2 WHERE id = ?3",
            params![note, now, id],
        )?;
    }
    if let Some(order) = patch.display_order {
        conn.execute(
            "UPDATE accounts SET display_order = ?1, updated_at = ?2 WHERE id = ?3",
            params![order, now, id],
        )?;
    }
    Ok(())
}

pub fn set_archived(conn: &Connection, id: i64, archived_at: Option<&str>, now: &str) -> AppResult<()> {
    let n = conn.execute(
        "UPDATE accounts SET archived_at = ?1, updated_at = ?2 WHERE id = ?3",
        params![archived_at, now, id],
    )?;
    if n == 0 {
        return Err(AppError::NotFound(format!("account {id}")));
    }
    Ok(())
}
```

- [ ] **Step 3: Build + clippy**

```bash
cargo build
cargo clippy --all-targets -- -D warnings
```

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/infra/repo
git commit -m "feat(infra): account_repo with list/insert/update/archive"
```

---

### Task 3: Integration test for account_repo

**Files:**
- Create: `src-tauri/tests/integration_accounts.rs`

- [ ] **Step 1: Add test file**

```rust
use budget_tracker_lib::domain::account::AccountKind;
use budget_tracker_lib::infra::migrations;
use budget_tracker_lib::infra::repo::account_repo;
use rusqlite::Connection;

const NOW: &str = "2026-05-25T00:00:00+00:00";

fn fresh() -> Connection {
    let mut c = Connection::open_in_memory().unwrap();
    migrations::run(&mut c).unwrap();
    c
}

#[test]
fn insert_then_find_returns_inserted_row() {
    let conn = fresh();
    let id = account_repo::insert(
        &conn,
        &account_repo::InsertInput {
            name: "三井住友銀行",
            kind: AccountKind::Bank,
            currency: "JPY",
            initial_balance: 100_000,
            display_order: 0,
            note: "メイン口座",
            now: NOW,
        },
    )
    .unwrap();
    let acc = account_repo::find_by_id(&conn, id).unwrap();
    assert_eq!(acc.name, "三井住友銀行");
    assert_eq!(acc.kind, AccountKind::Bank);
    assert_eq!(acc.initial_balance, 100_000);
    assert_eq!(acc.note, "メイン口座");
    assert_eq!(acc.created_at, NOW);
    assert_eq!(acc.updated_at, NOW);
}

#[test]
fn list_excludes_archived_by_default() {
    let conn = fresh();
    let id = account_repo::insert(
        &conn,
        &account_repo::InsertInput {
            name: "旧口座",
            kind: AccountKind::Cash,
            currency: "JPY",
            initial_balance: 0,
            display_order: 0,
            note: "",
            now: NOW,
        },
    )
    .unwrap();
    account_repo::set_archived(&conn, id, Some(NOW), NOW).unwrap();
    assert!(account_repo::list(&conn, false).unwrap().is_empty());
    assert_eq!(account_repo::list(&conn, true).unwrap().len(), 1);
}

#[test]
fn update_changes_updated_at() {
    let conn = fresh();
    let id = account_repo::insert(
        &conn,
        &account_repo::InsertInput {
            name: "PayPay",
            kind: AccountKind::EMoney,
            currency: "JPY",
            initial_balance: 0,
            display_order: 0,
            note: "",
            now: NOW,
        },
    )
    .unwrap();
    let later = "2026-06-01T00:00:00+00:00";
    account_repo::update(
        &conn,
        id,
        &account_repo::UpdatePatch {
            note: Some("チャージ用"),
            ..Default::default()
        },
        later,
    )
    .unwrap();
    let acc = account_repo::find_by_id(&conn, id).unwrap();
    assert_eq!(acc.note, "チャージ用");
    assert_eq!(acc.updated_at, later);
    assert_eq!(acc.created_at, NOW);
}

#[test]
fn next_display_order_starts_at_zero_and_increments() {
    let conn = fresh();
    assert_eq!(account_repo::next_display_order(&conn).unwrap(), 0);
    account_repo::insert(
        &conn,
        &account_repo::InsertInput {
            name: "A",
            kind: AccountKind::Cash,
            currency: "JPY",
            initial_balance: 0,
            display_order: 0,
            note: "",
            now: NOW,
        },
    )
    .unwrap();
    assert_eq!(account_repo::next_display_order(&conn).unwrap(), 1);
}
```

- [ ] **Step 2: Run**

```bash
cargo test --test integration_accounts
```

Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/tests/integration_accounts.rs
git commit -m "test(account_repo): insert/list/update/archive against memory db"
```

---

### Task 4: `commands/accounts.rs` + handler registration

**Files:**
- Modify: `src-tauri/src/commands/accounts.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Implement handlers**

```rust
// src-tauri/src/commands/accounts.rs
use serde::Deserialize;
use tauri::{AppHandle, State};

use crate::commands::meta::AppState;
use crate::domain::account::{self, Account, AccountKind};
use crate::error::{AppError, AppResult};
use crate::infra::events::{ChangedDomain, emit_changed};
use crate::infra::repo::account_repo;

fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339()
}

#[tauri::command]
pub fn list_accounts(state: State<'_, AppState>, include_archived: bool) -> AppResult<Vec<Account>> {
    let conn = state.conn.lock().map_err(|_| AppError::Corrupt("conn poisoned".into()))?;
    account_repo::list(&conn, include_archived)
}

#[derive(Debug, Deserialize)]
pub struct CreateAccountInput {
    pub name: String,
    pub kind: String,
    pub initial_balance: i64,
    #[serde(default)]
    pub note: String,
}

#[tauri::command]
pub fn create_account(
    app: AppHandle,
    state: State<'_, AppState>,
    input: CreateAccountInput,
) -> AppResult<Account> {
    let name = account::validate_name(&input.name)?;
    let note = account::validate_note(&input.note)?;
    let kind = AccountKind::parse(&input.kind)?;
    let now = now_iso();
    let conn = state.conn.lock().map_err(|_| AppError::Corrupt("conn poisoned".into()))?;
    let order = account_repo::next_display_order(&conn)?;
    let id = account_repo::insert(
        &conn,
        &account_repo::InsertInput {
            name: &name,
            kind,
            currency: "JPY",
            initial_balance: input.initial_balance,
            display_order: order,
            note: &note,
            now: &now,
        },
    )?;
    let acc = account_repo::find_by_id(&conn, id)?;
    drop(conn);
    emit_changed(&app, ChangedDomain::Accounts);
    Ok(acc)
}

#[derive(Debug, Deserialize)]
pub struct UpdateAccountPatch {
    pub name: Option<String>,
    pub kind: Option<String>,
    pub initial_balance: Option<i64>,
    pub note: Option<String>,
    pub display_order: Option<i64>,
}

#[tauri::command]
pub fn update_account(
    app: AppHandle,
    state: State<'_, AppState>,
    id: i64,
    patch: UpdateAccountPatch,
) -> AppResult<Account> {
    let name = patch.name.as_deref().map(account::validate_name).transpose()?;
    let note = patch.note.as_deref().map(account::validate_note).transpose()?;
    let kind = patch.kind.as_deref().map(AccountKind::parse).transpose()?;
    let now = now_iso();
    let conn = state.conn.lock().map_err(|_| AppError::Corrupt("conn poisoned".into()))?;
    account_repo::update(
        &conn,
        id,
        &account_repo::UpdatePatch {
            name: name.as_deref(),
            kind,
            initial_balance: patch.initial_balance,
            note: note.as_deref(),
            display_order: patch.display_order,
        },
        &now,
    )?;
    let acc = account_repo::find_by_id(&conn, id)?;
    drop(conn);
    emit_changed(&app, ChangedDomain::Accounts);
    Ok(acc)
}

#[tauri::command]
pub fn archive_account(app: AppHandle, state: State<'_, AppState>, id: i64) -> AppResult<()> {
    let now = now_iso();
    let conn = state.conn.lock().map_err(|_| AppError::Corrupt("conn poisoned".into()))?;
    account_repo::set_archived(&conn, id, Some(&now), &now)?;
    drop(conn);
    emit_changed(&app, ChangedDomain::Accounts);
    Ok(())
}

#[tauri::command]
pub fn unarchive_account(app: AppHandle, state: State<'_, AppState>, id: i64) -> AppResult<()> {
    let now = now_iso();
    let conn = state.conn.lock().map_err(|_| AppError::Corrupt("conn poisoned".into()))?;
    account_repo::set_archived(&conn, id, None, &now)?;
    drop(conn);
    emit_changed(&app, ChangedDomain::Accounts);
    Ok(())
}
```

- [ ] **Step 2: Register handlers**

In `src-tauri/src/lib.rs`, extend `generate_handler!`:

```rust
.invoke_handler(tauri::generate_handler![
    app_info,
    commands::categories::list_categories,
    commands::categories::create_category,
    commands::categories::update_category,
    commands::categories::archive_category,
    commands::categories::unarchive_category,
    commands::accounts::list_accounts,
    commands::accounts::create_account,
    commands::accounts::update_account,
    commands::accounts::archive_account,
    commands::accounts::unarchive_account,
])
```

- [ ] **Step 3: Build + clippy + test**

```bash
cargo build
cargo clippy --all-targets -- -D warnings
cargo test
```

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/commands/accounts.rs src-tauri/src/lib.rs
git commit -m "feat(commands): expose accounts CRUD over tauri invoke"
```

---

### Task 5: Frontend `lib/api/accounts.ts` + store

**Files:**
- Create: `src/lib/api/accounts.ts`
- Create: `src/lib/stores/accounts.svelte.ts`
- Modify: `src/lib/api/index.ts`

- [ ] **Step 1: API wrapper**

```ts
// src/lib/api/accounts.ts
import { invoke } from '@tauri-apps/api/core';

export type AccountKind = 'cash' | 'bank' | 'credit_card' | 'e_money' | 'investment';

export type Account = {
  id: number;
  name: string;
  kind: AccountKind;
  currency: string;
  initial_balance: number;
  display_order: number;
  note: string;
  archived_at: string | null;
  created_at: string;
  updated_at: string;
};

export function listAccounts(include_archived = false): Promise<Account[]> {
  return invoke<Account[]>('list_accounts', { includeArchived: include_archived });
}

export type CreateAccountInput = {
  name: string;
  kind: AccountKind;
  initial_balance: number;
  note?: string;
};

export function createAccount(input: CreateAccountInput): Promise<Account> {
  return invoke<Account>('create_account', { input });
}

export type UpdateAccountPatch = {
  name?: string;
  kind?: AccountKind;
  initial_balance?: number;
  note?: string;
  display_order?: number;
};

export function updateAccount(id: number, patch: UpdateAccountPatch): Promise<Account> {
  return invoke<Account>('update_account', { id, patch });
}

export function archiveAccount(id: number): Promise<void> {
  return invoke('archive_account', { id });
}

export function unarchiveAccount(id: number): Promise<void> {
  return invoke('unarchive_account', { id });
}

export const ACCOUNT_KIND_LABELS: Record<AccountKind, string> = {
  cash: '現金',
  bank: '銀行',
  credit_card: 'クレジットカード',
  e_money: '電子マネー',
  investment: '投資',
};

export const ACCOUNT_KIND_ICONS: Record<AccountKind, string> = {
  cash: '💵',
  bank: '🏦',
  credit_card: '💳',
  e_money: '📱',
  investment: '📈',
};
```

- [ ] **Step 2: Add to barrel**

Append to `src/lib/api/index.ts`:

```ts
export * from './accounts';
```

- [ ] **Step 3: Store**

```ts
// src/lib/stores/accounts.svelte.ts
import { listAccounts, type Account } from '../api/accounts';
import { onDataChanged } from '../api/events';
import type { UnlistenFn } from '@tauri-apps/api/event';

export type AccountsStore = {
  readonly items: Account[];
  readonly loading: boolean;
  readonly error: string | null;
  load(): Promise<void>;
  setIncludeArchived(value: boolean): void;
  dispose(): Promise<void>;
};

export function createAccountsStore(initialIncludeArchived = false): AccountsStore {
  let items = $state<Account[]>([]);
  let loading = $state(false);
  let error = $state<string | null>(null);
  let includeArchived = $state(initialIncludeArchived);
  let unlisten: UnlistenFn | null = null;

  async function load() {
    loading = true;
    error = null;
    try {
      items = await listAccounts(includeArchived);
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }

  function setIncludeArchived(v: boolean) {
    includeArchived = v;
    void load();
  }

  void (async () => {
    unlisten = await onDataChanged((domain) => {
      if (domain === 'accounts') void load();
    });
    await load();
  })();

  return {
    get items() { return items; },
    get loading() { return loading; },
    get error() { return error; },
    load,
    setIncludeArchived,
    async dispose() { unlisten?.(); unlisten = null; },
  };
}
```

- [ ] **Step 4: svelte-check + vitest**

```bash
pnpm check
pnpm test
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/lib/api src/lib/stores
git commit -m "feat(api): typed accounts wrapper and reactive store"
```

---

### Task 6: `routes/Accounts.svelte`

**Files:**
- Modify: `src/routes/Accounts.svelte`

- [ ] **Step 1: Replace placeholder**

```svelte
<script lang="ts">
  import Card from '../lib/components/Card.svelte';
  import Button from '../lib/components/Button.svelte';
  import Modal from '../lib/components/Modal.svelte';
  import TextField from '../lib/components/TextField.svelte';
  import Select from '../lib/components/Select.svelte';
  import EmptyState from '../lib/components/EmptyState.svelte';
  import { createAccountsStore } from '../lib/stores/accounts.svelte';
  import {
    createAccount,
    updateAccount,
    archiveAccount,
    unarchiveAccount,
    ACCOUNT_KIND_LABELS,
    ACCOUNT_KIND_ICONS,
    type Account,
    type AccountKind,
  } from '../lib/api/accounts';

  const store = createAccountsStore(true);
  const yen = new Intl.NumberFormat('ja-JP', { style: 'currency', currency: 'JPY' });

  let modalOpen = $state(false);
  let editing = $state<Account | null>(null);
  let name = $state('');
  let kind = $state<AccountKind>('cash');
  let initialBalance = $state('0');
  let note = $state('');
  let formError = $state<string | null>(null);

  function openCreate() {
    editing = null;
    name = '';
    kind = 'cash';
    initialBalance = '0';
    note = '';
    formError = null;
    modalOpen = true;
  }

  function openEdit(acc: Account) {
    editing = acc;
    name = acc.name;
    kind = acc.kind;
    initialBalance = String(acc.initial_balance);
    note = acc.note;
    formError = null;
    modalOpen = true;
  }

  async function submit() {
    formError = null;
    const balance = Number.parseInt(initialBalance, 10);
    if (!Number.isFinite(balance)) {
      formError = '開始残高は整数を入力してください';
      return;
    }
    try {
      if (editing) {
        await updateAccount(editing.id, {
          name,
          kind,
          initial_balance: balance,
          note,
        });
      } else {
        await createAccount({ name, kind, initial_balance: balance, note });
      }
      modalOpen = false;
    } catch (e) {
      formError = e instanceof Error ? e.message : String(e);
    }
  }

  async function toggleArchive(acc: Account) {
    if (acc.archived_at) await unarchiveAccount(acc.id);
    else await archiveAccount(acc.id);
  }

  const kindOptions = (Object.keys(ACCOUNT_KIND_LABELS) as AccountKind[]).map((k) => ({
    value: k,
    label: `${ACCOUNT_KIND_ICONS[k]} ${ACCOUNT_KIND_LABELS[k]}`,
  }));

  const visible = $derived(store.items.filter((a) => !a.archived_at));
  const archived = $derived(store.items.filter((a) => !!a.archived_at));
</script>

<section>
  <header class="page-header">
    <h1>口座</h1>
    <Button onclick={openCreate}>
      {#snippet children()}+ 追加{/snippet}
    </Button>
  </header>

  <Card>
    {#snippet children()}
      {#if visible.length === 0 && !store.loading}
        <EmptyState title="口座がありません" hint="右上の「+ 追加」から作成してください" />
      {:else}
        <ul class="list" data-testid="accounts-list">
          {#each visible as acc (acc.id)}
            <li>
              <span class="icon" aria-hidden="true">{ACCOUNT_KIND_ICONS[acc.kind]}</span>
              <div class="meta">
                <strong>{acc.name}</strong>
                <small>{ACCOUNT_KIND_LABELS[acc.kind]}</small>
              </div>
              <span class="balance">{yen.format(acc.initial_balance)}</span>
              <Button variant="ghost" onclick={() => openEdit(acc)}>
                {#snippet children()}編集{/snippet}
              </Button>
              <Button variant="ghost" onclick={() => toggleArchive(acc)}>
                {#snippet children()}アーカイブ{/snippet}
              </Button>
            </li>
          {/each}
        </ul>
      {/if}
    {/snippet}
  </Card>

  {#if archived.length > 0}
    <h2 class="sub">アーカイブ済み</h2>
    <Card>
      {#snippet children()}
        <ul class="list">
          {#each archived as acc (acc.id)}
            <li>
              <span class="icon" aria-hidden="true">{ACCOUNT_KIND_ICONS[acc.kind]}</span>
              <div class="meta">
                <strong>{acc.name}</strong>
                <small>{ACCOUNT_KIND_LABELS[acc.kind]}</small>
              </div>
              <span class="balance">{yen.format(acc.initial_balance)}</span>
              <Button variant="ghost" onclick={() => toggleArchive(acc)}>
                {#snippet children()}復元{/snippet}
              </Button>
            </li>
          {/each}
        </ul>
      {/snippet}
    </Card>
  {/if}
</section>

<Modal
  open={modalOpen}
  title={editing ? '口座を編集' : '口座を追加'}
  onclose={() => (modalOpen = false)}
>
  {#snippet children()}
    <TextField label="名前" required bind:value={name} testid="account-name" />
    <Select
      label="種別"
      required
      bind:value={kind}
      options={kindOptions}
      testid="account-kind"
    />
    <TextField label="開始残高 (円)" required type="number" bind:value={initialBalance} testid="account-initial" />
    <TextField label="メモ" bind:value={note} />
    {#if formError}<small class="error">{formError}</small>{/if}
  {/snippet}
  {#snippet footer()}
    <Button variant="ghost" onclick={() => (modalOpen = false)}>
      {#snippet children()}キャンセル{/snippet}
    </Button>
    <Button onclick={submit}>
      {#snippet children()}{editing ? '更新' : '追加'}{/snippet}
    </Button>
  {/snippet}
</Modal>

<style>
  .page-header { display: flex; justify-content: space-between; align-items: center; margin-bottom: var(--space-5); }
  h1 { color: white; margin: 0; }
  h2.sub { color: white; margin: var(--space-6) 0 var(--space-4); }
  .list { list-style: none; padding: 0; margin: 0; display: grid; gap: var(--space-3); }
  .list li {
    display: grid;
    grid-template-columns: auto 1fr auto auto auto;
    align-items: center;
    gap: var(--space-4);
    padding: var(--space-3);
    border-radius: var(--radius-md);
    background: rgba(0,0,0,0.02);
  }
  .icon { font-size: 1.5rem; }
  .meta { display: grid; }
  .meta small { color: var(--muted); }
  .balance { font-weight: 700; font-variant-numeric: tabular-nums; }
  .error { color: var(--danger); }
</style>
```

- [ ] **Step 2: svelte-check**

```bash
pnpm check
```

Expected: PASS.

- [ ] **Step 3: Manual smoke**

```bash
pnpm tauri dev
```

- Navigate to `口座`.
- Add a new account (e.g., name "現金", kind cash, initial_balance 5000).
- Edit it; archive it; restore it.

Stop the dev server.

- [ ] **Step 4: Commit**

```bash
git add src/routes/Accounts.svelte
git commit -m "feat(ui): accounts page with list, add, edit, archive flow"
```

---

### Accounts slice DoD

- [ ] `cargo clippy --all-targets -- -D warnings` green.
- [ ] `cargo test` green.
- [ ] `pnpm test` green.
- [ ] `pnpm check` green.
- [ ] Manual: account CRUD + archive/restore works; balance shown in JPY.
