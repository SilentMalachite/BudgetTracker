pub mod commands;
pub mod domain;
pub mod error;
pub mod infra;

use std::fs;
use std::sync::Mutex;

use tauri::Manager;

use crate::commands::meta::{app_info, AppState};
use crate::infra::{db, keychain, migrations};

const KEYCHAIN_SERVICE: &str = "jp.budget-tracker";
const KEYCHAIN_ACCOUNT: &str = "db_key";
const DB_FILENAME: &str = "data.db";

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let data_dir = app
                .path()
                .app_data_dir()
                .expect("failed to resolve app data dir");
            fs::create_dir_all(&data_dir).expect("failed to create app data dir");

            let key = keychain::get_or_create_key(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT)
                .expect("failed to acquire DB key from OS keychain");

            let db_path = data_dir.join(DB_FILENAME);
            let mut conn =
                db::open_encrypted(&db_path, &key).expect("failed to open encrypted database");
            let _version = migrations::run(&mut conn).expect("failed to apply migrations");
            crate::domain::seed::seed_default_categories_if_needed(&mut conn)
                .expect("failed to seed default categories");

            app.manage(AppState {
                conn: Mutex::new(conn),
                db_path,
            });
            Ok(())
        })
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
            commands::transactions::list_transactions,
            commands::transactions::create_transaction,
            commands::transactions::update_transaction,
            commands::transactions::delete_transaction,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
