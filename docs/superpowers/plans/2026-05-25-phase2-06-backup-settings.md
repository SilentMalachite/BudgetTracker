# Phase 2 — Slice 06: Backup, Settings, E2E

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this slice task-by-task.

**Goal:** Land JSON backup/restore, the Settings page (DB path, last backup time, export/import buttons), the Phase 2 Playwright happy-path E2E, and a final Phase 2 DoD sweep.

**Prerequisite:** Slices 02–05 completed.

**Spec:** sections 4.5, 4.6, 5.2, 9 of `docs/superpowers/specs/2026-05-25-phase2-transactions-design.md`.

---

### Task 1: `commands/backup.rs` — `export_json` and `import_json`

**Files:**
- Modify: `src-tauri/src/commands/backup.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Implement export + import**

```rust
// src-tauri/src/commands/backup.rs
use rusqlite::params;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tauri::{AppHandle, State};

use crate::commands::meta::AppState;
use crate::error::{AppError, AppResult};
use crate::infra::events::{ChangedDomain, emit_changed};
use crate::infra::repo::meta_repo;

const SUPPORTED_SCHEMA_VERSION: u32 = 1;
const MAX_PAYLOAD_BYTES: usize = 10 * 1024 * 1024;

#[derive(Debug, Serialize, Deserialize)]
struct Snapshot {
    schema_version: u32,
    exported_at: String,
    categories: Vec<serde_json::Value>,
    accounts: Vec<serde_json::Value>,
    transactions: Vec<serde_json::Value>,
    app_meta: Vec<serde_json::Value>,
}

fn select_all(
    conn: &rusqlite::Connection,
    sql: &str,
    cols: &[&str],
) -> AppResult<Vec<serde_json::Value>> {
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map([], |r| {
        let mut obj = serde_json::Map::new();
        for (i, col) in cols.iter().enumerate() {
            let v: rusqlite::types::Value = r.get(i)?;
            obj.insert((*col).to_string(), match v {
                rusqlite::types::Value::Null => serde_json::Value::Null,
                rusqlite::types::Value::Integer(i) => json!(i),
                rusqlite::types::Value::Real(f) => json!(f),
                rusqlite::types::Value::Text(s) => json!(s),
                rusqlite::types::Value::Blob(_) => serde_json::Value::Null,
            });
        }
        Ok(serde_json::Value::Object(obj))
    })?;
    rows.collect::<rusqlite::Result<_>>().map_err(AppError::Db)
}

#[tauri::command]
pub fn export_json(state: State<'_, AppState>) -> AppResult<String> {
    let conn = state.conn.lock().map_err(|_| AppError::Corrupt("conn poisoned".into()))?;
    let snap = Snapshot {
        schema_version: SUPPORTED_SCHEMA_VERSION,
        exported_at: chrono::Utc::now().to_rfc3339(),
        categories: select_all(
            &conn,
            "SELECT id, name, type, color, icon, display_order, archived_at FROM categories ORDER BY id",
            &["id", "name", "type", "color", "icon", "display_order", "archived_at"],
        )?,
        accounts: select_all(
            &conn,
            "SELECT id, name, kind, currency, initial_balance, display_order, note,
                    archived_at, created_at, updated_at FROM accounts ORDER BY id",
            &["id", "name", "kind", "currency", "initial_balance", "display_order",
              "note", "archived_at", "created_at", "updated_at"],
        )?,
        transactions: select_all(
            &conn,
            "SELECT id, occurred_on, type, amount, account_id, counter_account_id,
                    category_id, description, recurring_id, created_at, updated_at
               FROM transactions ORDER BY id",
            &["id", "occurred_on", "type", "amount", "account_id", "counter_account_id",
              "category_id", "description", "recurring_id", "created_at", "updated_at"],
        )?,
        app_meta: select_all(
            &conn,
            "SELECT key, value FROM app_meta ORDER BY key",
            &["key", "value"],
        )?,
    };
    serde_json::to_string_pretty(&snap)
        .map_err(|e| AppError::Migration(format!("encode snapshot: {e}")))
}

#[derive(Debug, Deserialize)]
pub struct ImportArgs {
    pub payload: String,
    pub mode: String, // "overwrite" | "append"
}

#[derive(Debug, Serialize)]
pub struct ImportResult {
    pub categories: u32,
    pub accounts: u32,
    pub transactions: u32,
    pub warnings: Vec<String>,
}

