use budget_tracker_lib::commands::backup;
use budget_tracker_lib::commands::recurring as recurring_cmd;
use budget_tracker_lib::domain::account::AccountKind;
use budget_tracker_lib::domain::category::CategoryType;
use budget_tracker_lib::domain::ledger::TxType;
use budget_tracker_lib::error::AppError;
use budget_tracker_lib::infra::migrations;
use budget_tracker_lib::infra::repo::{account_repo, category_repo, transaction_repo};
use chrono::NaiveDate;
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
    assert_eq!(result.recurring_rules, 1);
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

// ---------------------------------------------------------------------------
// Import validation: every row that the create-path commands would reject must
// surface as an `ImportValidation` row error (entity[index]: message) before
// anything touches the database.
// ---------------------------------------------------------------------------

const CASH_ACCOUNT_JSON: &str = r#"{
  "id": 1, "name": "現金", "kind": "cash", "currency": "JPY", "initial_balance": 1000,
  "display_order": 0, "note": "", "archived_at": null,
  "created_at": "2026-05-25T00:00:00Z", "updated_at": "2026-05-25T00:00:00Z"
}"#;

fn category_json(type_: &str, color: &str) -> String {
    format!(
        r#"{{ "id": 1, "name": "snapshot cat", "type": "{type_}", "color": {color},
              "icon": null, "display_order": 0, "archived_at": null }}"#
    )
}

fn expense_json(occurred_on: &str, type_: &str, category_id: &str) -> String {
    format!(
        r#"{{ "id": 1, "occurred_on": "{occurred_on}", "type": "{type_}", "amount": 100,
              "account_id": 1, "counter_account_id": null, "category_id": {category_id},
              "description": "", "recurring_id": null,
              "created_at": "2026-05-25T00:00:00Z", "updated_at": "2026-05-25T00:00:00Z" }}"#
    )
}

fn budget_json(period: &str, starts_on: &str) -> String {
    format!(
        r#"{{ "id": 1, "category_id": 1, "period": "{period}", "amount": 1000,
              "starts_on": "{starts_on}", "ends_on": null, "alert_threshold": 80 }}"#
    )
}

fn recurring_rule_json(
    frequency: &str,
    day_of_month: &str,
    day_of_week: &str,
    starts_on: &str,
    ends_on: &str,
) -> String {
    format!(
        r#"{{ "id": 1, "name": "家賃", "type": "expense", "amount": 80000, "account_id": 1,
              "counter_account_id": null, "category_id": 1, "description": "",
              "frequency": "{frequency}", "day_of_month": {day_of_month},
              "day_of_week": {day_of_week}, "starts_on": "{starts_on}", "ends_on": {ends_on},
              "last_generated_on": null, "active": 1 }}"#
    )
}

fn payload(
    categories: &str,
    accounts: &str,
    recurring_rules: &str,
    transactions: &str,
    budgets: &str,
) -> String {
    format!(
        r#"{{
      "schema_version": 1,
      "exported_at": "2026-05-25T00:00:00Z",
      "categories": [{categories}],
      "accounts": [{accounts}],
      "recurring_rules": [{recurring_rules}],
      "transactions": [{transactions}],
      "budgets": [{budgets}],
      "app_meta": []
    }}"#
    )
}

fn table_counts(conn: &Connection) -> [i64; 5] {
    [
        count(conn, "accounts"),
        count(conn, "categories"),
        count(conn, "recurring_rules"),
        count(conn, "transactions"),
        count(conn, "budgets"),
    ]
}

/// Import `payload` in append mode and assert it is rejected as an
/// `ImportValidation` error naming `entity_index` (e.g. `budget[0]`) with a
/// message containing `needle`, and that no table changed.
fn assert_import_rejected(payload: &str, entity_index: &str, needle: &str) {
    let mut conn = seeded_db();
    let before = table_counts(&conn);

    let err = backup::import_snapshot_json(&mut conn, payload, "append").unwrap_err();
    assert!(
        matches!(err, AppError::ImportValidation(_)),
        "expected ImportValidation, got {err:?}"
    );
    let message = err.to_string();
    assert!(message.contains(entity_index), "{message}");
    assert!(message.contains(needle), "{message}");
    assert_eq!(table_counts(&conn), before, "import must not touch the DB");
}

