use budget_tracker_lib::commands::budgets::{set_budget_for_conn, SetBudgetInput};
use budget_tracker_lib::domain::account::AccountKind;
use budget_tracker_lib::domain::budget::{parse_year_month, RawSetBudgetInput};
use budget_tracker_lib::domain::category::CategoryType;
use budget_tracker_lib::domain::ledger::TxType;
use budget_tracker_lib::infra::migrations;
use budget_tracker_lib::infra::repo::{
    account_repo, budget_repo, category_repo, transaction_repo,
};
use chrono::NaiveDate;
use rusqlite::Connection;

const NOW: &str = "2026-05-25T00:00:00+00:00";

fn seeded_db() -> (Connection, i64, i64) {
    let mut conn = Connection::open_in_memory().unwrap();
    migrations::run(&mut conn).unwrap();
    let account = account_repo::insert(
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
    let food = category_repo::insert(
        &conn,
        &category_repo::InsertInput {
            name: "Food",
            type_: CategoryType::Expense,
            color: Some("#FFAA00"),
            icon: Some("utensils"),
            display_order: 0,
        },
    )
    .unwrap();
    (conn, account, food)
}

fn add_category(conn: &Connection, name: &str, type_: CategoryType, display_order: i64) -> i64 {
    category_repo::insert(
        conn,
        &category_repo::InsertInput {
            name,
            type_,
            color: None,
            icon: None,
            display_order,
        },
    )
    .unwrap()
}

#[test]
fn list_statuses_returns_active_expense_categories_even_without_budget() {
    let (conn, _account, food) = seeded_db();
    let transport = add_category(&conn, "Transport", CategoryType::Expense, 1);
    let income = add_category(&conn, "Salary", CategoryType::Income, 0);
    category_repo::set_archived(&conn, transport, Some(NOW)).unwrap();

    let statuses = budget_repo::list_statuses(
        &conn,
        parse_year_month("2026-05").unwrap(),
        NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(),
    )
    .unwrap();

    assert_eq!(statuses.len(), 1);
    assert_eq!(statuses[0].category_id, food);
    assert_eq!(statuses[0].category_name, "Food");
    assert_eq!(statuses[0].category_color.as_deref(), Some("#FFAA00"));
    assert_eq!(statuses[0].category_icon.as_deref(), Some("utensils"));
    assert_eq!(statuses[0].budget_id, None);
    assert_eq!(statuses[0].budgeted, 0);
    assert_eq!(statuses[0].spent, 0);
    assert_eq!(statuses[0].alert_threshold, 80);
    assert!(statuses.iter().all(|s| s.category_id != income));
}

#[test]
fn list_statuses_counts_only_expense_spending_inside_selected_month() {
    let (conn, account, food) = seeded_db();
    let salary = add_category(&conn, "Salary", CategoryType::Income, 0);
    transaction_repo::insert(
        &conn,
        &transaction_repo::InsertInput {
            occurred_on: "2026-05-10",
            type_: TxType::Expense,
            amount: 12_000,
            account_id: account,
            category_id: food,
            description: "groceries",
            now: NOW,
        },
    )
    .unwrap();
    transaction_repo::insert(
        &conn,
        &transaction_repo::InsertInput {
            occurred_on: "2026-04-30",
            type_: TxType::Expense,
            amount: 999_999,
            account_id: account,
            category_id: food,
            description: "other month",
            now: NOW,
        },
    )
    .unwrap();
    transaction_repo::insert(
        &conn,
        &transaction_repo::InsertInput {
            occurred_on: "2026-05-15",
            type_: TxType::Income,
            amount: 300_000,
            account_id: account,
            category_id: salary,
            description: "salary",
            now: NOW,
        },
    )
    .unwrap();
    let other_account = account_repo::insert(
        &conn,
        &account_repo::InsertInput {
            name: "bank",
            kind: AccountKind::Bank,
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
            occurred_on: "2026-05-20",
            amount: 500_000,
            account_id: account,
            counter_account_id: other_account,
            description: "move",
            now: NOW,
        },
    )
    .unwrap();

    let statuses = budget_repo::list_statuses(
        &conn,
        parse_year_month("2026-05").unwrap(),
        NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(),
    )
    .unwrap();

    assert_eq!(statuses.len(), 1);
    assert_eq!(statuses[0].spent, 12_000);
}

#[test]
fn set_monthly_budget_inserts_then_updates_same_category_month() {
    let (conn, _account, food) = seeded_db();
    let first = budget_repo::set_monthly_budget(
        &conn,
        &budget_tracker_lib::domain::budget::validate_set_budget_input(&RawSetBudgetInput {
            category_id: food,
            year_month: "2026-05",
            amount: 50_000,
            alert_threshold: 80,
        })
        .unwrap(),
    )
    .unwrap();
    let second = budget_repo::set_monthly_budget(
        &conn,
        &budget_tracker_lib::domain::budget::validate_set_budget_input(&RawSetBudgetInput {
            category_id: food,
            year_month: "2026-05",
            amount: 60_000,
            alert_threshold: 90,
        })
        .unwrap(),
    )
    .unwrap();

    assert_eq!(second.id, first.id);
    assert_eq!(second.category_id, food);
    assert_eq!(second.period, "monthly");
    assert_eq!(second.amount, 60_000);
    assert_eq!(second.starts_on, "2026-05-01");
    assert_eq!(second.ends_on, None);
    assert_eq!(second.alert_threshold, 90);
}

#[test]
fn set_budget_for_conn_rejects_invalid_month() {
    let (conn, _account, food) = seeded_db();

    let err = set_budget_for_conn(
        &conn,
        SetBudgetInput {
            category_id: food,
            year_month: "2026-13".into(),
            amount: 10_000,
            alert_threshold: 80,
        },
    )
    .unwrap_err();

    assert!(matches!(
        err,
        budget_tracker_lib::error::AppError::InvalidArgument(_)
    ));
}

#[test]
fn set_budget_for_conn_rejects_income_and_archived_categories() {
    let (conn, _account, food) = seeded_db();
    let income = add_category(&conn, "Salary", CategoryType::Income, 0);
    let archived = add_category(&conn, "Old", CategoryType::Expense, 1);
    category_repo::set_archived(&conn, archived, Some(NOW)).unwrap();

    for category_id in [income, archived] {
        let err = set_budget_for_conn(
            &conn,
            SetBudgetInput {
                category_id,
                year_month: "2026-05".into(),
                amount: 10_000,
                alert_threshold: 80,
            },
        )
        .unwrap_err();

        assert!(matches!(
            err,
            budget_tracker_lib::error::AppError::InvalidArgument(_)
        ));
    }

    let ok = set_budget_for_conn(
        &conn,
        SetBudgetInput {
            category_id: food,
            year_month: "2026-05".into(),
            amount: 10_000,
            alert_threshold: 80,
        },
    )
    .unwrap();
    assert_eq!(ok.category_id, food);
}
