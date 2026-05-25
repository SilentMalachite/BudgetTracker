use budget_tracker_lib::commands::backup;
use budget_tracker_lib::domain::account::AccountKind;
use budget_tracker_lib::domain::category::CategoryType;
use budget_tracker_lib::domain::ledger::TxType;
use budget_tracker_lib::infra::migrations;
use budget_tracker_lib::infra::repo::{account_repo, category_repo, transaction_repo};
use rusqlite::{params, Connection};

const NOW: &str = "2026-05-25T00:00:00+00:00";

fn db_with_names(account_name: &str, category_name: &str, description: &str) -> Connection {
    let mut conn = Connection::open_in_memory().unwrap();
    migrations::run(&mut conn).unwrap();

    let account_id = account_repo::insert(
        &conn,
        &account_repo::InsertInput {
            name: account_name,
            kind: AccountKind::Cash,
            currency: "JPY",
            initial_balance: 1_000,
            display_order: 0,
            note: "",
            now: NOW,
        },
    )
    .unwrap();
    let category_id = category_repo::insert(
        &conn,
        &category_repo::InsertInput {
            name: category_name,
            type_: CategoryType::Expense,
            color: Some("#FF0000"),
            icon: None,
            display_order: 0,
        },
    )
    .unwrap();
    transaction_repo::insert(
        &conn,
        &transaction_repo::InsertInput {
            occurred_on: "2026-05-15",
            type_: TxType::Expense,
            amount: 500,
            account_id,
            category_id,
            description,
            now: NOW,
        },
    )
    .unwrap();

    conn
}

fn seeded_db() -> Connection {
    let conn = db_with_names("現金", "食費", "ランチ");
    let account_id: i64 = conn
        .query_row("SELECT id FROM accounts WHERE name = '現金'", [], |row| {
            row.get(0)
        })
        .unwrap();
    let category_id: i64 = conn
        .query_row(
            "SELECT id FROM categories WHERE name = '食費'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    conn.execute(
        "INSERT INTO recurring_rules(name, type, amount, account_id, counter_account_id,
                                     category_id, description, frequency, day_of_month,
                                     starts_on, active)
         VALUES('家賃', 'expense', 80000, ?1, NULL, ?2, '', 'monthly', 25, '2026-05-01', 1)",
        params![account_id, category_id],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO budgets(category_id, period, amount, starts_on, alert_threshold)
         VALUES(?1, 'monthly', 50000, '2026-05-01', 80)",
        params![category_id],
    )
    .unwrap();
    conn
}

fn count(conn: &Connection, table: &str) -> i64 {
    conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
        row.get(0)
    })
    .unwrap()
}

#[test]
fn export_then_overwrite_import_restores_state() {
    let mut conn = seeded_db();
    let snapshot = backup::export_snapshot_json(&conn).unwrap();
    assert!(snapshot.contains("食費"));
    assert!(snapshot.contains("ランチ"));
    assert!(snapshot.contains("recurring_rules"));
    assert!(snapshot.contains("budgets"));

    conn.execute("DELETE FROM transactions", []).unwrap();
    conn.execute("DELETE FROM budgets", []).unwrap();
    conn.execute("DELETE FROM recurring_rules", []).unwrap();
    conn.execute("DELETE FROM categories", []).unwrap();
    conn.execute("DELETE FROM accounts", []).unwrap();

    let result = backup::import_snapshot_json(&mut conn, &snapshot, "overwrite").unwrap();
    assert!(result.warnings.is_empty());
    assert_eq!(result.categories, 1);
    assert_eq!(result.accounts, 1);
    assert_eq!(result.transactions, 1);

    let categories = category_repo::list(&conn, &category_repo::ListFilter::default()).unwrap();
    let accounts = account_repo::list(&conn, false).unwrap();
    let (transactions, total) =
        transaction_repo::list(&conn, &transaction_repo::ListFilter::default(), 0, 50).unwrap();

    assert_eq!(categories.len(), 1);
    assert_eq!(accounts.len(), 1);
    assert_eq!(total, 1);
    assert_eq!(transactions[0].description, "ランチ");
    assert_eq!(count(&conn, "recurring_rules"), 1);
    assert_eq!(count(&conn, "budgets"), 1);
}

#[test]
fn append_import_remaps_new_account_and_category_ids() {
    let mut target = seeded_db();
    let source = db_with_names("銀行", "交通", "電車");
    let snapshot = backup::export_snapshot_json(&source).unwrap();

    let result = backup::import_snapshot_json(&mut target, &snapshot, "append").unwrap();
    assert_eq!(result.accounts, 1);
    assert_eq!(result.categories, 1);
    assert_eq!(result.transactions, 1);

    let (account_name, category_name): (String, String) = target
        .query_row(
            "SELECT a.name, c.name
               FROM transactions t
               JOIN accounts a ON a.id = t.account_id
               JOIN categories c ON c.id = t.category_id
              WHERE t.description = '電車'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(account_name, "銀行");
    assert_eq!(category_name, "交通");
}

#[test]
fn append_import_skips_rows_with_missing_fk() {
    let mut conn = seeded_db();
    let bogus = r##"{
      "schema_version": 1,
      "exported_at": "2026-05-25T00:00:00Z",
      "categories": [],
      "accounts": [],
      "recurring_rules": [],
      "transactions": [
        {
          "occurred_on": "2026-05-01",
          "type": "expense",
          "amount": 100,
          "account_id": 999,
          "counter_account_id": null,
          "category_id": 999,
          "description": "ghost",
          "recurring_id": null,
          "created_at": "2026-05-01T00:00:00Z",
          "updated_at": "2026-05-01T00:00:00Z"
        }
      ],
      "budgets": [],
      "app_meta": []
    }"##;

    let result = backup::import_snapshot_json(&mut conn, bogus, "append").unwrap();
    assert_eq!(result.transactions, 0);
    assert!(!result.warnings.is_empty());
}

#[test]
fn import_rejects_wrong_schema_version() {
    let mut conn = seeded_db();
    let payload = r#"{
      "schema_version": 999,
      "exported_at": "2026-05-25T00:00:00Z",
      "categories": [],
      "accounts": [],
      "recurring_rules": [],
      "transactions": [],
      "budgets": [],
      "app_meta": []
    }"#;

    let err = backup::import_snapshot_json(&mut conn, payload, "overwrite").unwrap_err();
    assert!(err.to_string().contains("unsupported schema_version"));
}

#[test]
fn overwrite_import_does_not_restore_last_backup_at() {
    let source = seeded_db();
    source
        .execute(
            "INSERT INTO app_meta(key, value) VALUES('last_backup_at', '2000-01-01T00:00:00Z')
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [],
        )
        .unwrap();
    let snapshot = backup::export_snapshot_json(&source).unwrap();

    let mut target = seeded_db();
    target
        .execute(
            "INSERT INTO app_meta(key, value) VALUES('last_backup_at', '2026-05-25T00:00:00Z')
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [],
        )
        .unwrap();

    backup::import_snapshot_json(&mut target, &snapshot, "overwrite").unwrap();

    let last_backup_at: String = target
        .query_row(
            "SELECT value FROM app_meta WHERE key = 'last_backup_at'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(last_backup_at, "2026-05-25T00:00:00Z");
}