#[tauri::command]
pub fn import_json(
    app: AppHandle,
    state: State<'_, AppState>,
    args: ImportArgs,
) -> AppResult<ImportResult> {
    if args.payload.len() > MAX_PAYLOAD_BYTES {
        return Err(AppError::InvalidArgument(format!(
            "payload exceeds {MAX_PAYLOAD_BYTES} bytes"
        )));
    }
    let snap: Snapshot = serde_json::from_str(&args.payload).map_err(|e| {
        AppError::InvalidArgument(format!("invalid backup JSON: {e}"))
    })?;
    if snap.schema_version != SUPPORTED_SCHEMA_VERSION {
        return Err(AppError::InvalidArgument(format!(
            "unsupported schema_version {}", snap.schema_version
        )));
    }

    let mode = args.mode.as_str();
    if mode != "overwrite" && mode != "append" {
        return Err(AppError::InvalidArgument(format!("unknown mode '{mode}'")));
    }

    let mut conn = state.conn.lock().map_err(|_| AppError::Corrupt("conn poisoned".into()))?;
    let tx = conn.transaction()?;

    let mut warnings: Vec<String> = Vec::new();
    let mut category_count = 0u32;
    let mut account_count = 0u32;
    let mut transaction_count = 0u32;

    if mode == "overwrite" {
        for sql in &[
            "DELETE FROM transactions",
            "DELETE FROM budgets",
            "DELETE FROM recurring_rules",
            "DELETE FROM categories",
            "DELETE FROM accounts",
        ] {
            tx.execute(sql, [])?;
        }
        tx.execute("DELETE FROM sqlite_sequence WHERE name IN ('categories','accounts','transactions')", [])?;
    }

    // Insert categories.
    for cat in &snap.categories {
        let result = if mode == "overwrite" {
            tx.execute(
                "INSERT INTO categories(id, name, type, color, icon, display_order, archived_at)
                 VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    cat.get("id").and_then(|v| v.as_i64()),
                    cat.get("name").and_then(|v| v.as_str()).unwrap_or(""),
                    cat.get("type").and_then(|v| v.as_str()).unwrap_or("expense"),
                    cat.get("color").and_then(|v| v.as_str()),
                    cat.get("icon").and_then(|v| v.as_str()),
                    cat.get("display_order").and_then(|v| v.as_i64()).unwrap_or(0),
                    cat.get("archived_at").and_then(|v| v.as_str()),
                ],
            )
        } else {
            tx.execute(
                "INSERT INTO categories(name, type, color, icon, display_order, archived_at)
                 VALUES(?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    cat.get("name").and_then(|v| v.as_str()).unwrap_or(""),
                    cat.get("type").and_then(|v| v.as_str()).unwrap_or("expense"),
                    cat.get("color").and_then(|v| v.as_str()),
                    cat.get("icon").and_then(|v| v.as_str()),
                    cat.get("display_order").and_then(|v| v.as_i64()).unwrap_or(0),
                    cat.get("archived_at").and_then(|v| v.as_str()),
                ],
            )
        };
        match result {
            Ok(_) => category_count += 1,
            Err(rusqlite::Error::SqliteFailure(err, _)) if err.code == rusqlite::ErrorCode::ConstraintViolation => {
                warnings.push(format!(
                    "skipped duplicate category: {}",
                    cat.get("name").and_then(|v| v.as_str()).unwrap_or("?")
                ));
            }
            Err(e) => return Err(AppError::Db(e)),
        }
    }

    // Insert accounts.
    for acc in &snap.accounts {
        let result = if mode == "overwrite" {
            tx.execute(
                "INSERT INTO accounts(id, name, kind, currency, initial_balance, display_order, note,
                                       archived_at, created_at, updated_at)
                 VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    acc.get("id").and_then(|v| v.as_i64()),
                    acc.get("name").and_then(|v| v.as_str()).unwrap_or(""),
                    acc.get("kind").and_then(|v| v.as_str()).unwrap_or("cash"),
                    acc.get("currency").and_then(|v| v.as_str()).unwrap_or("JPY"),
                    acc.get("initial_balance").and_then(|v| v.as_i64()).unwrap_or(0),
                    acc.get("display_order").and_then(|v| v.as_i64()).unwrap_or(0),
                    acc.get("note").and_then(|v| v.as_str()).unwrap_or(""),
                    acc.get("archived_at").and_then(|v| v.as_str()),
                    acc.get("created_at").and_then(|v| v.as_str()).unwrap_or("1970-01-01T00:00:00Z"),
                    acc.get("updated_at").and_then(|v| v.as_str()).unwrap_or("1970-01-01T00:00:00Z"),
                ],
            )
        } else {
            tx.execute(
                "INSERT INTO accounts(name, kind, currency, initial_balance, display_order, note,
                                      archived_at, created_at, updated_at)
                 VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    acc.get("name").and_then(|v| v.as_str()).unwrap_or(""),
                    acc.get("kind").and_then(|v| v.as_str()).unwrap_or("cash"),
                    acc.get("currency").and_then(|v| v.as_str()).unwrap_or("JPY"),
                    acc.get("initial_balance").and_then(|v| v.as_i64()).unwrap_or(0),
                    acc.get("display_order").and_then(|v| v.as_i64()).unwrap_or(0),
                    acc.get("note").and_then(|v| v.as_str()).unwrap_or(""),
                    acc.get("archived_at").and_then(|v| v.as_str()),
                    acc.get("created_at").and_then(|v| v.as_str()).unwrap_or("1970-01-01T00:00:00Z"),
                    acc.get("updated_at").and_then(|v| v.as_str()).unwrap_or("1970-01-01T00:00:00Z"),
                ],
            )
        };
        match result {
            Ok(_) => account_count += 1,
            Err(e) => return Err(AppError::Db(e)),
        }
    }

    // Insert transactions; in append mode, drop rows whose FK can't resolve.
    for t in &snap.transactions {
        let category_id = t.get("category_id").and_then(|v| v.as_i64());
        let account_id = t.get("account_id").and_then(|v| v.as_i64()).unwrap_or(0);
        let counter_account_id = t.get("counter_account_id").and_then(|v| v.as_i64());
        if mode == "append" {
            if let Some(cat) = category_id {
                let exists: i64 = tx.query_row(
                    "SELECT COUNT(*) FROM categories WHERE id = ?1",
                    params![cat],
                    |r| r.get(0),
                )?;
                if exists == 0 {
                    warnings.push(format!("skipped transaction referencing missing category {cat}"));
                    continue;
                }
            }
            let acc_exists: i64 = tx.query_row(
                "SELECT COUNT(*) FROM accounts WHERE id = ?1",
                params![account_id],
                |r| r.get(0),
            )?;
            if acc_exists == 0 {
                warnings.push(format!("skipped transaction referencing missing account {account_id}"));
                continue;
            }
        }
        let result = if mode == "overwrite" {
            tx.execute(
                "INSERT INTO transactions(id, occurred_on, type, amount, account_id,
                                          counter_account_id, category_id, description,
                                          recurring_id, created_at, updated_at)
                 VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                params![
                    t.get("id").and_then(|v| v.as_i64()),
                    t.get("occurred_on").and_then(|v| v.as_str()).unwrap_or(""),
                    t.get("type").and_then(|v| v.as_str()).unwrap_or("expense"),
                    t.get("amount").and_then(|v| v.as_i64()).unwrap_or(0),
                    account_id,
                    counter_account_id,
                    category_id,
                    t.get("description").and_then(|v| v.as_str()).unwrap_or(""),
                    t.get("recurring_id").and_then(|v| v.as_i64()),
                    t.get("created_at").and_then(|v| v.as_str()).unwrap_or("1970-01-01T00:00:00Z"),
                    t.get("updated_at").and_then(|v| v.as_str()).unwrap_or("1970-01-01T00:00:00Z"),
                ],
            )
        } else {
            tx.execute(
                "INSERT INTO transactions(occurred_on, type, amount, account_id,
                                          counter_account_id, category_id, description,
                                          recurring_id, created_at, updated_at)
                 VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    t.get("occurred_on").and_then(|v| v.as_str()).unwrap_or(""),
                    t.get("type").and_then(|v| v.as_str()).unwrap_or("expense"),
                    t.get("amount").and_then(|v| v.as_i64()).unwrap_or(0),
                    account_id,
                    counter_account_id,
                    category_id,
                    t.get("description").and_then(|v| v.as_str()).unwrap_or(""),
                    t.get("recurring_id").and_then(|v| v.as_i64()),
                    t.get("created_at").and_then(|v| v.as_str()).unwrap_or("1970-01-01T00:00:00Z"),
                    t.get("updated_at").and_then(|v| v.as_str()).unwrap_or("1970-01-01T00:00:00Z"),
                ],
            )
        };
        match result {
            Ok(_) => transaction_count += 1,
            Err(e) => return Err(AppError::Db(e)),
        }
    }

    // Replay app_meta only in overwrite mode (to preserve schema_version + seed flag).
    if mode == "overwrite" {
        for m in &snap.app_meta {
            let key = m.get("key").and_then(|v| v.as_str()).unwrap_or("");
            let value = m.get("value").and_then(|v| v.as_str()).unwrap_or("");
            if key.is_empty() { continue; }
            // schema_version must reflect current code, not snapshot.
            if key == "schema_version" { continue; }
            tx.execute(
                "INSERT INTO app_meta(key, value) VALUES(?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![key, value],
            )?;
        }
    }

    // Always set last_backup_at to now when the operation finishes successfully.
    meta_repo::set(&tx, "last_backup_at", &chrono::Utc::now().to_rfc3339())?;

    tx.commit()?;
    drop(conn);

    emit_changed(&app, ChangedDomain::Categories);
    emit_changed(&app, ChangedDomain::Accounts);
    emit_changed(&app, ChangedDomain::Transactions);
    emit_changed(&app, ChangedDomain::Meta);

    Ok(ImportResult {
        categories: category_count,
        accounts: account_count,
        transactions: transaction_count,
        warnings,
    })
}
```

- [ ] **Step 2: Register handlers**

In `lib.rs` `generate_handler!`:

```rust
    commands::backup::export_json,
    commands::backup::import_json,
