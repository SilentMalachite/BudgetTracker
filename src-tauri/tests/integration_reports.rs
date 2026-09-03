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

#[test]
fn monthly_report_compares_against_last_month_and_last_year() {
    let (conn, a, _b, expense, income) = seeded_db();
    insert_tx(&conn, "2025-05-10", "expense", 1_000, a, None, Some(expense));
    insert_tx(&conn, "2026-04-10", "expense", 2_000, a, None, Some(expense));
    insert_tx(&conn, "2026-05-10", "expense", 2_500, a, None, Some(expense));
    insert_tx(&conn, "2026-05-25", "income", 300_000, a, None, Some(income));

    let report = budget_tracker_lib::commands::reports::build_monthly_report(&conn, 2026, 5).unwrap();

    assert_eq!(report.current.income, 300_000);
    assert_eq!(report.current.expense, 2_500);
    assert_eq!(report.current.net, 297_500);
    assert_eq!(report.prev_month.expense, 2_000);
    assert_eq!(report.prev_year.expense, 1_000);
    assert_eq!(report.mom.expense_diff, 500);
    assert_eq!(report.mom.expense_percent, Some(25));
    assert_eq!(report.yoy.expense_percent, Some(150));
    assert_eq!(report.top_expense.len(), 1);
    assert_eq!(report.top_income.len(), 1);
    assert_eq!(report.top_expense[0].amount, 2_500);
}

#[test]
fn monthly_report_leaves_percent_empty_without_a_baseline() {
    let (conn, a, _b, expense, _income) = seeded_db();
    insert_tx(&conn, "2026-05-10", "expense", 2_500, a, None, Some(expense));

    let report = budget_tracker_lib::commands::reports::build_monthly_report(&conn, 2026, 5).unwrap();

    assert_eq!(report.mom.expense_percent, None);
    assert_eq!(report.yoy.expense_percent, None);
}

#[test]
fn yearly_report_always_has_twelve_months() {
    let (conn, a, _b, expense, income) = seeded_db();
    insert_tx(&conn, "2026-02-10", "expense", 600, a, None, Some(expense));
    insert_tx(&conn, "2026-07-10", "expense", 1_800, a, None, Some(expense));
    insert_tx(&conn, "2026-07-25", "income", 12_000, a, None, Some(income));
    // 前年と翌年は入らない。
    insert_tx(&conn, "2025-12-31", "expense", 9_999, a, None, Some(expense));

    let report = budget_tracker_lib::commands::reports::build_yearly_report(&conn, 2026).unwrap();

    assert_eq!(report.months.len(), 12);
    assert_eq!(report.months[0].year_month, "2026-01");
    assert_eq!(report.months[11].year_month, "2026-12");
    assert_eq!(report.total_expense, 2_400);
    assert_eq!(report.total_income, 12_000);
    assert_eq!(report.avg_expense, 200); // 2400 / 12
    assert_eq!(report.max_expense_month, Some("2026-07".to_string()));
}

use budget_tracker_lib::commands::reports::range_months;

#[test]
fn range_months_lists_the_closed_interval() {
    let months = range_months("2026-03", "2026-05").unwrap();
    assert_eq!(months, vec!["2026-03", "2026-04", "2026-05"]);
    assert_eq!(range_months("2026-05", "2026-05").unwrap().len(), 1);
}

#[test]
fn range_months_rejects_bad_ranges() {
    assert!(range_months("2026-05", "2026-04").is_err()); // 逆転
    assert!(range_months("2026/05", "2026-06").is_err()); // 書式
    assert!(range_months("2020-01", "2026-01").is_err()); // 73ヶ月 > 60
    assert!(range_months("2021-02", "2026-02").is_err()); // 60ヶ月ちょうどの1つ外
    assert!(range_months("2021-03", "2026-02").unwrap().len() == 60);
}

#[test]
fn category_report_zero_fills_and_ranks_by_period_total() {
    let (conn, a, _b, expense, income) = seeded_db();
    insert_tx(&conn, "2026-04-02", "expense", 300, a, None, Some(expense));
    insert_tx(&conn, "2026-06-02", "expense", 700, a, None, Some(expense));
    insert_tx(&conn, "2026-05-25", "income", 300_000, a, None, Some(income));

    let report =
        budget_tracker_lib::commands::reports::build_category_report(&conn, "2026-04", "2026-06")
            .unwrap();

    assert_eq!(report.months, vec!["2026-04", "2026-05", "2026-06"]);
    // 期間合計の降順: 収入 300,000 が先、支出 1,000 が後。
    assert_eq!(report.series.len(), 2);
    assert_eq!(report.series[0].type_, "income");
    assert_eq!(report.series[1].points, vec![300, 0, 700]);
    assert_eq!(report.expense.len(), 1);
    assert_eq!(report.expense[0].amount, 1_000);
    assert_eq!(report.income[0].amount, 300_000);
}

#[test]
fn net_worth_series_accumulates_from_the_opening_balance() {
    let (conn, a, _b, expense, income) = seeded_db();
    conn.execute(
        "UPDATE accounts SET initial_balance = 10000 WHERE id = ?1",
        rusqlite::params![a],
    )
    .unwrap();
    insert_tx(&conn, "2026-03-31", "income", 5_000, a, None, Some(income));
    insert_tx(&conn, "2026-04-10", "expense", 1_000, a, None, Some(expense));
    insert_tx(&conn, "2026-06-10", "income", 2_000, a, None, Some(income));

    let report = budget_tracker_lib::commands::reports::build_net_worth_report(
        &conn, "2026-04", "2026-06",
    )
    .unwrap();

    let worth: Vec<i64> = report.points.iter().map(|p| p.net_worth).collect();
    // 開始 15,000 -> 4月 14,000 -> 5月 据え置き -> 6月 16,000
    assert_eq!(worth, vec![14_000, 14_000, 16_000]);

    let net: Vec<i64> = report.points.iter().map(|p| p.net).collect();
    assert_eq!(net, vec![-1_000, 0, 2_000]);

    // 窓が埋まるのは3点目から。(-1000 + 0 + 2000) / 3 = 333
    let avg: Vec<Option<i64>> = report.points.iter().map(|p| p.net_moving_avg).collect();
    assert_eq!(avg, vec![None, None, Some(333)]);
}

#[test]
fn net_worth_series_ends_on_the_dashboard_total() {
    use budget_tracker_lib::domain::balance::total_assets;
    use budget_tracker_lib::infra::repo::balance_repo;

    let (conn, a, b, expense, income) = seeded_db();
    conn.execute(
        "UPDATE accounts SET initial_balance = 50000 WHERE id IN (?1, ?2)",
        rusqlite::params![a, b],
    )
    .unwrap();
    insert_tx(&conn, "2026-04-10", "income", 8_000, a, None, Some(income));
    insert_tx(&conn, "2026-05-10", "expense", 3_000, b, None, Some(expense));
    insert_tx(&conn, "2026-05-11", "transfer", 7_000, a, Some(b), None);

    let report = budget_tracker_lib::commands::reports::build_net_worth_report(
        &conn, "2026-04", "2026-05",
    )
    .unwrap();
    let rows = balance_repo::list_balances(&conn).unwrap();
    let expected = total_assets(
        rows.iter()
            .map(|r| (r.archived_at.is_some(), r.balance)),
    );

    assert_eq!(report.points.last().unwrap().net_worth, expected);
}
