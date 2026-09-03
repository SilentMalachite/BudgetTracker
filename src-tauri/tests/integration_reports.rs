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

#[test]
fn buckets_between_bounds_both_ends_and_excludes_transfers() {
    let (conn, a, b, expense, income) = seeded_db();
    insert_tx(&conn, "2026-03-31", "expense", 100, a, None, Some(expense));
    insert_tx(&conn, "2026-04-01", "expense", 200, a, None, Some(expense));
    insert_tx(&conn, "2026-04-30", "income", 900, a, None, Some(income));
    insert_tx(&conn, "2026-05-31", "expense", 400, a, None, Some(expense));
    insert_tx(&conn, "2026-06-01", "expense", 800, a, None, Some(expense));
    // 振替は収入にも支出にも数えない (規約3)。
    insert_tx(&conn, "2026-04-15", "transfer", 5_000, a, Some(b), None);

    let buckets = report_repo::monthly_buckets_between(&conn, "2026-04", "2026-05").unwrap();

    assert_eq!(buckets.len(), 2);
    assert_eq!(buckets[0].year_month, "2026-04");
    assert_eq!(buckets[0].income, 900);
    assert_eq!(buckets[0].expense, 200);
    assert_eq!(buckets[1].year_month, "2026-05");
    assert_eq!(buckets[1].expense, 400);
}

#[test]
fn category_totals_are_ordered_by_amount_desc() {
    let (conn, a, _b, expense, income) = seeded_db();
    insert_tx(&conn, "2026-04-02", "expense", 300, a, None, Some(expense));
    insert_tx(&conn, "2026-05-02", "expense", 400, a, None, Some(expense));
    insert_tx(&conn, "2026-04-25", "income", 10_000, a, None, Some(income));

    let totals = report_repo::category_totals_between(&conn, "2026-04", "2026-05").unwrap();

    assert_eq!(totals.len(), 2);
    assert_eq!(totals[0].type_, "income");
    assert_eq!(totals[0].amount, 10_000);
    assert_eq!(totals[1].type_, "expense");
    assert_eq!(totals[1].amount, 700);
}

#[test]
fn category_month_amounts_split_by_month() {
    let (conn, a, _b, expense, _income) = seeded_db();
    insert_tx(&conn, "2026-04-02", "expense", 300, a, None, Some(expense));
    insert_tx(&conn, "2026-04-20", "expense", 200, a, None, Some(expense));
    insert_tx(&conn, "2026-05-02", "expense", 400, a, None, Some(expense));

    let rows = report_repo::category_month_amounts(&conn, "2026-04", "2026-05").unwrap();

    assert_eq!(rows.len(), 2);
    assert_eq!((rows[0].year_month.as_str(), rows[0].amount), ("2026-04", 500));
    assert_eq!((rows[1].year_month.as_str(), rows[1].amount), ("2026-05", 400));
    assert_eq!(rows[0].type_, "expense");
}

#[test]
fn net_worth_delta_counts_both_legs_of_a_transfer() {
    let (conn, a, b, expense, income) = seeded_db();
    insert_tx(&conn, "2026-04-05", "income", 1_000, a, None, Some(income));
    insert_tx(&conn, "2026-04-06", "expense", 400, a, None, Some(expense));
    // 両口座とも非アーカイブなので、振替は出金と入金で相殺されて 0 になる。
    insert_tx(&conn, "2026-04-07", "transfer", 5_000, a, Some(b), None);

    let deltas = report_repo::monthly_net_worth_delta(&conn, "2026-04", "2026-04").unwrap();

    assert_eq!(deltas, vec![("2026-04".to_string(), 600)]);
}

#[test]
fn net_worth_delta_drops_the_leg_that_lands_in_an_archived_account() {
    let (conn, a, b, _expense, _income) = seeded_db();
    insert_tx(&conn, "2026-04-07", "transfer", 5_000, a, Some(b), None);
    conn.execute("UPDATE accounts SET archived_at = ?1 WHERE id = ?2", rusqlite::params![NOW, b])
        .unwrap();

    let deltas = report_repo::monthly_net_worth_delta(&conn, "2026-04", "2026-04").unwrap();

    // 受け側が隠れた分、送金の -5000 だけが残る。
    assert_eq!(deltas, vec![("2026-04".to_string(), -5_000)]);
}

#[test]
fn opening_net_worth_sums_initial_balances_and_everything_before_the_window() {
    let (conn, a, _b, expense, income) = seeded_db();
    conn.execute("UPDATE accounts SET initial_balance = 10000 WHERE id = ?1", rusqlite::params![a])
        .unwrap();
    insert_tx(&conn, "2026-03-31", "income", 2_000, a, None, Some(income));
    // ウィンドウ内なので開始残高には入らない。
    insert_tx(&conn, "2026-04-01", "expense", 500, a, None, Some(expense));

    assert_eq!(report_repo::opening_net_worth(&conn, "2026-04").unwrap(), 12_000);
}

#[test]
fn opening_net_worth_ignores_archived_accounts() {
    let (conn, a, b, _expense, income) = seeded_db();
    conn.execute(
        "UPDATE accounts SET initial_balance = 10000 WHERE id IN (?1, ?2)",
        rusqlite::params![a, b],
    )
    .unwrap();
    insert_tx(&conn, "2026-01-10", "income", 3_000, b, None, Some(income));
    conn.execute("UPDATE accounts SET archived_at = ?1 WHERE id = ?2", rusqlite::params![NOW, b])
        .unwrap();

    assert_eq!(report_repo::opening_net_worth(&conn, "2026-04").unwrap(), 10_000);
}