```

- [ ] **Step 3: Build + clippy**

```bash
cargo build
cargo clippy --all-targets -- -D warnings
```

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/commands/backup.rs src-tauri/src/lib.rs
git commit -m "feat(commands): JSON export + import (overwrite/append) with rollback"
```

---

### Task 2: Backup roundtrip integration test

**Files:**
- Create: `src-tauri/tests/integration_backup.rs`

This test exercises the export/import logic through the *command* layer in-process. It uses `tauri::test::mock_builder` to set up a minimal app with `AppState`.

- [ ] **Step 1: Write the test**

```rust
use budget_tracker_lib::commands::backup;
use budget_tracker_lib::commands::meta::AppState;
use budget_tracker_lib::domain::account::AccountKind;
use budget_tracker_lib::domain::category::CategoryType;
use budget_tracker_lib::domain::ledger::TxType;
use budget_tracker_lib::infra::migrations;
use budget_tracker_lib::infra::repo::{account_repo, category_repo, transaction_repo};
use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::Manager;

const NOW: &str = "2026-05-25T00:00:00+00:00";

fn make_app() -> tauri::App<tauri::test::MockRuntime> {
    let mut conn = Connection::open_in_memory().unwrap();
    migrations::run(&mut conn).unwrap();

    let acc = account_repo::insert(&conn, &account_repo::InsertInput {
        name: "現金", kind: AccountKind::Cash, currency: "JPY", initial_balance: 1000,
        display_order: 0, note: "", now: NOW,
    }).unwrap();
    let cat = category_repo::insert(&conn, &category_repo::InsertInput {
        name: "食費", type_: CategoryType::Expense, color: Some("#FF0000"),
        icon: None, display_order: 0,
    }).unwrap();
    transaction_repo::insert(&conn, &transaction_repo::InsertInput {
        occurred_on: "2026-05-15", type_: TxType::Expense, amount: 500,
        account_id: acc, category_id: cat, description: "ランチ", now: NOW,
    }).unwrap();

    let app = tauri::test::mock_app();
    app.manage(AppState {
        conn: Mutex::new(conn),
        db_path: PathBuf::from(":memory:"),
    });
    app
}

#[test]
fn export_then_overwrite_import_restores_identical_state() {
    let app = make_app();
    let state = app.state::<AppState>();

    let snapshot = backup::export_json(state.clone()).unwrap();
    assert!(snapshot.contains("食費"));
    assert!(snapshot.contains("ランチ"));

    // Mutate the DB after exporting.
    {
        let conn = state.conn.lock().unwrap();
        conn.execute("DELETE FROM transactions", []).unwrap();
        conn.execute("DELETE FROM categories", []).unwrap();
        conn.execute("DELETE FROM accounts", []).unwrap();
    }

    let result = backup::import_json(
        app.handle().clone(),
        state.clone(),
        backup::ImportArgs { payload: snapshot, mode: "overwrite".into() },
    )
    .unwrap();
    assert!(result.warnings.is_empty());
    assert_eq!(result.categories, 1);
    assert_eq!(result.accounts, 1);
    assert_eq!(result.transactions, 1);

    let conn = state.conn.lock().unwrap();
    let cats = category_repo::list(&conn, &category_repo::ListFilter::default()).unwrap();
    let accs = account_repo::list(&conn, false).unwrap();
    let (txs, total) = transaction_repo::list(
        &conn,
        &transaction_repo::ListFilter::default(),
        0, 50,
    ).unwrap();
    assert_eq!(cats.len(), 1);
    assert_eq!(accs.len(), 1);
    assert_eq!(total, 1);
    assert_eq!(txs[0].description, "ランチ");
}

#[test]
fn append_import_skips_rows_with_missing_fk() {
    let app = make_app();
    let state = app.state::<AppState>();

    // Snapshot referencing a nonexistent category_id.
    let bogus = r#"{
      "schema_version": 1,
      "exported_at": "2026-05-25T00:00:00Z",
      "categories": [],
      "accounts": [],
      "transactions": [
        {"occurred_on": "2026-05-01", "type": "expense", "amount": 100,
         "account_id": 999, "counter_account_id": null, "category_id": 999,
         "description": "ghost", "recurring_id": null,
         "created_at": "2026-05-01T00:00:00Z", "updated_at": "2026-05-01T00:00:00Z"}
      ],
      "app_meta": []
    }"#;
    let result = backup::import_json(
        app.handle().clone(),
        state.clone(),
        backup::ImportArgs { payload: bogus.into(), mode: "append".into() },
    )
    .unwrap();
    assert_eq!(result.transactions, 0);
    assert!(!result.warnings.is_empty());
}
```

