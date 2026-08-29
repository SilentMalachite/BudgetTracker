use budget_tracker_lib::domain::account::AccountKind;
use budget_tracker_lib::domain::category::CategoryType;
use budget_tracker_lib::domain::ledger::TxType;
use budget_tracker_lib::error::AppError;
use budget_tracker_lib::infra::migrations;
use budget_tracker_lib::infra::repo::{account_repo, category_repo, transaction_repo};
use rusqlite::Connection;

const NOW: &str = "2026-05-25T00:00:00+00:00";

fn seeded_db() -> (Connection, i64, i64) {
    let mut conn = Connection::open_in_memory().unwrap();
    migrations::run(&mut conn).unwrap();
    let cash = account_repo::insert(
        &conn,
        &account_repo::InsertInput {
            name: "cash",
            kind: AccountKind::Cash,
            currency: "JPY",
            initial_balance: 100_000,
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
            initial_balance: 500_000,
            display_order: 1,
            note: "",
            now: NOW,
        },
    )
    .unwrap();
    (conn, cash, bank)
}

#[test]
fn insert_transfer_persists_with_correct_shape() {
    let (conn, cash, bank) = seeded_db();
    let id = transaction_repo::insert_transfer(
        &conn,
        &transaction_repo::InsertTransferInput {
            occurred_on: "2026-05-25",
            amount: 30_000,
            account_id: cash,
            counter_account_id: bank,
            description: "ATM入金",
            now: NOW,
        },
    )
    .unwrap();

    let tx = transaction_repo::find_by_id(&conn, id).unwrap();
    assert!(matches!(tx.type_, TxType::Transfer));
    assert_eq!(tx.amount, 30_000);
    assert_eq!(tx.account_id, cash);
    assert_eq!(tx.counter_account_id, Some(bank));
    assert!(tx.category_id.is_none());
    assert_eq!(tx.description, "ATM入金");
}

#[test]
fn transfer_appears_in_list_unfiltered() {
    let (conn, cash, bank) = seeded_db();
    transaction_repo::insert_transfer(
        &conn,
        &transaction_repo::InsertTransferInput {
            occurred_on: "2026-05-25",
            amount: 30_000,
            account_id: cash,
            counter_account_id: bank,
            description: "",
            now: NOW,
        },
    )
    .unwrap();

    let (items, total) =
        transaction_repo::list(&conn, &transaction_repo::ListFilter::default(), 0, 50).unwrap();
    assert_eq!(total, 1);
    assert!(matches!(items[0].type_, TxType::Transfer));
}

#[test]
fn list_can_filter_by_transfer_type() {
    let (conn, cash, bank) = seeded_db();
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

    let (items, total) = transaction_repo::list(
        &conn,
        &transaction_repo::ListFilter {
            type_: Some(TxType::Transfer),
            ..Default::default()
        },
        0,
        50,
    )
    .unwrap();
    assert_eq!(total, 1);
    assert_eq!(items.len(), 1);
}

#[test]
fn account_filter_includes_inbound_transfer_rows() {
    let (conn, cash, bank) = seeded_db();
    transaction_repo::insert_transfer(
        &conn,
        &transaction_repo::InsertTransferInput {
            occurred_on: "2026-05-25",
            amount: 10_000,
            account_id: cash,
            counter_account_id: bank,
            description: "to bank",
            now: NOW,
        },
    )
    .unwrap();

    let (items, total) = transaction_repo::list(
        &conn,
        &transaction_repo::ListFilter {
            account_id: Some(bank),
            ..Default::default()
        },
        0,
        50,
    )
    .unwrap();

    assert_eq!(total, 1);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].counter_account_id, Some(bank));
}

#[test]
fn update_transfer_changes_amount_and_destination() {
    let (conn, cash, bank) = seeded_db();
    let id = transaction_repo::insert_transfer(
        &conn,
        &transaction_repo::InsertTransferInput {
            occurred_on: "2026-05-25",
            amount: 1_000,
            account_id: cash,
            counter_account_id: bank,
            description: "",
            now: NOW,
        },
    )
    .unwrap();

    // Add a third account and update the transfer to send into it.
    let wallet = account_repo::insert(
        &conn,
        &account_repo::InsertInput {
            name: "wallet",
            kind: AccountKind::Cash,
            currency: "JPY",
            initial_balance: 0,
            display_order: 2,
            note: "",
            now: NOW,
        },
    )
    .unwrap();

    transaction_repo::update_transfer(
        &conn,
        id,
        &transaction_repo::UpdateTransferInput {
            occurred_on: "2026-05-26",
            amount: 2_000,
            account_id: cash,
            counter_account_id: wallet,
            description: "fix",
            now: NOW,
        },
    )
    .unwrap();

    let tx = transaction_repo::find_by_id(&conn, id).unwrap();
    assert_eq!(tx.amount, 2_000);
    assert_eq!(tx.counter_account_id, Some(wallet));
    assert_eq!(tx.occurred_on, "2026-05-26");
}

#[test]
fn update_transaction_rejects_existing_transfer_row() {
    let (conn, cash, bank) = seeded_db();
    let id = transaction_repo::insert_transfer(
        &conn,
        &transaction_repo::InsertTransferInput {
            occurred_on: "2026-05-25",
            amount: 1_000,
            account_id: cash,
            counter_account_id: bank,
            description: "",
            now: NOW,
        },
    )
    .unwrap();
    let cat = category_repo::insert(
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
    let err = transaction_repo::update(
        &conn,
        id,
        &transaction_repo::UpdateInput {
            occurred_on: "2026-05-25",
            type_: TxType::Expense,
            amount: 100,
            account_id: cash,
            category_id: cat,
            description: "",
            now: NOW,
        },
    )
    .unwrap_err();
    assert!(matches!(err, AppError::InvalidArgument(_)));
    assert!(err
        .to_string()
        .contains("use create_transfer or update_transfer for transfer rows"));
}

#[test]
fn update_transfer_refuses_non_transfer_row() {
    let (conn, cash, bank) = seeded_db();
    let cat = category_repo::insert(
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
    let expense_id = transaction_repo::insert(
        &conn,
        &transaction_repo::InsertInput {
            occurred_on: "2026-05-25",
            type_: TxType::Expense,
            amount: 100,
            account_id: cash,
            category_id: cat,
            description: "",
            now: NOW,
        },
    )
    .unwrap();

    let err = transaction_repo::update_transfer(
        &conn,
        expense_id,
        &transaction_repo::UpdateTransferInput {
            occurred_on: "2026-05-25",
            amount: 100,
            account_id: cash,
            counter_account_id: bank,
            description: "",
            now: NOW,
        },
    )
    .unwrap_err();
    assert!(matches!(err, AppError::NotFound(_)));
}

#[test]
fn delete_works_on_transfer_row() {
    let (conn, cash, bank) = seeded_db();
    let id = transaction_repo::insert_transfer(
        &conn,
        &transaction_repo::InsertTransferInput {
            occurred_on: "2026-05-25",
            amount: 500,
            account_id: cash,
            counter_account_id: bank,
            description: "",
            now: NOW,
        },
    )
    .unwrap();
    transaction_repo::delete(&conn, id).unwrap();
    let err = transaction_repo::find_by_id(&conn, id).unwrap_err();
    assert!(matches!(err, AppError::NotFound(_)));
}
