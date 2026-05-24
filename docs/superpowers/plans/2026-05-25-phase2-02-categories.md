# Phase 2 — Slice 02: Categories

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this slice task-by-task.

**Goal:** Implement category CRUD end-to-end (domain validators → SQL repo → Tauri commands → typed API → reactive store → page UI), plus first-run default category seed and the shared UI primitives (`Card`, `Button`, `Modal`, `TextField`, `Select`, `CategoryBadge`).

**Prerequisite:** Slice 01 (foundation) completed.

**Spec:** sections 4.1, 5.1, 7 of `docs/superpowers/specs/2026-05-25-phase2-transactions-design.md`.

---

### Task 1: `domain/category.rs` types and validators (TDD)

**Files:**
- Modify: `src-tauri/src/domain/category.rs`

- [ ] **Step 1: Write failing tests**

Replace the stub content of `src-tauri/src/domain/category.rs` with:

```rust
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CategoryType {
    Income,
    Expense,
}

impl CategoryType {
    pub fn as_sql(self) -> &'static str {
        match self {
            CategoryType::Income => "income",
            CategoryType::Expense => "expense",
        }
    }

    pub fn parse(raw: &str) -> AppResult<Self> {
        match raw {
            "income" => Ok(Self::Income),
            "expense" => Ok(Self::Expense),
            other => Err(AppError::InvalidArgument(format!(
                "category type must be 'income' or 'expense', got '{other}'"
            ))),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Category {
    pub id: i64,
    pub name: String,
    #[serde(rename = "type")]
    pub type_: CategoryType,
    pub color: Option<String>,
    pub icon: Option<String>,
    pub display_order: i64,
    pub archived_at: Option<String>,
}

const MAX_NAME_LEN: usize = 40;

pub fn validate_name(raw: &str) -> AppResult<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(AppError::InvalidArgument("category name is empty".into()));
    }
    if trimmed.chars().count() > MAX_NAME_LEN {
        return Err(AppError::InvalidArgument(format!(
            "category name must be {MAX_NAME_LEN} chars or fewer"
        )));
    }
    Ok(trimmed.to_string())
}

pub fn validate_color(raw: &str) -> AppResult<String> {
    let ok = raw.len() == 7
        && raw.starts_with('#')
        && raw[1..].chars().all(|c| c.is_ascii_hexdigit());
    if !ok {
        return Err(AppError::InvalidArgument(format!(
            "color must match #RRGGBB hex, got '{raw}'"
        )));
    }
    Ok(raw.to_ascii_uppercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_known_types() {
        assert_eq!(CategoryType::parse("income").unwrap(), CategoryType::Income);
        assert_eq!(CategoryType::parse("expense").unwrap(), CategoryType::Expense);
    }

    #[test]
    fn rejects_unknown_type() {
        let err = CategoryType::parse("transfer").unwrap_err();
        assert!(matches!(err, AppError::InvalidArgument(_)));
    }

    #[test]
    fn validate_name_trims_and_accepts_jp() {
        assert_eq!(validate_name("  食費  ").unwrap(), "食費");
    }

    #[test]
    fn validate_name_rejects_empty_and_whitespace_only() {
        assert!(validate_name("").is_err());
        assert!(validate_name("   ").is_err());
    }

    #[test]
    fn validate_name_rejects_over_40_chars() {
        let name = "あ".repeat(41);
        assert!(validate_name(&name).is_err());
    }

    #[test]
    fn validate_color_accepts_lower_and_uppercase() {
        assert_eq!(validate_color("#ff00aa").unwrap(), "#FF00AA");
        assert_eq!(validate_color("#FF00AA").unwrap(), "#FF00AA");
    }

    #[test]
    fn validate_color_rejects_short_and_non_hex() {
        assert!(validate_color("#FFF").is_err());
        assert!(validate_color("FF00AA").is_err());
        assert!(validate_color("#GGGGGG").is_err());
    }
}
```

- [ ] **Step 2: Run tests**

```bash
cargo test --lib domain::category
```

Expected: all PASS (validators implemented inline with tests in step 1 to keep the slice moving — single commit). If a test fails, edit the implementation, not the tests.

- [ ] **Step 3: Run clippy**

```bash
cargo clippy --all-targets -- -D warnings
```

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/domain/category.rs
git commit -m "feat(domain): category types and name/color validators"
```

---

### Task 2: `infra/repo/category_repo.rs`

**Files:**
- Create: `src-tauri/src/infra/repo/category_repo.rs`
- Modify: `src-tauri/src/infra/repo/mod.rs`

- [ ] **Step 1: Wire the module**

In `src-tauri/src/infra/repo/mod.rs`, add:

```rust
pub mod category_repo;
```

- [ ] **Step 2: Implement the repo**

```rust
// src-tauri/src/infra/repo/category_repo.rs
use rusqlite::{Connection, OptionalExtension, params};

use crate::domain::category::{Category, CategoryType};
use crate::error::{AppError, AppResult};

#[derive(Debug, Default)]
pub struct ListFilter {
    pub type_: Option<CategoryType>,
    pub include_archived: bool,
}

fn row_to_category(row: &rusqlite::Row<'_>) -> rusqlite::Result<Category> {
    let type_raw: String = row.get("type")?;
    Ok(Category {
        id: row.get("id")?,
        name: row.get("name")?,
        type_: match type_raw.as_str() {
            "income" => CategoryType::Income,
            "expense" => CategoryType::Expense,
            other => {
                return Err(rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Text,
                    format!("unknown category type '{other}'").into(),
                ));
            }
        },
        color: row.get("color")?,
        icon: row.get("icon")?,
        display_order: row.get("display_order")?,
        archived_at: row.get("archived_at")?,
    })
}

