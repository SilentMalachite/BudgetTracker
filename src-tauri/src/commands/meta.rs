use std::path::PathBuf;
use std::sync::Mutex;

use rusqlite::Connection;
use serde::Serialize;
use tauri::State;

use crate::error::{AppError, AppResult};

pub struct AppState {
    pub conn: Mutex<Connection>,
    pub db_path: PathBuf,
}

#[derive(Debug, Serialize)]
pub struct AppInfo {
    pub schema_version: u32,
    pub db_path: String,
}

#[tauri::command]
pub fn app_info(state: State<'_, AppState>) -> AppResult<AppInfo> {
    let conn = state.conn.lock().map_err(|_| {
        AppError::Corrupt("connection mutex poisoned".into())
    })?;
    let version: String = conn
        .query_row(
            "SELECT value FROM app_meta WHERE key = 'schema_version'",
            [],
            |r| r.get(0),
        )
        .unwrap_or_else(|_| "0".to_string());
    Ok(AppInfo {
        schema_version: version.parse().unwrap_or(0),
        db_path: state.db_path.to_string_lossy().to_string(),
    })
}