#[test]
fn import_rejects_non_canonical_transaction_date() {
    let payload = payload(
        &category_json("expense", "null"),
        CASH_ACCOUNT_JSON,
        "",
        &expense_json("2026-5-25", "expense", "1"),
        "",
    );
    assert_import_rejected(&payload, "transaction[0]", "YYYY-MM-DD");
}

#[test]
fn import_rejects_transaction_whose_category_type_mismatches() {
    let payload = payload(
        &category_json("income", "null"),
        CASH_ACCOUNT_JSON,
        "",
        &expense_json("2026-05-25", "expense", "1"),
        "",
    );
    assert_import_rejected(&payload, "transaction[0]", "type does not match");
}

#[test]
fn import_rejects_transfer_with_category() {
    let transfer = r#"{ "id": 1, "occurred_on": "2026-05-25", "type": "transfer", "amount": 100,
          "account_id": 1, "counter_account_id": 2, "category_id": 1, "description": "",
          "recurring_id": null, "created_at": "2026-05-25T00:00:00Z",
          "updated_at": "2026-05-25T00:00:00Z" }"#;
    let bank = CASH_ACCOUNT_JSON
        .replace("\"id\": 1", "\"id\": 2")
        .replace("現金", "銀行");
    let accounts = format!("{CASH_ACCOUNT_JSON},{bank}");
    let payload = payload(
        &category_json("expense", "null"),
        &accounts,
        "",
        transfer,
        "",
    );
    assert_import_rejected(&payload, "transfer[0]", "category");
}

#[test]
fn import_rejects_budget_with_non_monthly_period() {
    let payload = payload(
        &category_json("expense", "null"),
        CASH_ACCOUNT_JSON,
        "",
        "",
        &budget_json("yearly", "2026-05-01"),
    );
    assert_import_rejected(&payload, "budget[0]", "monthly");
}

#[test]
fn import_rejects_budget_not_starting_on_first_of_month() {
    let payload = payload(
        &category_json("expense", "null"),
        CASH_ACCOUNT_JSON,
        "",
        "",
        &budget_json("monthly", "2026-05-15"),
    );
    assert_import_rejected(&payload, "budget[0]", "first day");
}

#[test]
fn import_rejects_budget_with_non_canonical_starts_on() {
    let payload = payload(
        &category_json("expense", "null"),
        CASH_ACCOUNT_JSON,
        "",
        "",
        &budget_json("monthly", "2026-5-1"),
    );
    assert_import_rejected(&payload, "budget[0]", "YYYY-MM-DD");
}

#[test]
fn import_rejects_budget_on_income_category() {
    let payload = payload(
        &category_json("income", "null"),
        CASH_ACCOUNT_JSON,
        "",
        "",
        &budget_json("monthly", "2026-05-01"),
    );
    assert_import_rejected(&payload, "budget[0]", "not an expense category");
}

#[test]
fn import_rejects_account_note_over_limit() {
    let long_note = "あ".repeat(201);
    let account =
        CASH_ACCOUNT_JSON.replace("\"note\": \"\"", &format!("\"note\": \"{long_note}\""));
    let payload = payload("", &account, "", "", "");
    assert_import_rejected(&payload, "account[0]", "note must be 200 chars or fewer");
}

#[test]
fn import_rejects_category_with_invalid_color() {
    // The frontend interpolates category.color into an inline style attribute,
    // so anything other than #RRGGBB must never reach the database.
    let payload = payload(
        &category_json("expense", "\"red;background:url(x)\""),
        "",
        "",
        "",
        "",
    );
    assert_import_rejected(&payload, "category[0]", "#RRGGBB");
}

#[test]
fn import_rejects_recurring_rule_with_invalid_frequency() {
    let payload = payload(
        &category_json("expense", "null"),
        CASH_ACCOUNT_JSON,
        &recurring_rule_json("daily", "25", "null", "2026-05-01", "null"),
        "",
        "",
    );
    assert_import_rejected(&payload, "recurring_rule[0]", "frequency");
}

#[test]
fn import_rejects_recurring_rule_with_day_of_month_out_of_range() {
    let payload = payload(
        &category_json("expense", "null"),
        CASH_ACCOUNT_JSON,
        &recurring_rule_json("monthly", "32", "null", "2026-05-01", "null"),
        "",
        "",
    );
    assert_import_rejected(&payload, "recurring_rule[0]", "day_of_month");
}