> **Note:** `tauri::test::mock_app` and `tauri::test::MockRuntime` ship with Tauri 2's `test` module. Enable the `test` feature for the `tauri` dev-dep in `src-tauri/Cargo.toml`:
> ```toml
> [dev-dependencies]
> tauri = { version = "2", features = ["test"] }
> ```
> The `pub mod` change to `lib.rs` was done in slice 01.

- [ ] **Step 2: Run**

```bash
cargo test --test integration_backup
```

Expected: PASS.

- [ ] **Step 3: Clippy**

```bash
cargo clippy --all-targets -- -D warnings
```

- [ ] **Step 4: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/tests/integration_backup.rs
git commit -m "test(backup): roundtrip + append FK-skip cases via mock tauri app"
```

---

### Task 3: Frontend `lib/api/backup.ts` + `lib/api/settings.ts`

**Files:**
- Create: `src/lib/api/backup.ts`
- Create: `src/lib/api/settings.ts`
- Modify: `src/lib/api/index.ts`

- [ ] **Step 1: backup wrapper**

```ts
// src/lib/api/backup.ts
import { invoke } from '@tauri-apps/api/core';

export type ImportMode = 'overwrite' | 'append';

export type ImportResult = {
  categories: number;
  accounts: number;
  transactions: number;
  warnings: string[];
};