pub fn list(conn: &Connection, filter: &ListFilter) -> AppResult<Vec<Category>> {
    let mut sql = String::from(
        "SELECT id, name, type, color, icon, display_order, archived_at FROM categories",
    );
    let mut clauses: Vec<&str> = Vec::new();
    if !filter.include_archived {
        clauses.push("archived_at IS NULL");
    }
    let type_clause = filter.type_.map(|t| match t {
        CategoryType::Income => "type = 'income'",
        CategoryType::Expense => "type = 'expense'",
    });
    if let Some(c) = type_clause {
        clauses.push(c);
    }
    if !clauses.is_empty() {
        sql.push_str(" WHERE ");
        sql.push_str(&clauses.join(" AND "));
    }
    sql.push_str(" ORDER BY display_order ASC, id ASC");

    let mut stmt = conn.prepare(&sql)?;
    let items = stmt
        .query_map([], row_to_category)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(items)
}

pub fn find_by_id(conn: &Connection, id: i64) -> AppResult<Category> {
    let cat = conn
        .query_row(
            "SELECT id, name, type, color, icon, display_order, archived_at
               FROM categories WHERE id = ?1",
            params![id],
            row_to_category,
        )
        .optional()?
        .ok_or_else(|| AppError::NotFound(format!("category {id}")))?;
    Ok(cat)
}

pub fn next_display_order(conn: &Connection, type_: CategoryType) -> AppResult<i64> {
    let max: Option<i64> = conn.query_row(
        "SELECT MAX(display_order) FROM categories WHERE type = ?1",
        params![type_.as_sql()],
        |r| r.get(0),
    )?;
    Ok(max.unwrap_or(-1) + 1)
}

pub struct InsertInput<'a> {
    pub name: &'a str,
    pub type_: CategoryType,
    pub color: Option<&'a str>,
    pub icon: Option<&'a str>,
    pub display_order: i64,
}

pub fn insert(conn: &Connection, input: &InsertInput<'_>) -> AppResult<i64> {
    conn.execute(
        "INSERT INTO categories(name, type, color, icon, display_order)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            input.name,
            input.type_.as_sql(),
            input.color,
            input.icon,
            input.display_order,
        ],
    )
    .map_err(|e| match e {
        rusqlite::Error::SqliteFailure(err, _)
            if err.code == rusqlite::ErrorCode::ConstraintViolation =>
        {
            AppError::Conflict(format!(
                "category '{}' ({}) already exists",
                input.name,
                input.type_.as_sql()
            ))
        }
        other => AppError::Db(other),
    })?;
    Ok(conn.last_insert_rowid())
}

#[derive(Debug, Default)]
pub struct UpdatePatch<'a> {
    pub name: Option<&'a str>,
    pub color: Option<Option<&'a str>>,
    pub icon: Option<Option<&'a str>>,
    pub display_order: Option<i64>,
}

pub fn update(conn: &Connection, id: i64, patch: &UpdatePatch<'_>) -> AppResult<()> {
    if let Some(name) = patch.name {
        conn.execute("UPDATE categories SET name = ?1 WHERE id = ?2", params![name, id])
            .map_err(|e| match e {
                rusqlite::Error::SqliteFailure(err, _)
                    if err.code == rusqlite::ErrorCode::ConstraintViolation =>
                {
                    AppError::Conflict(format!("category name '{name}' already used"))
                }
                other => AppError::Db(other),
            })?;
    }
    if let Some(color) = patch.color {
        conn.execute("UPDATE categories SET color = ?1 WHERE id = ?2", params![color, id])?;
    }
    if let Some(icon) = patch.icon {
        conn.execute("UPDATE categories SET icon = ?1 WHERE id = ?2", params![icon, id])?;
    }
    if let Some(order) = patch.display_order {
        conn.execute(
            "UPDATE categories SET display_order = ?1 WHERE id = ?2",
            params![order, id],
        )?;
    }
    Ok(())
}

pub fn set_archived(conn: &Connection, id: i64, archived_at: Option<&str>) -> AppResult<()> {
    let n = conn.execute(
        "UPDATE categories SET archived_at = ?1 WHERE id = ?2",
        params![archived_at, id],
    )?;
    if n == 0 {
        return Err(AppError::NotFound(format!("category {id}")));
    }
    Ok(())
}
```

- [ ] **Step 3: Build**

```bash
cargo build
```

Expected: PASS.

- [ ] **Step 4: Clippy**

```bash
cargo clippy --all-targets -- -D warnings
```

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/infra/repo
git commit -m "feat(infra): category_repo with list/insert/update/archive"
```

---

### Task 3: Integration test for category_repo

**Files:**
- Create: `src-tauri/tests/integration_categories.rs`

- [ ] **Step 1: Add a test that exercises the full repo through migrations**

