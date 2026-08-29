use budget_tracker_lib::domain::account::AccountKind;
use budget_tracker_lib::domain::category::CategoryType;
use budget_tracker_lib::domain::ledger::TxType;
use budget_tracker_lib::infra::migrations;
use budget_tracker_lib::infra::repo::{account_repo, category_repo, transaction_repo};
use rusqlite::Connection;

const NOW: &str = "2026-05-25T00:00:00+00:00";

fn seeded_db() -> (Connection, i64, i64) {
    let mut conn = Connection::open_in_memory().unwrap();
    migrations::run(&mut conn).unwrap();
    let acc = account_repo::insert(
        &conn,
        &account_repo::InsertInput {
            name: "cash",
            kind: AccountKind::Cash,
            currency: "JPY",
            initial_balance: 0,
            display_order: 0,
            note: "",
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
    (conn, acc, cat)
}

#[test]
fn insert_then_list_pagination() {
    let (conn, acc, cat) = seeded_db();
    for i in 1..=25 {
        transaction_repo::insert(
            &conn,
            &transaction_repo::InsertInput {
                occurred_on: &format!("2026-05-{:02}", (i % 28) + 1),
                type_: TxType::Expense,
                amount: 100 * i as i64,
                account_id: acc,
                category_id: cat,
                description: "lunch",
                now: NOW,
            },
        )
        .unwrap();
    }

    let (page0, total) =
        transaction_repo::list(&conn, &transaction_repo::ListFilter::default(), 0, 10).unwrap();
    assert_eq!(total, 25);
    assert_eq!(page0.len(), 10);

    let (page2, _) =
        transaction_repo::list(&conn, &transaction_repo::ListFilter::default(), 2, 10).unwrap();
    assert_eq!(page2.len(), 5);
}

#[test]
fn filter_by_search_and_type() {
    let (conn, acc, cat) = seeded_db();
    transaction_repo::insert(
        &conn,
        &transaction_repo::InsertInput {
            occurred_on: "2026-05-10",
            type_: TxType::Expense,
            amount: 100,
            account_id: acc,
            category_id: cat,
            description: "コーヒー",
            now: NOW,
        },
    )
    .unwrap();
    let income_cat = category_repo::insert(
        &conn,
        &category_repo::InsertInput {
            name: "Salary",
            type_: CategoryType::Income,
            color: None,
            icon: None,
            display_order: 0,
        },
    )
    .unwrap();
    transaction_repo::insert(
        &conn,
        &transaction_repo::InsertInput {
            occurred_on: "2026-05-25",
            type_: TxType::Income,
            amount: 200_000,
            account_id: acc,
            category_id: income_cat,
            description: "monthly salary",
            now: NOW,
        },
    )
    .unwrap();

    let (only_income, n_income) = transaction_repo::list(
        &conn,
        &transaction_repo::ListFilter {
            type_: Some(TxType::Income),
            ..Default::default()
        },
        0,
        50,
    )
    .unwrap();
    assert_eq!(n_income, 1);
    assert_eq!(only_income[0].amount, 200_000);

    let (coffee, n_coffee) = transaction_repo::list(
        &conn,
        &transaction_repo::ListFilter {
            search: Some("コーヒー".into()),
            ..Default::default()
        },
        0,
        50,
    )
    .unwrap();
    assert_eq!(n_coffee, 1);
    assert_eq!(coffee[0].description, "コーヒー");
}

#[test]
fn search_treats_percent_as_literal() {
    let (conn, acc, cat) = seeded_db();
    transaction_repo::insert(
        &conn,
        &transaction_repo::InsertInput {
            occurred_on: "2026-05-10",
            type_: TxType::Expense,
            amount: 100,
            account_id: acc,
            category_id: cat,
            description: "100%オフ",
            now: NOW,
        },
    )
    .unwrap();
    transaction_repo::insert(
        &conn,
        &transaction_repo::InsertInput {
            occurred_on: "2026-05-11",
            type_: TxType::Expense,
            amount: 100,
            account_id: acc,
            category_id: cat,
            description: "ランチ",
            now: NOW,
        },
    )
    .unwrap();
    let filter = transaction_repo::ListFilter {
        search: Some("%".into()),
        ..Default::default()
    };
    let (items, total) = transaction_repo::list(&conn, &filter, 0, 50).unwrap();
    assert_eq!(total, 1);
    assert_eq!(items[0].description, "100%オフ");
}

#[test]
fn list_includes_transfer_rows_after_slice_02() {
    // Phase 3 Slice 02 removed the `type IN ('income','expense')` clamp from
    // `transaction_repo::build_where` so the Transactions page can list transfers
    // alongside income/expense. Income/expense aggregates in `report_repo`
    // continue to filter `type IN ('income','expense')` at the SQL level.
    let (conn, acc, cat) = seeded_db();
    transaction_repo::insert(
        &conn,
        &transaction_repo::InsertInput {
            occurred_on: "2026-05-10",
            type_: TxType::Expense,
            amount: 100,
            account_id: acc,
            category_id: cat,
            description: "visible",
            now: NOW,
        },
    )
    .unwrap();
    // Insert a second account so the transfer source != destination.
    let other_acc = budget_tracker_lib::infra::repo::account_repo::insert(
        &conn,
        &budget_tracker_lib::infra::repo::account_repo::InsertInput {
            name: "other",
            kind: budget_tracker_lib::domain::account::AccountKind::Bank,
            currency: "JPY",
            initial_balance: 0,
            display_order: 1,
            note: "",
            now: NOW,
        },
    )
    .unwrap();
    transaction_repo::insert_transfer(
        &conn,
        &transaction_repo::InsertTransferInput {
            occurred_on: "2026-05-11",
            amount: 999_999,
            account_id: acc,
            counter_account_id: other_acc,
            description: "visible transfer",
            now: NOW,
        },
    )
    .unwrap();

    let (items, total) =
        transaction_repo::list(&conn, &transaction_repo::ListFilter::default(), 0, 50).unwrap();
    assert_eq!(total, 2);
    assert_eq!(items.len(), 2);
}

#[test]
fn insert_rejects_missing_account_fk() {
    let (conn, _acc, cat) = seeded_db();
    let err = transaction_repo::insert(
        &conn,
        &transaction_repo::InsertInput {
            occurred_on: "2026-05-25",
            type_: TxType::Expense,
            amount: 100,
            account_id: 9_999_999,
            category_id: cat,
            description: "ghost",
            now: NOW,
        },
    )
    .unwrap_err();
    assert!(
        matches!(err, budget_tracker_lib::error::AppError::Db(_)),
        "got {err:?}"
    );
}

#[test]
fn delete_removes_row() {
    let (conn, acc, cat) = seeded_db();
    let id = transaction_repo::insert(
        &conn,
        &transaction_repo::InsertInput {
            occurred_on: "2026-05-10",
            type_: TxType::Expense,
            amount: 100,
            account_id: acc,
            category_id: cat,
            description: "",
            now: NOW,
        },
    )
    .unwrap();
    transaction_repo::delete(&conn, id).unwrap();
    let err = transaction_repo::find_by_id(&conn, id).unwrap_err();
    assert!(matches!(
        err,
        budget_tracker_lib::error::AppError::NotFound(_)
    ));
}