export function exportJson(): Promise<string> {
  return invoke<string>('export_json');
}

export function importJson(payload: string, mode: ImportMode): Promise<ImportResult> {
  return invoke<ImportResult>('import_json', { args: { payload, mode } });
}
```

- [ ] **Step 2: settings wrapper + Rust handlers**

First, create the Rust handlers — `src-tauri/src/commands/settings.rs`:

```rust
use tauri::State;

use crate::commands::meta::AppState;
use crate::error::{AppError, AppResult};
use crate::infra::repo::meta_repo;

#[tauri::command]
pub fn get_last_backup_at(state: State<'_, AppState>) -> AppResult<Option<String>> {
    let conn = state.conn.lock().map_err(|_| AppError::Corrupt("conn poisoned".into()))?;
    meta_repo::get(&conn, "last_backup_at")
}

#[tauri::command]
pub fn set_last_backup_at(state: State<'_, AppState>, iso: String) -> AppResult<()> {
    let conn = state.conn.lock().map_err(|_| AppError::Corrupt("conn poisoned".into()))?;
    meta_repo::set(&conn, "last_backup_at", &iso)
}

#[tauri::command]
pub fn get_db_path(state: State<'_, AppState>) -> AppResult<String> {
    Ok(state.db_path.to_string_lossy().to_string())
}
```

Then register in `lib.rs`:

```rust
    commands::settings::get_last_backup_at,
    commands::settings::set_last_backup_at,
    commands::settings::get_db_path,
```

- [ ] **Step 3: settings api wrapper**

```ts
// src/lib/api/settings.ts
import { invoke } from '@tauri-apps/api/core';

export function getLastBackupAt(): Promise<string | null> {
  return invoke<string | null>('get_last_backup_at');
}

export function setLastBackupAt(iso: string): Promise<void> {
  return invoke('set_last_backup_at', { iso });
}

