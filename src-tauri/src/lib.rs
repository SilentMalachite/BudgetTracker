pub mod commands;
pub mod domain;
pub mod error;
pub mod infra;

use std::sync::Mutex;

use tauri::Manager;

use crate::commands::meta::{app_info, AppInner, AppState};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
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
                crate::infra::boot::BootOutcome::Recovery {
                    reason,
                    data_dir,
                    db_path,
                } => AppInner::Recovery {
                    reason,
                    data_dir,
                    db_path,
                },
            };
            app.manage(AppState {
                inner: Mutex::new(inner),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_info,
            commands::recovery::boot_status,
            commands::recovery::recover_import_json,
            commands::recovery::recover_start_empty_cmd,
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
            commands::transactions::list_transactions,
            commands::transactions::create_transaction,
            commands::transactions::update_transaction,
            commands::transactions::delete_transaction,
            commands::transactions::create_transfer,
            commands::transactions::update_transfer,
            commands::reports::monthly_summary,
            commands::reports::monthly_series,
            commands::reports::report_monthly,
            commands::reports::report_yearly,
            commands::reports::report_by_category,
            commands::reports::report_net_worth_series,
            commands::budgets::list_budget_statuses,
            commands::budgets::list_top_budget_statuses,
            commands::budgets::set_budget,
            commands::recurring::list_recurring_rules,
            commands::recurring::create_recurring_rule,
            commands::recurring::update_recurring_rule,
            commands::recurring::set_recurring_rule_active,
            commands::recurring::preview_recurring_occurrences,
            commands::recurring::expand_due_recurring,
            commands::backup::export_json,
            commands::backup::import_json,
            commands::settings::get_last_backup_at,
            commands::settings::export_backup_to_file,
            commands::settings::get_db_path,
            commands::snapshots::list_pre_import_snapshots,
            commands::snapshots::restore_pre_import_snapshot,
            commands::balances::list_balances,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
