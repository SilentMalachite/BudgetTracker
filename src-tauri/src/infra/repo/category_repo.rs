use rusqlite::{params, Connection, OptionalExtension};

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
        conn.execute(
            "UPDATE categories SET name = ?1 WHERE id = ?2",
            params![name, id],
        )
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
        conn.execute(
            "UPDATE categories SET color = ?1 WHERE id = ?2",
            params![color, id],
        )?;
    }
    if let Some(icon) = patch.icon {
        conn.execute(
            "UPDATE categories SET icon = ?1 WHERE id = ?2",
            params![icon, id],
        )?;
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
