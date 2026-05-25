use budget_tracker_lib::domain::account::AccountKind;
use budget_tracker_lib::domain::category::CategoryType;
use budget_tracker_lib::infra::migrations;
use budget_tracker_lib::infra::repo::{account_repo, category_repo, report_repo};
use rusqlite::{params, Connection};

const NOW: &str = "2026-05-25T00:00:00+00:00";

fn seeded_db() -> (Connection, i64, i64, i64, i64) {
    let mut conn = Connection::open_in_memory().unwrap();
    migrations::run(&mut conn).unwrap();
    let account_a = account_repo::insert(
        &conn,
        &account_repo::InsertInput {
            name: "A",
            kind: AccountKind::Bank,
            currency: "JPY",
            initial_balance: 0,
            display_order: 0,
            note: "",
            now: NOW,
        },
    )
    .unwrap();
    let account_b = account_repo::insert(
        &conn,
        &account_repo::InsertInput {
            name: "B",
            kind: AccountKind::Cash,
            currency: "JPY",
            initial_balance: 0,
            display_order: 1,
            note: "",
            now: NOW,
        },
    )
    .unwrap();
    let expense = category_repo::insert(
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
    let income = category_repo::insert(
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
    (conn, account_a, account_b, expense, income)
}

fn insert_tx(
    conn: &Connection,
    occurred_on: &str,
    type_: &str,
    amount: i64,
    account_id: i64,
    counter_account_id: Option<i64>,
    category_id: Option<i64>,
) {
    conn.execute(
        "INSERT INTO transactions(
            occurred_on, type, amount, account_id, counter_account_id,
            category_id, created_at, updated_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)",
        params![
            occurred_on,
            type_,
            amount,
            account_id,
            counter_account_id,
            category_id,
            NOW,
        ],
    )
    .unwrap();
}

#[test]
fn monthly_summary_excludes_transfer_rows() {
    let (conn, account_a, account_b, expense, income) = seeded_db();
    insert_tx(
        &conn,
        "2026-05-10",
        "income",
        300_000,
        account_a,
        None,
        Some(income),
    );
    insert_tx(
        &conn,
        "2026-05-15",
        "expense",
        1_500,
        account_a,
        None,
        Some(expense),
    );
    insert_tx(
        &conn,
        "2026-05-20",
        "transfer",
        50_000,
        account_a,
        Some(account_b),
        None,
    );

    let summary = report_repo::monthly_summary(&conn, 2026, 5).unwrap();

    assert_eq!(summary.income, 300_000);
    assert_eq!(summary.expense, 1_500);
    assert_eq!(summary.net, 298_500);
    assert_eq!(summary.by_category.len(), 2);
    assert!(summary.by_category.iter().all(|entry| entry.amount > 0));
}

#[test]
fn monthly_summary_ignores_other_months() {
    let (conn, account_a, _account_b, expense, _income) = seeded_db();
    insert_tx(
        &conn,
        "2026-04-30",
        "expense",
        999,
        account_a,
        None,
        Some(expense),
    );
    insert_tx(
        &conn,
        "2026-06-01",
        "expense",
        777,
        account_a,
        None,
        Some(expense),
    );
    insert_tx(
        &conn,
        "2026-05-15",
        "expense",
        500,
        account_a,
        None,
        Some(expense),
    );

    let summary = report_repo::monthly_summary(&conn, 2026, 5).unwrap();

    assert_eq!(summary.income, 0);
    assert_eq!(summary.expense, 500);
    assert_eq!(summary.net, -500);
}

#[test]
fn monthly_buckets_since_returns_sorted_groups() {
    let (conn, account_a, account_b, expense, income) = seeded_db();
    insert_tx(
        &conn,
        "2026-03-10",
        "income",
        100,
        account_a,
        None,
        Some(income),
    );
    insert_tx(
        &conn,
        "2026-04-15",
        "expense",
        200,
        account_a,
        None,
        Some(expense),
    );
    insert_tx(
        &conn,
        "2026-05-20",
        "income",
        300,
        account_a,
        None,
        Some(income),
    );
    insert_tx(
        &conn,
        "2026-05-25",
        "expense",
        50,
        account_a,
        None,
        Some(expense),
    );
    insert_tx(
        &conn,
        "2026-05-26",
        "transfer",
        999,
        account_a,
        Some(account_b),
        None,
    );

    let buckets = report_repo::monthly_buckets_since(&conn, "2026-03").unwrap();

    assert_eq!(buckets.len(), 3);
    assert_eq!(buckets[0].year_month, "2026-03");
    assert_eq!(buckets[0].income, 100);
    assert_eq!(buckets[0].expense, 0);
    assert_eq!(buckets[1].year_month, "2026-04");
    assert_eq!(buckets[1].income, 0);
    assert_eq!(buckets[1].expense, 200);
    assert_eq!(buckets[2].year_month, "2026-05");
    assert_eq!(buckets[2].income, 300);
    assert_eq!(buckets[2].expense, 50);
}
