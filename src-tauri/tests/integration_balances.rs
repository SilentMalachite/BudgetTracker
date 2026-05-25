use budget_tracker_lib::domain::account::AccountKind;
use budget_tracker_lib::domain::balance::compute_balance;
use budget_tracker_lib::domain::category::CategoryType;
use budget_tracker_lib::domain::ledger::TxType;
use budget_tracker_lib::infra::migrations;
use budget_tracker_lib::infra::repo::{
    account_repo, balance_repo, category_repo, transaction_repo,
};
use rusqlite::Connection;

const NOW: &str = "2026-05-25T00:00:00+00:00";

fn seeded_db() -> (Connection, i64, i64, i64) {
    let mut conn = Connection::open_in_memory().unwrap();
    migrations::run(&mut conn).unwrap();
    let cash = account_repo::insert(
        &conn,
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
        &conn,
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
    let food = category_repo::insert(
        &conn,
        &category_repo::InsertInput {
            name: "Food",
            type_: CategoryType::Expense,
            color: None,
            icon: None,
            display_order: 0,
        },
    )
    .unwrap();
    (conn, cash, bank, food)
}

#[test]
fn empty_db_returns_initial_balances() {
    let (conn, cash, bank, _) = seeded_db();
    let balances = balance_repo::list_balances(&conn).unwrap();
    let by_id: std::collections::HashMap<i64, i64> =
        balances.iter().map(|b| (b.account_id, b.balance)).collect();
    assert_eq!(by_id[&cash], 50_000);
    assert_eq!(by_id[&bank], 200_000);
}

#[test]
fn income_expense_and_transfer_all_flow_through() {
    let (conn, cash, bank, food) = seeded_db();
    // Expense 3,000 from cash
    transaction_repo::insert(
        &conn,
        &transaction_repo::InsertInput {
            occurred_on: "2026-05-25",
            type_: TxType::Expense,
            amount: 3_000,
            account_id: cash,
            category_id: food,
            description: "",
            now: NOW,
        },
    )
    .unwrap();
    // Transfer 10,000 from cash -> bank
    transaction_repo::insert_transfer(
        &conn,
        &transaction_repo::InsertTransferInput {
            occurred_on: "2026-05-25",
            amount: 10_000,
            account_id: cash,
            counter_account_id: bank,
            description: "",
            now: NOW,
        },
    )
    .unwrap();

    let balances = balance_repo::list_balances(&conn).unwrap();
    let by_id: std::collections::HashMap<i64, i64> =
        balances.iter().map(|b| (b.account_id, b.balance)).collect();
    assert_eq!(by_id[&cash], 50_000 - 3_000 - 10_000);
    assert_eq!(by_id[&bank], 200_000 + 10_000);
}

#[test]
fn balance_repo_matches_pure_compute_balance() {
    let (conn, cash, bank, food) = seeded_db();
    transaction_repo::insert(
        &conn,
        &transaction_repo::InsertInput {
            occurred_on: "2026-05-25",
            type_: TxType::Expense,
            amount: 1_234,
            account_id: cash,
            category_id: food,
            description: "",
            now: NOW,
        },
    )
    .unwrap();
    transaction_repo::insert_transfer(
        &conn,
        &transaction_repo::InsertTransferInput {
            occurred_on: "2026-05-25",
            amount: 5_678,
            account_id: bank,
            counter_account_id: cash,
            description: "",
            now: NOW,
        },
    )
    .unwrap();

    let (all_txs, _) = transaction_repo::list(
        &conn,
        &transaction_repo::ListFilter::default(),
        0,
        1_000,
    )
    .unwrap();
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM transactions", [], |r| r.get(0))
        .unwrap();
    assert_eq!(all_txs.len() as i64, count, "test must read all transactions");
    let balances_sql = balance_repo::list_balances(&conn).unwrap();
    for row in &balances_sql {
        let initial = row.initial_balance;
        let pure = compute_balance(initial, &all_txs, row.account_id);
        assert_eq!(
            row.balance, pure,
            "SQL balance ({}) != pure compute_balance ({}) for account {}",
            row.balance, pure, row.account_id
        );
    }
}

#[test]
fn list_balances_uses_both_indexes() {
    let mut conn = Connection::open_in_memory().unwrap();
    migrations::run(&mut conn).unwrap();

    let explain_sql = format!("EXPLAIN QUERY PLAN {}", balance_repo::LIST_BALANCES_SQL);
    let mut stmt = conn.prepare(&explain_sql).unwrap();
    // EXPLAIN QUERY PLAN columns: id, parent, notused, detail
    let details: Vec<String> = stmt
        .query_map([], |row| row.get::<_, String>(3))
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap();

    let joined = details.join("\n");
    assert!(
        details
            .iter()
            .any(|d| d.contains("idx_tx_account") && !d.contains("idx_tx_counter_account")),
        "expected at least one plan row to use idx_tx_account; got:\n{joined}"
    );
    assert!(
        details.iter().any(|d| d.contains("idx_tx_counter_account")),
        "expected at least one plan row to use idx_tx_counter_account; got:\n{joined}"
    );
}
