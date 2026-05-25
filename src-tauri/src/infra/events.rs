use serde::Serialize;
use tauri::{AppHandle, Emitter};

/// Domains that can be broadcast on the `data:changed` channel.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangedDomain {
    Categories,
    Accounts,
    Transactions,
    Budgets,
    Meta,
}

#[derive(Debug, Clone, Serialize)]
struct ChangedEvent {
    domain: ChangedDomain,
}

/// Emit `data:changed { domain }` to all webview windows. Failures are logged
/// to stderr but never propagated because UI re-fetch is best-effort.
pub fn emit_changed(app: &AppHandle, domain: ChangedDomain) {
    if let Err(e) = app.emit("data:changed", ChangedEvent { domain }) {
        eprintln!("[events] failed to emit data:changed: {e}");
    }
}