```rust
// src-tauri/tests/integration_categories.rs
use budget_tracker_lib::domain::category::CategoryType;
use budget_tracker_lib::infra::migrations;
use budget_tracker_lib::infra::repo::category_repo;
use rusqlite::Connection;

fn fresh_db() -> Connection {
    let mut conn = Connection::open_in_memory().unwrap();
    migrations::run(&mut conn).unwrap();
    conn
}

#[test]
fn insert_then_list_returns_inserted_row() {
    let conn = fresh_db();
    let id = category_repo::insert(
        &conn,
        &category_repo::InsertInput {
            name: "食費",
            type_: CategoryType::Expense,
            color: Some("#FF00AA"),
            icon: None,
            display_order: 0,
        },
    )
    .unwrap();
    let listed = category_repo::list(&conn, &category_repo::ListFilter::default()).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, id);
    assert_eq!(listed[0].name, "食費");
    assert_eq!(listed[0].type_, CategoryType::Expense);
    assert_eq!(listed[0].color.as_deref(), Some("#FF00AA"));
}

#[test]
fn duplicate_name_same_type_returns_conflict() {
    let conn = fresh_db();
    category_repo::insert(
        &conn,
        &category_repo::InsertInput {
            name: "食費",
            type_: CategoryType::Expense,
            color: None,
            icon: None,
            display_order: 0,
        },
    )
    .unwrap();
    let err = category_repo::insert(
        &conn,
        &category_repo::InsertInput {
            name: "食費",
            type_: CategoryType::Expense,
            color: None,
            icon: None,
            display_order: 1,
        },
    )
    .unwrap_err();
    assert!(matches!(err, budget_tracker_lib::error::AppError::Conflict(_)));
}

#[test]
fn same_name_different_type_is_allowed() {
    let conn = fresh_db();
    category_repo::insert(
        &conn,
        &category_repo::InsertInput {
            name: "その他",
            type_: CategoryType::Expense,
            color: None,
            icon: None,
            display_order: 0,
        },
    )
    .unwrap();
    category_repo::insert(
        &conn,
        &category_repo::InsertInput {
            name: "その他",
            type_: CategoryType::Income,
            color: None,
            icon: None,
            display_order: 0,
        },
    )
    .unwrap();
    let all = category_repo::list(&conn, &category_repo::ListFilter::default()).unwrap();
    assert_eq!(all.len(), 2);
}

#[test]
fn archive_filters_from_default_list() {
    let conn = fresh_db();
    let id = category_repo::insert(
        &conn,
        &category_repo::InsertInput {
            name: "副業",
            type_: CategoryType::Income,
            color: None,
            icon: None,
            display_order: 0,
        },
    )
    .unwrap();
    category_repo::set_archived(&conn, id, Some("2026-05-25T00:00:00Z")).unwrap();

    let visible = category_repo::list(&conn, &category_repo::ListFilter::default()).unwrap();
    assert!(visible.is_empty());

    let all = category_repo::list(
        &conn,
        &category_repo::ListFilter { include_archived: true, ..Default::default() },
    )
    .unwrap();
    assert_eq!(all.len(), 1);
}

#[test]
fn filter_by_type() {
    let conn = fresh_db();
    for (name, t) in [("給与", CategoryType::Income), ("食費", CategoryType::Expense)] {
        category_repo::insert(
            &conn,
            &category_repo::InsertInput {
                name,
                type_: t,
                color: None,
                icon: None,
                display_order: 0,
            },
        )
        .unwrap();
    }
    let only_income = category_repo::list(
        &conn,
        &category_repo::ListFilter { type_: Some(CategoryType::Income), ..Default::default() },
    )
    .unwrap();
    assert_eq!(only_income.len(), 1);
    assert_eq!(only_income[0].name, "給与");
}
```

- [ ] **Step 2: Run the test**

```bash
cargo test --test integration_categories
```

Expected: all PASS.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/tests/integration_categories.rs
git commit -m "test(category_repo): cover insert/list/archive/conflict against memory db"
```

---

### Task 4: `commands/categories.rs` — Tauri handlers

**Files:**
- Modify: `src-tauri/src/commands/categories.rs`
- Modify: `src-tauri/src/lib.rs` (register handlers)

- [ ] **Step 1: Implement the handlers**

Replace the stub `commands/categories.rs` with:

```rust
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};

use crate::commands::meta::AppState;
use crate::domain::category::{self, Category, CategoryType};
use crate::error::{AppError, AppResult};
use crate::infra::events::{ChangedDomain, emit_changed};
use crate::infra::repo::category_repo;

fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339()
}

#[derive(Debug, Deserialize)]
pub struct ListFilterInput {
    #[serde(rename = "type")]
    pub type_: Option<String>,
    #[serde(default)]
    pub include_archived: bool,
}

#[tauri::command]
pub fn list_categories(
    state: State<'_, AppState>,
    filter: ListFilterInput,
) -> AppResult<Vec<Category>> {
    let type_ = filter.type_.as_deref().map(CategoryType::parse).transpose()?;
    let conn = state.conn.lock().map_err(|_| AppError::Corrupt("conn poisoned".into()))?;
    category_repo::list(
        &conn,
        &category_repo::ListFilter { type_, include_archived: filter.include_archived },
    )
}

#[derive(Debug, Deserialize)]
pub struct CreateCategoryInput {
    pub name: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub color: Option<String>,
    pub icon: Option<String>,
}

#[tauri::command]
pub fn create_category(
    app: AppHandle,
    state: State<'_, AppState>,
    input: CreateCategoryInput,
) -> AppResult<Category> {
    let name = category::validate_name(&input.name)?;
    let type_ = CategoryType::parse(&input.type_)?;
    let color = match input.color.as_deref() {
        Some(c) => Some(category::validate_color(c)?),
        None => None,
    };
    let conn = state.conn.lock().map_err(|_| AppError::Corrupt("conn poisoned".into()))?;
    let order = category_repo::next_display_order(&conn, type_)?;
    let id = category_repo::insert(
        &conn,
        &category_repo::InsertInput {
            name: &name,
            type_,
            color: color.as_deref(),
            icon: input.icon.as_deref(),
            display_order: order,
        },
    )?;
    let cat = category_repo::find_by_id(&conn, id)?;
    drop(conn);
    emit_changed(&app, ChangedDomain::Categories);
    Ok(cat)
}

