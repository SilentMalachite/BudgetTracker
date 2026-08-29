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
    assert!(snapshot.contains("\"schema_version\": 1"));
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
    assert_eq!(result.budgets, 1);

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
    let budget_amount: i64 = conn
        .query_row("SELECT amount FROM budgets", [], |row| row.get(0))
        .unwrap();
    assert_eq!(budget_amount, 50_000);
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
fn append_import_reports_inserted_budget_count() {
    let mut target = seeded_db();
    let source = db_with_names("カード", "日用品", "洗剤");
    let category_id: i64 = source
        .query_row(
            "SELECT id FROM categories WHERE name = '日用品'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    source
        .execute(
            "INSERT INTO budgets(category_id, period, amount, starts_on, alert_threshold)
             VALUES(?1, 'monthly', 25000, '2026-05-01', 75)",
            params![category_id],
        )
        .unwrap();
    let snapshot = backup::export_snapshot_json(&source).unwrap();

    let result = backup::import_snapshot_json(&mut target, &snapshot, "append").unwrap();

    assert_eq!(result.budgets, 1);
    let amount: i64 = target
        .query_row(
            "SELECT b.amount
               FROM budgets b
               JOIN categories c ON c.id = b.category_id
              WHERE c.name = '日用品'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(amount, 25_000);
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
    assert!(err
        .to_string()
        .contains("unsupported backup format version 999 (expected 1)"));
}

#[test]
fn import_rejects_backup_format_version_matching_db_schema() {
    let mut conn = seeded_db();
    let payload = r#"{
      "schema_version": 3,
      "exported_at": "2026-05-25T00:00:00Z",
      "categories": [],
      "accounts": [],
      "recurring_rules": [],
      "transactions": [],
      "budgets": [],
      "app_meta": []
    }"#;

    let err = backup::import_snapshot_json(&mut conn, payload, "overwrite").unwrap_err();
    let message = err.to_string();
    assert!(
        message.contains(
            "unsupported backup format version 3 (expected 1); this is not app_meta.schema_version"
        ),
        "{message}"
    );
}

#[test]
fn import_rejects_same_account_transfer_before_commit() {
    let mut conn = seeded_db();
    let accounts_before = count(&conn, "accounts");
    let transactions_before = count(&conn, "transactions");
    let payload = r#"{
      "schema_version": 1,
      "exported_at": "2026-05-25T00:00:00Z",
      "categories": [],
      "accounts": [
        {
          "id": 1,
          "name": "現金",
          "kind": "cash",
          "currency": "JPY",
          "initial_balance": 1000,
          "display_order": 0,
          "note": "",
          "archived_at": null,
          "created_at": "2026-05-25T00:00:00Z",
          "updated_at": "2026-05-25T00:00:00Z"
        }
      ],
      "recurring_rules": [],
      "transactions": [
        {
          "id": 1,
          "occurred_on": "2026-05-25",
          "type": "transfer",
          "amount": 100,
          "account_id": 1,
          "counter_account_id": 1,
          "category_id": null,
          "description": "self",
          "recurring_id": null,
          "created_at": "2026-05-25T00:00:00Z",
          "updated_at": "2026-05-25T00:00:00Z"
        }
      ],
      "budgets": [],
      "app_meta": []
    }"#;

    let err = backup::import_snapshot_json(&mut conn, payload, "append").unwrap_err();
    let message = err.to_string();
    assert!(message.contains("transfer["), "{message}");
    assert!(message.contains("source and destination"), "{message}");
    assert_eq!(count(&conn, "accounts"), accounts_before);
    assert_eq!(count(&conn, "transactions"), transactions_before);
}

#[test]
fn import_rejects_empty_account_name() {
    let mut conn = seeded_db();
    let accounts_before = count(&conn, "accounts");
    let payload = r#"{
      "schema_version": 1,
      "exported_at": "2026-05-25T00:00:00Z",
      "categories": [],
      "accounts": [
        {
          "id": 1,
          "name": "",
          "kind": "cash",
          "currency": "JPY",
          "initial_balance": 0,
          "display_order": 0,
          "note": "",
          "archived_at": null,
          "created_at": "2026-05-25T00:00:00Z",
          "updated_at": "2026-05-25T00:00:00Z"
        }
      ],
      "recurring_rules": [],
      "transactions": [],
      "budgets": [],
      "app_meta": []
    }"#;

    let err = backup::import_snapshot_json(&mut conn, payload, "append").unwrap_err();
    let message = err.to_string();
    assert!(message.contains("account["), "{message}");
    assert!(message.contains("empty"), "{message}");
    assert_eq!(count(&conn, "accounts"), accounts_before);
}

#[test]
fn export_then_overwrite_import_preserves_transfer_row() {
    use budget_tracker_lib::commands::backup::{export_snapshot_json, import_snapshot_json};
    use budget_tracker_lib::infra::repo::balance_repo;

    // --- Source DB: two accounts + one transfer ---
    let mut src = rusqlite::Connection::open_in_memory().unwrap();
    migrations::run(&mut src).unwrap();
    let cash = account_repo::insert(
        &src,
        &account_repo::InsertInput {
            name: "cash",
            kind: AccountKind::Cash,
            currency: "JPY",
            initial_balance: 50_000,
            display_order: 0,
            note: "",
            now: NOW,
        },
    )
    .unwrap();
    let bank = account_repo::insert(
        &src,
        &account_repo::InsertInput {
            name: "bank",
            kind: AccountKind::Bank,
            currency: "JPY",
            initial_balance: 200_000,
            display_order: 1,
            note: "",
            now: NOW,
        },
    )
    .unwrap();
    let transfer_id = transaction_repo::insert_transfer(
        &src,
        &transaction_repo::InsertTransferInput {
            occurred_on: "2026-05-25",
            amount: 30_000,
            account_id: bank,
            counter_account_id: cash,
            description: "ATM",
            now: NOW,
        },
    )
    .unwrap();

    let src_balances = balance_repo::list_balances(&src).unwrap();
    let src_total: i64 = src_balances.iter().map(|b| b.balance).sum();

    // --- Snapshot to JSON ---
    let snapshot = export_snapshot_json(&src).unwrap();

    // --- Destination DB: empty, imported in 'overwrite' mode ---
    let mut dst = rusqlite::Connection::open_in_memory().unwrap();
    migrations::run(&mut dst).unwrap();
    let _import_result = import_snapshot_json(&mut dst, &snapshot, "overwrite").unwrap();

    // 1) Transfer row is back with the same shape.
    let restored = transaction_repo::find_by_id(&dst, transfer_id).unwrap();
    assert!(matches!(restored.type_, TxType::Transfer));
    assert_eq!(restored.amount, 30_000);
    assert_eq!(restored.account_id, bank);
    assert_eq!(restored.counter_account_id, Some(cash));
    assert!(restored.category_id.is_none());

    // 2) Aggregate balance survives.
    let dst_balances = balance_repo::list_balances(&dst).unwrap();
    let dst_total: i64 = dst_balances.iter().map(|b| b.balance).sum();
    assert_eq!(src_total, dst_total);
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
