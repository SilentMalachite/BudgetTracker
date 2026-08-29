use std::path::PathBuf;
use std::sync::Mutex;

use rusqlite::Connection;
use serde::Serialize;
use tauri::State;

use crate::error::{AppError, AppResult};

pub enum AppInner {
    Ready {
        conn: Connection,
        db_path: PathBuf,
    },
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
        let inner = self
            .inner
            .lock()
            .map_err(|_| AppError::Corrupt("connection mutex poisoned".into()))?;
        match &*inner {
            AppInner::Ready { db_path, .. } | AppInner::Recovery { db_path, .. } => {
                Ok(db_path.clone())
            }
        }
    }

    pub fn with_conn<R>(&self, f: impl FnOnce(&Connection) -> AppResult<R>) -> AppResult<R> {
        let inner = self
            .inner
            .lock()
            .map_err(|_| AppError::Corrupt("connection mutex poisoned".into()))?;
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
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| AppError::Corrupt("connection mutex poisoned".into()))?;
        match &mut *inner {
            AppInner::Ready { conn, .. } => f(conn),
            AppInner::Recovery { reason, .. } => Err(AppError::Unavailable(format!(
                "database is in recovery ({reason:?})"
            ))),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct AppInfo {
    pub schema_version: u32,
    pub db_path: String,
}

#[tauri::command]
pub fn app_info(state: State<'_, AppState>) -> AppResult<AppInfo> {
    let db_path = state.db_path()?.to_string_lossy().into_owned();
    state.with_conn(|conn| {
        let version: String = conn
            .query_row(
                "SELECT value FROM app_meta WHERE key = 'schema_version'",
                [],
                |r| r.get(0),
            )
            .unwrap_or_else(|_| "0".to_string());
        Ok(AppInfo {
            schema_version: version.parse().unwrap_or(0),
            db_path,
        })
    })
}
