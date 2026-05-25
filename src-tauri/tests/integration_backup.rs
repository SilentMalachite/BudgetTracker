use budget_tracker_lib::commands::backup;
use budget_tracker_lib::domain::account::AccountKind;
use budget_tracker_lib::domain::category::CategoryType;
use budget_tracker_lib::domain::ledger::TxType;
use budget_tracker_lib::infra::migrations;
use budget_tracker_lib::infra::repo::{account_repo, category_repo, transaction_repo};
use rusqlite::Connection;

const NOW: &str = "2026-05-25T00:00:00+00:00";

fn seeded_db() -> Connection {
    let mut conn = Connection::open_in_memory().unwrap();
    migrations::run(&mut conn).unwrap();

    let account_id = account_repo::insert(
        &conn,
        &account_repo::InsertInput {
            name: "現金",
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
            name: "食費",
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
            description: "ランチ",
            now: NOW,
        },
    )
    .unwrap();

    conn
}

#[test]
fn export_then_overwrite_import_restores_state() {
    let mut conn = seeded_db();
    let snapshot = backup::export_snapshot_json(&conn).unwrap();
    assert!(snapshot.contains("食費"));
    assert!(snapshot.contains("ランチ"));

    conn.execute("DELETE FROM transactions", []).unwrap();
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
}

#[test]
fn append_import_skips_rows_with_missing_fk() {
    let mut conn = seeded_db();
    let bogus = r##"{
      "schema_version": 1,
      "exported_at": "2026-05-25T00:00:00Z",
      "categories": [],
      "accounts": [],
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
      "transactions": [],
      "app_meta": []
    }"#;

    let err = backup::import_snapshot_json(&mut conn, payload, "overwrite").unwrap_err();
    assert!(err.to_string().contains("unsupported schema_version"));
}