#[derive(Debug, Deserialize)]
pub struct UpdateCategoryPatch {
    pub name: Option<String>,
    pub color: Option<Option<String>>,
    pub icon: Option<Option<String>>,
    pub display_order: Option<i64>,
}

#[tauri::command]
pub fn update_category(
    app: AppHandle,
    state: State<'_, AppState>,
    id: i64,
    patch: UpdateCategoryPatch,
) -> AppResult<Category> {
    let name = patch.name.as_deref().map(category::validate_name).transpose()?;
    let color: Option<Option<String>> = match patch.color {
        Some(Some(c)) => Some(Some(category::validate_color(&c)?)),
        Some(None) => Some(None),
        None => None,
    };
    let conn = state.conn.lock().map_err(|_| AppError::Corrupt("conn poisoned".into()))?;
    category_repo::update(
        &conn,
        id,
        &category_repo::UpdatePatch {
            name: name.as_deref(),
            color: color.as_ref().map(|opt| opt.as_deref()),
            icon: patch.icon.as_ref().map(|opt| opt.as_deref()),
            display_order: patch.display_order,
        },
    )?;
    let cat = category_repo::find_by_id(&conn, id)?;
    drop(conn);
    emit_changed(&app, ChangedDomain::Categories);
    Ok(cat)
}

#[tauri::command]
pub fn archive_category(
    app: AppHandle,
    state: State<'_, AppState>,
    id: i64,
) -> AppResult<()> {
    let conn = state.conn.lock().map_err(|_| AppError::Corrupt("conn poisoned".into()))?;
    category_repo::set_archived(&conn, id, Some(&now_iso()))?;
    drop(conn);
    emit_changed(&app, ChangedDomain::Categories);
    Ok(())
}

#[tauri::command]
pub fn unarchive_category(
    app: AppHandle,
    state: State<'_, AppState>,
    id: i64,
) -> AppResult<()> {
    let conn = state.conn.lock().map_err(|_| AppError::Corrupt("conn poisoned".into()))?;
    category_repo::set_archived(&conn, id, None)?;
    drop(conn);
    emit_changed(&app, ChangedDomain::Categories);
    Ok(())
}
```

- [ ] **Step 2: Register handlers in `lib.rs`**

In `src-tauri/src/lib.rs`, extend `.invoke_handler` from:

```rust
.invoke_handler(tauri::generate_handler![app_info])
```

to:

```rust
.invoke_handler(tauri::generate_handler![
    app_info,
    commands::categories::list_categories,
    commands::categories::create_category,
    commands::categories::update_category,
    commands::categories::archive_category,
    commands::categories::unarchive_category,
])
```

- [ ] **Step 3: Build + clippy + test**

```bash
cargo build
cargo clippy --all-targets -- -D warnings
cargo test
```

Expected: all PASS.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/commands/categories.rs src-tauri/src/lib.rs
git commit -m "feat(commands): expose categories CRUD over tauri invoke"
```

---

### Task 5: First-run default-category seed

**Files:**
- Modify: `src-tauri/src/domain/seed.rs`
- Create: `src-tauri/src/infra/repo/meta_repo.rs`
- Modify: `src-tauri/src/infra/repo/mod.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: `meta_repo` for key/value**

Add to `src-tauri/src/infra/repo/mod.rs`:

```rust
pub mod meta_repo;
```

Then create `src-tauri/src/infra/repo/meta_repo.rs`:

```rust
use rusqlite::{Connection, OptionalExtension, params};

use crate::error::AppResult;

pub fn get(conn: &Connection, key: &str) -> AppResult<Option<String>> {
    let v: Option<String> = conn
        .query_row(
            "SELECT value FROM app_meta WHERE key = ?1",
            params![key],
            |r| r.get(0),
        )
        .optional()?;
    Ok(v)
}

