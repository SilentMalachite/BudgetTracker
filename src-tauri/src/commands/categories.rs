use serde::{Deserialize, Deserializer};
use tauri::{AppHandle, State};

use crate::commands::meta::AppState;
use crate::domain::category::{self, Category, CategoryType};
use crate::error::{AppError, AppResult};
use crate::infra::events::{emit_changed, ChangedDomain};
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
    let type_ = filter
        .type_
        .as_deref()
        .map(CategoryType::parse)
        .transpose()?;
    let conn = state
        .conn
        .lock()
        .map_err(|_| AppError::Corrupt("connection mutex poisoned".into()))?;
    category_repo::list(
        &conn,
        &category_repo::ListFilter {
            type_,
            include_archived: filter.include_archived,
        },
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
    let conn = state
        .conn
        .lock()
        .map_err(|_| AppError::Corrupt("connection mutex poisoned".into()))?;
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

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum NullablePatch<T> {
    #[default]
    Missing,
    Value(Option<T>),
}

fn nullable_string_patch<'de, D>(deserializer: D) -> Result<NullablePatch<String>, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<String>::deserialize(deserializer).map(NullablePatch::Value)
}

#[derive(Debug, Deserialize)]
pub struct UpdateCategoryPatch {
    pub name: Option<String>,
    #[serde(default, deserialize_with = "nullable_string_patch")]
    pub color: NullablePatch<String>,
    #[serde(default, deserialize_with = "nullable_string_patch")]
    pub icon: NullablePatch<String>,
    pub display_order: Option<i64>,
}

#[tauri::command]
pub fn update_category(
    app: AppHandle,
    state: State<'_, AppState>,
    id: i64,
    patch: UpdateCategoryPatch,
) -> AppResult<Category> {
    let name = patch
        .name
        .as_deref()
        .map(category::validate_name)
        .transpose()?;
    let color = match patch.color {
        NullablePatch::Value(Some(c)) => NullablePatch::Value(Some(category::validate_color(&c)?)),
        NullablePatch::Value(None) => NullablePatch::Value(None),
        NullablePatch::Missing => NullablePatch::Missing,
    };
    let conn = state
        .conn
        .lock()
        .map_err(|_| AppError::Corrupt("connection mutex poisoned".into()))?;
    category_repo::update(
        &conn,
        id,
        &category_repo::UpdatePatch {
            name: name.as_deref(),
            color: match &color {
                NullablePatch::Value(value) => Some(value.as_deref()),
                NullablePatch::Missing => None,
            },
            icon: match &patch.icon {
                NullablePatch::Value(value) => Some(value.as_deref()),
                NullablePatch::Missing => None,
            },
            display_order: patch.display_order,
        },
    )?;
    let cat = category_repo::find_by_id(&conn, id)?;
    drop(conn);
    emit_changed(&app, ChangedDomain::Categories);
    Ok(cat)
}

#[tauri::command]
pub fn archive_category(app: AppHandle, state: State<'_, AppState>, id: i64) -> AppResult<()> {
    let conn = state
        .conn
        .lock()
        .map_err(|_| AppError::Corrupt("connection mutex poisoned".into()))?;
    category_repo::set_archived(&conn, id, Some(&now_iso()))?;
    drop(conn);
    emit_changed(&app, ChangedDomain::Categories);
    Ok(())
}

#[tauri::command]
pub fn unarchive_category(app: AppHandle, state: State<'_, AppState>, id: i64) -> AppResult<()> {
    let conn = state
        .conn
        .lock()
        .map_err(|_| AppError::Corrupt("connection mutex poisoned".into()))?;
    category_repo::set_archived(&conn, id, None)?;
    drop(conn);
    emit_changed(&app, ChangedDomain::Categories);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_patch_distinguishes_null_from_missing_color() {
        let missing: UpdateCategoryPatch = serde_json::from_str("{}").unwrap();
        assert_eq!(missing.color, NullablePatch::Missing);

        let cleared: UpdateCategoryPatch = serde_json::from_str(r#"{"color":null}"#).unwrap();
        assert_eq!(cleared.color, NullablePatch::Value(None));
    }
}