#[test]
fn import_rejects_recurring_rule_with_day_of_week_out_of_range() {
    let payload = payload(
        &category_json("expense", "null"),
        CASH_ACCOUNT_JSON,
        &recurring_rule_json("weekly", "null", "7", "2026-05-01", "null"),
        "",
        "",
    );
    assert_import_rejected(&payload, "recurring_rule[0]", "day_of_week");
}

#[test]
fn import_rejects_recurring_rule_with_non_canonical_starts_on() {
    let payload = payload(
        &category_json("expense", "null"),
        CASH_ACCOUNT_JSON,
        &recurring_rule_json("monthly", "25", "null", "2026-5-1", "null"),
        "",
        "",
    );
    assert_import_rejected(&payload, "recurring_rule[0]", "starts_on");
}

#[test]
fn import_rejects_recurring_rule_with_ends_on_before_starts_on() {
    let payload = payload(
        &category_json("expense", "null"),
        CASH_ACCOUNT_JSON,
        &recurring_rule_json("monthly", "25", "null", "2026-05-01", "\"2026-04-30\""),
        "",
        "",
    );
    assert_import_rejected(&payload, "recurring_rule[0]", "ends_on");
}

#[test]
fn import_rejects_recurring_rule_with_non_positive_amount() {
    let rule = recurring_rule_json("monthly", "25", "null", "2026-05-01", "null")
        .replace("\"amount\": 80000", "\"amount\": 0");
    let payload = payload(
        &category_json("expense", "null"),
        CASH_ACCOUNT_JSON,
        &rule,
        "",
        "",
    );
    assert_import_rejected(&payload, "recurring_rule[0]", "amount");
}

#[test]
fn overwrite_import_restores_history_on_archived_category() {
    // Rule 5: categories are archived, never deleted, precisely so that
    // historical transactions and budgets survive. A backup taken after the
    // archive must therefore round-trip, even though the create-path commands
    // refuse to attach *new* rows to an archived category.
    let source = seeded_db();
    source
        .execute(
            "UPDATE categories SET archived_at = '2026-06-01T00:00:00Z' WHERE name = '食費'",
            [],
        )
        .unwrap();
    let snapshot = backup::export_snapshot_json(&source).unwrap();

    let mut target = Connection::open_in_memory().unwrap();
    migrations::run(&mut target).unwrap();
    let result = backup::import_snapshot_json(&mut target, &snapshot, "overwrite").unwrap();

    assert!(result.warnings.is_empty(), "{:?}", result.warnings);
    assert_eq!(result.transactions, 1);
    assert_eq!(result.budgets, 1);
    assert_eq!(count(&target, "recurring_rules"), 1);
}

fn occurred_dates(conn: &Connection) -> Vec<String> {
    conn.prepare("SELECT occurred_on FROM transactions ORDER BY occurred_on")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .map(|row| row.unwrap())
        .collect()
}

/// 追記インポートで定期取引ルールが増えると、以後の展開が毎月二重に取引を書く。
/// 一度きりの重複ではなく永続的な二重課金になるので、カテゴリと同じく重複は
/// 飛ばして警告する。
#[test]
fn append_importing_a_snapshot_into_its_own_db_keeps_one_recurring_rule() {
    let mut conn = seeded_db();
    let snapshot = backup::export_snapshot_json(&conn).unwrap();

    let result = backup::import_snapshot_json(&mut conn, &snapshot, "append").unwrap();

    assert_eq!(count(&conn, "recurring_rules"), 1);
    // 何も入らなかったことがサマリにも出る。
    assert_eq!(result.recurring_rules, 0);
    assert!(
        result
            .warnings
            .iter()
            .any(|w| w == "skipped duplicate recurring rule: 家賃"),
        "{:?}",
        result.warnings
    );

    // 取り込み後の展開は各日付を 1 回だけ生成する。
    conn.execute("DELETE FROM transactions", []).unwrap();
    let expansion = recurring_cmd::expand_due_recurring_for_conn(
        &conn,
        NaiveDate::from_ymd_opt(2026, 8, 1).unwrap(),
        NOW,
    )
    .unwrap();

    assert_eq!(expansion.generated, 3);
    assert_eq!(
        occurred_dates(&conn),
        vec!["2026-05-25", "2026-06-25", "2026-07-25"]
    );
}