pub fn set(conn: &Connection, key: &str, value: &str) -> AppResult<()> {
    conn.execute(
        "INSERT INTO app_meta(key, value) VALUES(?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}
```

- [ ] **Step 2: Seed logic**

Replace `src-tauri/src/domain/seed.rs` with:

```rust
use rusqlite::Connection;

use crate::domain::category::CategoryType;
use crate::error::AppResult;
use crate::infra::repo::{category_repo, meta_repo};

const SEED_FLAG_KEY: &str = "categories_seeded";

const EXPENSE_DEFAULTS: &[&str] = &[
    "食費",
    "日用品",
    "交通費",
    "住居費",
    "水道光熱費",
    "通信費",
    "医療費",
    "衣服",
    "交際費",
    "趣味・娯楽",
    "その他",
];

const INCOME_DEFAULTS: &[&str] = &["給与", "賞与", "副業", "その他"];

/// Seed default categories iff the flag is unset and the table is empty.
/// Always sets the flag afterwards to prevent re-seeding after the user
/// deletes everything intentionally.
pub fn seed_default_categories_if_needed(conn: &Connection) -> AppResult<bool> {
    if meta_repo::get(conn, SEED_FLAG_KEY)?.is_some() {
        return Ok(false);
    }
    let existing = category_repo::list(conn, &category_repo::ListFilter {
        include_archived: true,
        ..Default::default()
    })?;
    if !existing.is_empty() {
        meta_repo::set(conn, SEED_FLAG_KEY, "true")?;
        return Ok(false);
    }
    for (order, name) in EXPENSE_DEFAULTS.iter().enumerate() {
        category_repo::insert(conn, &category_repo::InsertInput {
            name,
            type_: CategoryType::Expense,
            color: None,
            icon: None,
            display_order: order as i64,
        })?;
    }
    for (order, name) in INCOME_DEFAULTS.iter().enumerate() {
        category_repo::insert(conn, &category_repo::InsertInput {
            name,
            type_: CategoryType::Income,
            color: None,
            icon: None,
            display_order: order as i64,
        })?;
    }
    meta_repo::set(conn, SEED_FLAG_KEY, "true")?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infra::migrations;

    fn fresh() -> Connection {
        let mut c = Connection::open_in_memory().unwrap();
        migrations::run(&mut c).unwrap();
        c
    }

    #[test]
    fn seeds_when_table_empty_and_flag_unset() {
        let conn = fresh();
        let seeded = seed_default_categories_if_needed(&conn).unwrap();
        assert!(seeded);
        let cats = category_repo::list(&conn, &category_repo::ListFilter::default()).unwrap();
        assert_eq!(cats.len(), EXPENSE_DEFAULTS.len() + INCOME_DEFAULTS.len());
    }

    #[test]
    fn does_not_seed_twice() {
        let conn = fresh();
        assert!(seed_default_categories_if_needed(&conn).unwrap());
        // User deletes everything.
        conn.execute_batch("DELETE FROM categories;").unwrap();
        assert!(!seed_default_categories_if_needed(&conn).unwrap());
        let cats = category_repo::list(&conn, &category_repo::ListFilter::default()).unwrap();
        assert!(cats.is_empty());
    }

    #[test]
    fn does_not_seed_if_user_already_has_categories() {
        let conn = fresh();
        category_repo::insert(&conn, &category_repo::InsertInput {
            name: "自分カテゴリ",
            type_: CategoryType::Expense,
            color: None,
            icon: None,
            display_order: 0,
        })
        .unwrap();
        let seeded = seed_default_categories_if_needed(&conn).unwrap();
        assert!(!seeded);
        let cats = category_repo::list(&conn, &category_repo::ListFilter::default()).unwrap();
        assert_eq!(cats.len(), 1);
    }
}
```

- [ ] **Step 3: Run seed tests**

```bash
cargo test --lib domain::seed
```

Expected: PASS.

- [ ] **Step 4: Call seed from `lib.rs`**

In `src-tauri/src/lib.rs`, immediately after the existing `let _version = migrations::run(&mut conn)…;` line, add:

```rust
            crate::domain::seed::seed_default_categories_if_needed(&conn)
                .expect("failed to seed default categories");
```

- [ ] **Step 5: Build + clippy**

```bash
cargo build
cargo clippy --all-targets -- -D warnings
```

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/domain/seed.rs src-tauri/src/infra/repo/meta_repo.rs src-tauri/src/infra/repo/mod.rs src-tauri/src/lib.rs
git commit -m "feat(seed): seed default categories once on first run"
```

---

### Task 6: Frontend `lib/api/categories.ts` + barrel + event helper

**Files:**
- Create: `src/lib/api/events.ts`
- Create: `src/lib/api/categories.ts`
- Modify: `src/lib/api/index.ts`

- [ ] **Step 1: Add typed event helper**

```ts
// src/lib/api/events.ts
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

export type ChangedDomain =
  | 'categories'
  | 'accounts'
  | 'transactions'
  | 'meta';

export type DataChangedPayload = { domain: ChangedDomain };

export async function onDataChanged(
  cb: (domain: ChangedDomain) => void,
): Promise<UnlistenFn> {
  return listen<DataChangedPayload>('data:changed', (event) => {
    cb(event.payload.domain);
  });
}
```

- [ ] **Step 2: API wrapper**

```ts
// src/lib/api/categories.ts
import { invoke } from '@tauri-apps/api/core';

export type CategoryType = 'income' | 'expense';

export type Category = {
  id: number;
  name: string;
  type: CategoryType;
  color: string | null;
  icon: string | null;
  display_order: number;
  archived_at: string | null;
};

export type ListCategoryFilter = {
  type?: CategoryType;
  include_archived?: boolean;
};

export function listCategories(filter: ListCategoryFilter = {}): Promise<Category[]> {
  return invoke<Category[]>('list_categories', { filter });
}

export type CreateCategoryInput = {
  name: string;
  type: CategoryType;
  color?: string;
  icon?: string;
};

export function createCategory(input: CreateCategoryInput): Promise<Category> {
  return invoke<Category>('create_category', { input });
}

export type UpdateCategoryPatch = {
  name?: string;
  color?: string | null;
  icon?: string | null;
  display_order?: number;
};

export function updateCategory(id: number, patch: UpdateCategoryPatch): Promise<Category> {
  return invoke<Category>('update_category', { id, patch });
}

export function archiveCategory(id: number): Promise<void> {
  return invoke('archive_category', { id });
}

export function unarchiveCategory(id: number): Promise<void> {
  return invoke('unarchive_category', { id });
}
```

- [ ] **Step 3: Re-export from barrel**

Replace `src/lib/api/index.ts` with:

```ts
import { invoke } from '@tauri-apps/api/core';

export type AppInfo = {
  schema_version: number;
  db_path: string;
};

export async function getAppInfo(): Promise<AppInfo> {
  return invoke<AppInfo>('app_info');
}

export * from './events';
export * from './categories';
```

- [ ] **Step 4: Type-check**

```bash
pnpm check
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/lib/api
git commit -m "feat(api): typed categories wrapper and data:changed listener"
```

---

### Task 7: `categories.svelte.ts` store

**Files:**
- Create: `src/lib/stores/categories.svelte.ts`
- Create: `src/lib/stores/categories.test.ts`

- [ ] **Step 1: Store**

```ts
// src/lib/stores/categories.svelte.ts
import {
  listCategories,
  type Category,
  type ListCategoryFilter,
} from '../api/categories';
import { onDataChanged } from '../api/events';
import type { UnlistenFn } from '@tauri-apps/api/event';

export type CategoriesStore = {
  readonly items: Category[];
  readonly loading: boolean;
  readonly error: string | null;
  load(): Promise<void>;
  setFilter(filter: ListCategoryFilter): void;
  dispose(): Promise<void>;
};

export function createCategoriesStore(initialFilter: ListCategoryFilter = {}): CategoriesStore {
  let items = $state<Category[]>([]);
  let loading = $state(false);
  let error = $state<string | null>(null);
  let filter = $state<ListCategoryFilter>(initialFilter);
  let unlisten: UnlistenFn | null = null;

  async function load() {
    loading = true;
    error = null;
    try {
      items = await listCategories(filter);
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }

  function setFilter(next: ListCategoryFilter) {
    filter = next;
    void load();
  }

  // Subscribe lazily — first read triggers init via load().
  void (async () => {
    unlisten = await onDataChanged((domain) => {
      if (domain === 'categories') void load();
    });
    await load();
  })();

  return {
    get items() { return items; },
    get loading() { return loading; },
    get error() { return error; },
    load,
    setFilter,
    async dispose() {
      if (unlisten) {
        unlisten();
        unlisten = null;
      }
    },
  };
}
```

- [ ] **Step 2: Vitest**

```ts
// src/lib/stores/categories.test.ts
import { describe, expect, it, vi, beforeEach, afterEach } from 'vitest';

const listMock = vi.fn();
const onChangedMock = vi.fn();

vi.mock('../api/categories', () => ({
  listCategories: (...args: unknown[]) => listMock(...args),
}));

vi.mock('../api/events', () => ({
  onDataChanged: (cb: (d: string) => void) => onChangedMock(cb),
}));

import { createCategoriesStore } from './categories.svelte';

describe('categories store', () => {
  beforeEach(() => {
    listMock.mockReset();
    onChangedMock.mockReset();
    onChangedMock.mockImplementation(() => Promise.resolve(() => {}));
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('loads items on construction', async () => {
    listMock.mockResolvedValueOnce([{ id: 1, name: '食費', type: 'expense' }]);
    const store = createCategoriesStore();
    await vi.waitFor(() => expect(store.items).toHaveLength(1));
    expect(store.items[0].name).toBe('食費');
  });

  it('re-fetches when data:changed fires for categories', async () => {
    let trigger: ((domain: string) => void) | undefined;
    onChangedMock.mockImplementation((cb: (d: string) => void) => {
      trigger = cb;
      return Promise.resolve(() => {});
    });
    listMock.mockResolvedValue([]);

    const store = createCategoriesStore();
    await vi.waitFor(() => expect(listMock).toHaveBeenCalledTimes(1));
    listMock.mockResolvedValueOnce([{ id: 2, name: '給与', type: 'income' }]);
    trigger?.('categories');
    await vi.waitFor(() => expect(store.items[0]?.name).toBe('給与'));
  });

  it('ignores unrelated domain changes', async () => {
    let trigger: ((domain: string) => void) | undefined;
    onChangedMock.mockImplementation((cb: (d: string) => void) => {
      trigger = cb;
      return Promise.resolve(() => {});
    });
    listMock.mockResolvedValue([]);
    createCategoriesStore();
    await vi.waitFor(() => expect(listMock).toHaveBeenCalledTimes(1));
    trigger?.('accounts');
    await new Promise((r) => setTimeout(r, 20));
    expect(listMock).toHaveBeenCalledTimes(1);
  });
});
```

- [ ] **Step 3: Run vitest**

```bash
pnpm test
```

Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add src/lib/stores
git commit -m "feat(store): categories store with data:changed subscription"
```

---

### Task 8: Shared UI primitives — Card, Button, TextField, Select, Modal, CategoryBadge, EmptyState

**Files:**
- Create: `src/lib/components/Card.svelte`
- Create: `src/lib/components/Button.svelte`
- Create: `src/lib/components/TextField.svelte`
- Create: `src/lib/components/Select.svelte`
- Create: `src/lib/components/Modal.svelte`
- Create: `src/lib/components/CategoryBadge.svelte`
- Create: `src/lib/components/EmptyState.svelte`

- [ ] **Step 1: `Card.svelte`**

```svelte
<script lang="ts">
  let { children, padded = true }: { children?: any; padded?: boolean } = $props();
</script>
<section class="card" class:padded>
  {@render children?.()}
</section>
<style>
  .card {
    background: var(--card-bg);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-md);
  }
  .card.padded { padding: var(--space-5); }
</style>
```

- [ ] **Step 2: `Button.svelte`**

```svelte
<script lang="ts">
  type Variant = 'primary' | 'ghost' | 'danger';
  let {
    children,
    onclick,
    type = 'button',
    variant = 'primary',
    disabled = false,
  }: {
    children?: any;
    onclick?: (e: MouseEvent) => void;
    type?: 'button' | 'submit';
    variant?: Variant;
    disabled?: boolean;
  } = $props();
</script>
<button {type} {disabled} class={variant} {onclick}>
  {@render children?.()}
</button>
<style>
  button {
    border: 0;
    border-radius: var(--radius-md);
    padding: var(--space-3) var(--space-4);
    font-weight: 600;
    cursor: pointer;
    transition: filter 0.15s ease;
  }
  button:disabled { opacity: 0.5; cursor: not-allowed; }
  button.primary {
    color: white;
    background: linear-gradient(135deg, var(--accent-grad-start), var(--accent-grad-end));
  }
  button.primary:hover:not(:disabled) { filter: brightness(1.05); }
  button.ghost {
    background: transparent;
    color: var(--text);
    border: 1px solid var(--border);
  }
  button.ghost:hover:not(:disabled) { background: rgba(0,0,0,0.04); }
  button.danger {
    background: var(--danger);
    color: white;
  }
</style>
```

- [ ] **Step 3: `TextField.svelte`**

```svelte
<script lang="ts">
  let {
    label,
    value = $bindable(''),
    type = 'text',
    placeholder = '',
    required = false,
    error = null,
    testid,
  }: {
    label: string;
    value?: string;
    type?: 'text' | 'number' | 'date';
    placeholder?: string;
    required?: boolean;
    error?: string | null;
    testid?: string;
  } = $props();
</script>
<label class="field">
  <span>{label}{#if required}<em>*</em>{/if}</span>
  <input
    {type}
    {placeholder}
    bind:value
    {required}
    data-testid={testid}
    aria-invalid={error ? 'true' : undefined}
  />
  {#if error}<small class="error">{error}</small>{/if}
</label>
<style>
  .field { display: grid; gap: var(--space-2); }
  .field span { font-size: 0.85rem; color: var(--muted); }
  .field em { color: var(--danger); margin-left: var(--space-1); font-style: normal; }
  input {
    padding: var(--space-3);
    border-radius: var(--radius-sm);
    border: 1px solid var(--border);
    background: var(--surface);
  }
  input[aria-invalid='true'] { border-color: var(--danger); }
  .error { color: var(--danger); font-size: 0.8rem; }
</style>
```

- [ ] **Step 4: `Select.svelte`**

```svelte
<script lang="ts">
  type Option = { value: string; label: string };
  let {
    label,
    value = $bindable(''),
    options,
    required = false,
    testid,
  }: {
    label: string;
    value?: string;
    options: Option[];
    required?: boolean;
    testid?: string;
  } = $props();
</script>
<label class="field">
  <span>{label}{#if required}<em>*</em>{/if}</span>
  <select bind:value {required} data-testid={testid}>
    {#each options as opt}
      <option value={opt.value}>{opt.label}</option>
    {/each}
  </select>
</label>
<style>
  .field { display: grid; gap: var(--space-2); }
  .field span { font-size: 0.85rem; color: var(--muted); }
  .field em { color: var(--danger); margin-left: var(--space-1); font-style: normal; }
  select {
    padding: var(--space-3);
    border-radius: var(--radius-sm);
    border: 1px solid var(--border);
    background: var(--surface);
  }
</style>
```

- [ ] **Step 5: `Modal.svelte`**

```svelte
<script lang="ts">
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
    children?: any;
    footer?: any;
  } = $props();

  function handleBackdrop(e: MouseEvent) {
    if (e.target === e.currentTarget) onclose?.();
  }
</script>

{#if open}
  <div
    class="backdrop"
    role="presentation"
    onclick={handleBackdrop}
    onkeydown={(e) => e.key === 'Escape' && onclose?.()}
  >
    <div class="dialog" role="dialog" aria-modal="true" aria-label={title}>
      <header><h2>{title}</h2></header>
      <div class="body">{@render children?.()}</div>
      {#if footer}<footer>{@render footer()}</footer>{/if}
    </div>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed; inset: 0;
    background: rgba(0,0,0,0.4);
    display: grid; place-items: center;
    z-index: 100;
  }
  .dialog {
    background: var(--surface);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-lg);
    min-width: 360px;
    max-width: 560px;
  }
  header { padding: var(--space-5); border-bottom: 1px solid var(--border); }
  h2 { margin: 0; font-size: 1.2rem; }
  .body { padding: var(--space-5); display: grid; gap: var(--space-4); }
  footer { padding: var(--space-4) var(--space-5); border-top: 1px solid var(--border); display: flex; justify-content: flex-end; gap: var(--space-3); }
</style>
```

- [ ] **Step 6: `CategoryBadge.svelte`**

```svelte
<script lang="ts">
  import type { Category } from '../api/categories';
  let { category }: { category: Category } = $props();
  const fallback = category.type === 'income' ? '#4FACFE' : '#FF7A85';
  const bg = category.color ?? fallback;
</script>
<span class="badge" style="background:{bg}">
  {#if category.icon}<span class="icon" aria-hidden="true">{category.icon}</span>{/if}
  <span>{category.name}</span>
</span>
<style>
  .badge {
    display: inline-flex;
    align-items: center;
    gap: var(--space-2);
    padding: 2px 10px;
    border-radius: 999px;
    color: white;
    font-size: 0.85rem;
    font-weight: 600;
  }
</style>
```

- [ ] **Step 7: `EmptyState.svelte`**

```svelte
<script lang="ts">
  let { title, hint }: { title: string; hint?: string } = $props();
</script>
<div class="empty">
  <p class="title">{title}</p>
  {#if hint}<p class="hint">{hint}</p>{/if}
</div>
<style>
  .empty { text-align: center; padding: var(--space-6); color: var(--muted); }
  .title { font-weight: 600; margin: 0 0 var(--space-2); color: var(--text); }
  .hint { margin: 0; }
</style>
```

- [ ] **Step 8: svelte-check**

```bash
pnpm check
```

Expected: PASS.

- [ ] **Step 9: Commit**

```bash
git add src/lib/components
git commit -m "feat(ui): Card/Button/TextField/Select/Modal/CategoryBadge/EmptyState primitives"
```

---

### Task 9: `routes/Categories.svelte` — list, create, edit, archive

**Files:**
- Modify: `src/routes/Categories.svelte`

- [ ] **Step 1: Replace placeholder with the real page**

```svelte
<script lang="ts">
  import Card from '../lib/components/Card.svelte';
  import Button from '../lib/components/Button.svelte';
  import Modal from '../lib/components/Modal.svelte';
  import TextField from '../lib/components/TextField.svelte';
  import Select from '../lib/components/Select.svelte';
  import CategoryBadge from '../lib/components/CategoryBadge.svelte';
  import EmptyState from '../lib/components/EmptyState.svelte';
  import { createCategoriesStore } from '../lib/stores/categories.svelte';
  import {
    createCategory,
    updateCategory,
    archiveCategory,
    unarchiveCategory,
    type Category,
    type CategoryType,
  } from '../lib/api/categories';

  const store = createCategoriesStore({ include_archived: true });

  let modalOpen = $state(false);
  let editing = $state<Category | null>(null);
  let name = $state('');
  let type = $state<CategoryType>('expense');
  let color = $state('');
  let icon = $state('');
  let formError = $state<string | null>(null);

  function openCreate() {
    editing = null;
    name = '';
    type = 'expense';
    color = '';
    icon = '';
    formError = null;
    modalOpen = true;
  }

  function openEdit(cat: Category) {
    editing = cat;
    name = cat.name;
    type = cat.type;
    color = cat.color ?? '';
    icon = cat.icon ?? '';
    formError = null;
    modalOpen = true;
  }

  async function submit() {
    formError = null;
    try {
      if (editing) {
        await updateCategory(editing.id, {
          name,
          color: color.length > 0 ? color : null,
          icon: icon.length > 0 ? icon : null,
        });
      } else {
        await createCategory({
          name,
          type,
          color: color.length > 0 ? color : undefined,
          icon: icon.length > 0 ? icon : undefined,
        });
      }
      modalOpen = false;
    } catch (e) {
      formError = e instanceof Error ? e.message : String(e);
    }
  }

  async function toggleArchive(cat: Category) {
    if (cat.archived_at) await unarchiveCategory(cat.id);
    else await archiveCategory(cat.id);
  }

  const visible = $derived(store.items.filter((c) => !c.archived_at));
  const archived = $derived(store.items.filter((c) => !!c.archived_at));
</script>

<section>
  <header class="page-header">
    <h1>カテゴリ</h1>
    <Button onclick={openCreate}>
      {#snippet children()}+ 追加{/snippet}
    </Button>
  </header>

  <Card>
    {#snippet children()}
      {#if store.loading && store.items.length === 0}
        <p>読み込み中…</p>
      {:else if visible.length === 0}
        <EmptyState title="カテゴリがありません" hint="右上の「+ 追加」から作成してください" />
      {:else}
        <ul class="list" data-testid="categories-list">
          {#each visible as cat (cat.id)}
            <li>
              <CategoryBadge category={cat} />
              <small class="type">{cat.type === 'income' ? '収入' : '支出'}</small>
              <span class="spacer"></span>
              <Button variant="ghost" onclick={() => openEdit(cat)}>
                {#snippet children()}編集{/snippet}
              </Button>
              <Button variant="ghost" onclick={() => toggleArchive(cat)}>
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
          {#each archived as cat (cat.id)}
            <li>
              <CategoryBadge category={cat} />
              <small class="type">{cat.type === 'income' ? '収入' : '支出'}</small>
              <span class="spacer"></span>
              <Button variant="ghost" onclick={() => toggleArchive(cat)}>
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
  title={editing ? 'カテゴリを編集' : 'カテゴリを追加'}
  onclose={() => (modalOpen = false)}
>
  {#snippet children()}
    <TextField label="名前" required bind:value={name} testid="category-name" />
    {#if !editing}
      <Select
        label="種別"
        required
        bind:value={type}
        options={[
          { value: 'expense', label: '支出' },
          { value: 'income', label: '収入' },
        ]}
        testid="category-type"
      />
    {/if}
    <TextField label="色 (#RRGGBB)" bind:value={color} placeholder="#FF00AA" />
    <TextField label="アイコン (絵文字)" bind:value={icon} placeholder="🍱" />
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
  .page-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: var(--space-5);
  }
  h1 { color: white; margin: 0; }
  h2.sub { color: white; margin: var(--space-6) 0 var(--space-4); }
  .list { list-style: none; padding: 0; margin: 0; display: grid; gap: var(--space-3); }
  .list li {
    display: flex; align-items: center; gap: var(--space-4);
    padding: var(--space-3);
    border-radius: var(--radius-md);
    background: rgba(0,0,0,0.02);
  }
  .type { color: var(--muted); }
  .spacer { flex: 1; }
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

- Navigate to `カテゴリ`.
- Verify the seeded defaults (15 items: 11 expense + 4 income) render.
- Add a new category, edit it, archive it, restore it.
- Watch the list update without reloading (events are wired).

Stop the dev server.

- [ ] **Step 4: Commit**

```bash
git add src/routes/Categories.svelte
git commit -m "feat(ui): categories page with list, add, edit, archive flow"
```

---

### Categories slice DoD

- [ ] `cargo clippy --all-targets -- -D warnings` green.
- [ ] `cargo test` green (domain + integration + seed tests).
- [ ] `pnpm test` green (store test).
- [ ] `pnpm check` green.
- [ ] Manual: seeded categories visible on first launch; CRUD works; archive/restore round-trips.