export function getDbPath(): Promise<string> {
  return invoke<string>('get_db_path');
}
```

- [ ] **Step 4: Update barrel**

Append to `src/lib/api/index.ts`:

```ts
export * from './backup';
export * from './settings';
```

- [ ] **Step 5: Build + check**

```bash
cargo build
cargo clippy --all-targets -- -D warnings
pnpm check
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/commands/settings.rs src-tauri/src/lib.rs src/lib/api
git commit -m "feat(settings): expose backup/restore wrappers and meta accessors"
```

---

### Task 4: `routes/Settings.svelte`

**Files:**
- Modify: `src/routes/Settings.svelte`

- [ ] **Step 1: Replace placeholder**

```svelte
<script lang="ts">
  import { onMount } from 'svelte';
  import Card from '../lib/components/Card.svelte';
  import Button from '../lib/components/Button.svelte';
  import { exportJson, importJson, type ImportMode, type ImportResult } from '../lib/api/backup';
  import { getDbPath, getLastBackupAt, setLastBackupAt } from '../lib/api/settings';

  let dbPath = $state('');
  let lastBackup = $state<string | null>(null);
  let busy = $state(false);
  let message = $state<string | null>(null);
  let mode = $state<ImportMode>('append');
  let warnings = $state<string[]>([]);
  let importStats = $state<ImportResult | null>(null);

  let fileInput: HTMLInputElement | null = null;

  onMount(async () => {
    dbPath = await getDbPath();
    lastBackup = await getLastBackupAt();
  });

  async function doExport() {
    busy = true;
    message = null;
    try {
      const json = await exportJson();
      const blob = new Blob([json], { type: 'application/json' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      const stamp = new Date().toISOString().replace(/[:.]/g, '-');
      a.href = url;
      a.download = `budget-backup-${stamp}.json`;
      a.click();
      URL.revokeObjectURL(url);
      const now = new Date().toISOString();
      await setLastBackupAt(now);
      lastBackup = now;
      message = 'バックアップを書き出しました';
    } catch (e) {
      message = `エクスポート失敗: ${e instanceof Error ? e.message : String(e)}`;
    } finally {
      busy = false;
    }
  }

  async function onFileChosen(e: Event) {
    const input = e.target as HTMLInputElement;
    const file = input.files?.[0];
    if (!file) return;
    if (file.size > 10 * 1024 * 1024) {
      message = 'ファイルサイズが 10MB を超えています';
      input.value = '';
      return;
    }
    if (mode === 'overwrite' && !confirm('現在のデータをすべて置き換えます。続行しますか?')) {
      input.value = '';
      return;
    }
    busy = true;
    message = null;
    importStats = null;
    warnings = [];
    try {
      const text = await file.text();
      const result = await importJson(text, mode);
      importStats = result;
      warnings = result.warnings;
      message = `読み込み完了: カテゴリ ${result.categories} / 口座 ${result.accounts} / 取引 ${result.transactions}`;
      lastBackup = await getLastBackupAt();
    } catch (e) {
      message = `インポート失敗: ${e instanceof Error ? e.message : String(e)}`;
    } finally {
      busy = false;
      input.value = '';
    }
  }
</script>

<section>
  <h1>設定</h1>

  <Card>
    {#snippet children()}
      <h2>データ</h2>
      <dl>
        <dt>DB ファイル</dt>
        <dd data-testid="settings-db-path">{dbPath || '読み込み中…'}</dd>
        <dt>最終バックアップ</dt>
        <dd data-testid="settings-last-backup">{lastBackup ?? '未実施'}</dd>
      </dl>
    {/snippet}
  </Card>

  <Card>
    {#snippet children()}
      <h2>バックアップ</h2>
      <p>JSON で全データをエクスポート / 復元します。鍵紛失時の復旧手段なので、定期的にバックアップしてください。</p>
      <div class="actions">
        <Button onclick={doExport} disabled={busy}>
          {#snippet children()}JSON をエクスポート{/snippet}
        </Button>
      </div>

      <h3>インポート</h3>
      <label class="mode">
        <input type="radio" name="mode" value="append" bind:group={mode} />
        追記 (既存データは保持、FK解決できない取引はスキップ)
      </label>
      <label class="mode">
        <input type="radio" name="mode" value="overwrite" bind:group={mode} />
        上書き (現在のデータをすべて置き換え)
      </label>
      <div class="actions">
        <input
          type="file"
          accept="application/json"
          bind:this={fileInput}
          onchange={onFileChosen}
          disabled={busy}
          data-testid="settings-import-file"
        />
      </div>

      {#if message}<p class="message">{message}</p>{/if}
      {#if warnings.length > 0}
        <details>
          <summary>{warnings.length} 件の警告</summary>
          <ul>
            {#each warnings as w}<li>{w}</li>{/each}
          </ul>
        </details>
      {/if}
    {/snippet}
  </Card>
</section>

<style>
  h1 { color: white; }
  h2 { margin-top: 0; }
  dl { display: grid; grid-template-columns: 160px 1fr; gap: var(--space-2) var(--space-4); }
  dt { color: var(--muted); }
  dd { margin: 0; font-variant-numeric: tabular-nums; word-break: break-all; }
  .actions { margin: var(--space-3) 0; display: flex; gap: var(--space-3); }
  .mode { display: flex; gap: var(--space-2); align-items: center; padding: var(--space-1) 0; }
  .message { padding: var(--space-3); background: rgba(0,0,0,0.04); border-radius: var(--radius-sm); }
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

- Go to 設定; verify DB path renders and "未実施" shows.
- Click `JSON をエクスポート`; the download should save a `budget-backup-…json` file and the page should now show a timestamp.
- Choose `追記` and pick the just-downloaded file; the message should show counts and warnings (likely "skipped duplicate category…").
- Switch to `上書き` and re-import the file; counts should match the originals and warnings should be empty.

Stop the dev server.

- [ ] **Step 4: Commit**

```bash
git add src/routes/Settings.svelte
git commit -m "feat(ui): settings page with JSON backup/restore and DB info"
```

---

### Task 5: Playwright E2E happy path

**Files:**
- Create: `tests/e2e/transaction-flow.spec.ts`

> **Caveat:** Playwright drives the browser dev server (`pnpm dev`), which has no Tauri runtime, so `invoke` calls will fail. The intent of this E2E is the **frontend flow** — that pages render, navigate, the modal opens, the form validates. We mock `invoke` at `window.__TAURI_INTERNALS__.invoke` before the page loads.

- [ ] **Step 1: Write the spec**

```ts
import { test, expect } from '@playwright/test';

const seededCategories = [
  { id: 1, name: '食費', type: 'expense', color: null, icon: null, display_order: 0, archived_at: null },
  { id: 2, name: '給与', type: 'income',  color: null, icon: null, display_order: 0, archived_at: null },
];
const seededAccounts = [
  { id: 1, name: '現金', kind: 'cash', currency: 'JPY', initial_balance: 0, display_order: 0,
    note: '', archived_at: null, created_at: '2026-05-25T00:00:00Z', updated_at: '2026-05-25T00:00:00Z' },
];

test('happy path: dashboard → add transaction → see total update', async ({ page }) => {
  await page.addInitScript(({ cats, accs }) => {
    const state = {
      cats: structuredClone(cats),
      accs: structuredClone(accs),
      txs: [] as any[],
      nextId: 100,
    };
    const listeners: Array<(payload: { domain: string }) => void> = [];

    (window as any).__TAURI_INTERNALS__ = {
      transformCallback: (cb: any) => cb,
      invoke: async (cmd: string, args: any) => {
        switch (cmd) {
          case 'app_info':
            return { schema_version: 1, db_path: '/tmp/data.db' };
          case 'list_categories':
            return state.cats;
          case 'list_accounts':
            return state.accs;
          case 'list_transactions': {
            return { items: [...state.txs].reverse().slice(args.page * args.pageSize, (args.page + 1) * args.pageSize), total: state.txs.length };
          }
          case 'monthly_summary': {
            const income = state.txs.filter(t => t.type === 'income').reduce((a, t) => a + t.amount, 0);
            const expense = state.txs.filter(t => t.type === 'expense').reduce((a, t) => a + t.amount, 0);
            return { income, expense, net: income - expense, by_category: [] };
          }
          case 'monthly_series':
            return Array.from({ length: args.months }, (_, i) => ({ year_month: `2026-${String(i + 1).padStart(2, '0')}`, income: 0, expense: 0 }));
          case 'create_transaction': {
            const tx = {
              id: state.nextId++,
              ...args.input,
              counter_account_id: null,
              recurring_id: null,
              created_at: '2026-05-25T00:00:00Z',
              updated_at: '2026-05-25T00:00:00Z',
            };
            state.txs.push(tx);
            listeners.forEach((cb) => cb({ domain: 'transactions' }));
            return tx;
          }
          case 'get_db_path': return '/tmp/data.db';
          case 'get_last_backup_at': return null;
          default: return null;
        }
      },
    };
    (window as any).__TAURI_EVENT__ = {
      listen: async (event: string, cb: any) => {
        if (event === 'data:changed') listeners.push((p: any) => cb({ payload: p }));
        return () => {};
      },
    };
  }, { cats: seededCategories, accs: seededAccounts });

  await page.goto('/');
  await expect(page.getByRole('heading', { name: 'ダッシュボード' })).toBeVisible();
  await expect(page.getByTestId('card-income')).toBeVisible();

  await page.getByTestId('nav-transactions').click();
  await expect(page.getByTestId('tx-table').or(page.getByText('該当する取引がありません'))).toBeVisible();

  await page.getByRole('button', { name: '+ 取引を追加' }).click();
  await page.getByTestId('tx-amount').fill('1500');
  await page.getByTestId('tx-description').fill('テスト');
  await page.getByRole('button', { name: '追加' }).click();

  await expect(page.getByText('テスト')).toBeVisible();

  await page.getByTestId('nav-dashboard').click();
  await expect(page.getByTestId('card-expense')).toContainText('1,500');
});
```

- [ ] **Step 2: Run E2E**

```bash
pnpm test:e2e
```

Expected: PASS. If the existing `tests/e2e/smoke.spec.ts` from slice 01 fails because of the mocked invoke environment, gate its assertions behind the same `__TAURI_INTERNALS__` mock or simplify it to verify only that the sidebar renders (the foundation slice already wired its assertions to the new shell).

- [ ] **Step 3: Commit**

```bash
git add tests/e2e/transaction-flow.spec.ts
git commit -m "test(e2e): happy path through transactions + dashboard via mocked invoke"
```

---

### Task 6: Phase 2 final DoD sweep

- [ ] **Step 1: Clippy**

```bash
cd src-tauri && cargo clippy --all-targets -- -D warnings
```

Expected: PASS.

- [ ] **Step 2: Rust tests**

```bash
cargo test
```

Expected: PASS (all unit + proptest + integration).

- [ ] **Step 3: Frontend type-check + tests**

From repo root:

```bash
pnpm check
pnpm test
```

Expected: PASS.

- [ ] **Step 4: E2E**

```bash
pnpm test:e2e
```

Expected: PASS.

- [ ] **Step 5: Manual smoke (per spec DoD)**

```bash
pnpm tauri dev
```

Walk through:

- Default categories present on first launch (15 items).
- Add an account ("現金", cash, 0).
- Add an income (給与, 300_000) and an expense (食費, 1_500) for today.
- Dashboard cards show income=¥300,000 / expense=¥1,500 / net=¥298,500.
- Bar chart shows the current month's bars.
- 取引ページのフィルタが種別/期間/メモで絞れる。
- 設定で JSON エクスポート → ファイル保存 → 上書きインポート → 同じ状態に戻る (差分は最終バックアップ日時のみ)。
- sqlite シェルで `INSERT INTO transactions(...) VALUES (..., 'transfer', ...)` を入れて再起動 → ダッシュボード集計に含まれず、取引リストでも編集ボタンが出ないこと。

- [ ] **Step 6: Verify CI builds (macOS + Windows)**

Push the branch to GitHub and let `.github/workflows/build.yml` (Phase 1 in `2960bf2`) run. Both `macos-latest` and `windows-latest` jobs must finish green. This is the last CLAUDE.md DoD item.

```bash
git push -u origin <branch>
gh pr create --title "Phase 2: transactions, categories, accounts, dashboard, backup" \
  --body "$(cat <<'EOF'
## Summary
- Implements Phase 2 of [the Phase 2 design](docs/superpowers/specs/2026-05-25-phase2-transactions-design.md).
- Categories + accounts + income/expense transaction CRUD with filtering & pagination.
- Dashboard cards + 12-month bar chart + recent transactions feed.
- JSON export/import (overwrite + append) with FK warnings.
- First-run default-category seed gated by app_meta flag.
- Reject 'transfer' rows at the validator layer; SQL aggregates exclude them.

## Test plan
- [ ] cargo clippy clean
- [ ] cargo test green (incl. proptest + integration + backup roundtrip)
- [ ] pnpm test green
- [ ] pnpm check green
- [ ] pnpm test:e2e green (browser happy path)
- [ ] macOS + Windows CI build green
- [ ] Manual: default categories visible on first launch
- [ ] Manual: end-to-end (add account → add income/expense → see dashboard cards + chart update)
- [ ] Manual: JSON export → overwrite import → state identical
- [ ] Manual: sqlite-injected transfer row is excluded from cards/list edits
EOF
)"
```

- [ ] **Step 7: Update project memory once merged**

After merge, update `~/.claude/projects/.../memory/project_phase_status.md` to mark Phase 2 complete and point at the merge commit.

---

### Phase 2 DoD (gate to Phase 3)

- [ ] All slice DoDs (01–06) checked off.
- [ ] CI matrix green on macOS + Windows.
- [ ] Spec sections 1, 2, 3, 4, 5, 6, 7, 8, 9 of the Phase 2 design verifiably implemented.
- [ ] PR merged; project memory updated.
