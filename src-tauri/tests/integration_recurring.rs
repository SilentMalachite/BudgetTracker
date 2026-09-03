use budget_tracker_lib::domain::ledger::TxType;
use budget_tracker_lib::domain::recurring::{Frequency, RecurringRule};
use budget_tracker_lib::infra::migrations;
use budget_tracker_lib::infra::repo::recurring_repo;
use chrono::NaiveDate;
use rusqlite::{params, Connection};

const NOW: &str = "2026-05-25T00:00:00+00:00";

fn fresh() -> Connection {
    let mut conn = Connection::open_in_memory().unwrap();
    migrations::run(&mut conn).unwrap();
    conn
}

/// (account_id, counter_account_id, expense_category_id) を作る。
fn seed(conn: &Connection) -> (i64, i64, i64) {
    conn.execute(
        "INSERT INTO accounts(name, kind, currency, initial_balance, display_order, note,
                              created_at, updated_at)
         VALUES ('現金', 'cash', 'JPY', 0, 0, '', ?1, ?1)",
        params![NOW],
    )
    .unwrap();
    let account_id = conn.last_insert_rowid();

    conn.execute(
        "INSERT INTO accounts(name, kind, currency, initial_balance, display_order, note,
                              created_at, updated_at)
         VALUES ('普通預金', 'bank', 'JPY', 0, 1, '', ?1, ?1)",
        params![NOW],
    )
    .unwrap();
    let counter_account_id = conn.last_insert_rowid();

    conn.execute(
        "INSERT INTO categories(name, type, color, icon, display_order)
         VALUES ('家賃', 'expense', NULL, NULL, 0)",
        [],
    )
    .unwrap();
    let category_id = conn.last_insert_rowid();

    (account_id, counter_account_id, category_id)
}

fn monthly_rent(account_id: i64, category_id: i64) -> recurring_repo::InsertInput<'static> {
    recurring_repo::InsertInput {
        name: "家賃",
        type_: TxType::Expense,
        amount: 85_000,
        account_id,
        counter_account_id: None,
        category_id: Some(category_id),
        description: "毎月の家賃",
        frequency: Frequency::Monthly,
        day_of_month: Some(27),
        day_of_week: None,
        starts_on: "2026-01-27",
        ends_on: None,
    }
}

fn date(y: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, d).unwrap()
}

#[test]
fn insert_then_find_round_trips_every_column() {
    let conn = fresh();
    let (account_id, _, category_id) = seed(&conn);

    let id = recurring_repo::insert(&conn, &monthly_rent(account_id, category_id)).unwrap();
    let rule = recurring_repo::find_by_id(&conn, id).unwrap();

    assert_eq!(rule.name, "家賃");
    assert_eq!(rule.type_, TxType::Expense);
    assert_eq!(rule.amount, 85_000);
    assert_eq!(rule.account_id, account_id);
    assert_eq!(rule.counter_account_id, None);
    assert_eq!(rule.category_id, Some(category_id));
    assert_eq!(rule.description, "毎月の家賃");
    assert_eq!(rule.frequency, Frequency::Monthly);
    assert_eq!(rule.day_of_month, Some(27));
    assert_eq!(rule.day_of_week, None);
    assert_eq!(rule.starts_on, "2026-01-27");
    assert_eq!(rule.ends_on, None);
    assert_eq!(rule.last_generated_on, None);
    assert!(rule.active);
}

#[test]
fn list_hides_inactive_rules_unless_asked() {
    let conn = fresh();
    let (account_id, _, category_id) = seed(&conn);
    let id = recurring_repo::insert(&conn, &monthly_rent(account_id, category_id)).unwrap();

    recurring_repo::set_active(&conn, id, false).unwrap();

    assert!(recurring_repo::list(&conn, false).unwrap().is_empty());
    assert_eq!(recurring_repo::list(&conn, true).unwrap().len(), 1);
}

#[test]
fn set_active_never_deletes_the_row() {
    let conn = fresh();
    let (account_id, _, category_id) = seed(&conn);
    let id = recurring_repo::insert(&conn, &monthly_rent(account_id, category_id)).unwrap();

    recurring_repo::set_active(&conn, id, false).unwrap();

    let rule = recurring_repo::find_by_id(&conn, id).unwrap();
    assert!(!rule.active);
}

#[test]
fn update_replaces_the_schedule_but_keeps_last_generated_on() {
    let conn = fresh();
    let (account_id, _, category_id) = seed(&conn);
    let id = recurring_repo::insert(&conn, &monthly_rent(account_id, category_id)).unwrap();
    recurring_repo::set_last_generated_on(&conn, id, "2026-03-27").unwrap();

    recurring_repo::update(
        &conn,
        id,
        &recurring_repo::UpdateInput {
            name: "家賃 (改定後)",
            type_: TxType::Expense,
            amount: 90_000,
            account_id,
            counter_account_id: None,
            category_id: Some(category_id),
            description: "",
            frequency: Frequency::Monthly,
            day_of_month: Some(1),
            day_of_week: None,
            starts_on: "2026-01-27",
            ends_on: Some("2027-01-27"),
        },
    )
    .unwrap();

    let rule = recurring_repo::find_by_id(&conn, id).unwrap();
    assert_eq!(rule.amount, 90_000);
    assert_eq!(rule.day_of_month, Some(1));
    assert_eq!(rule.ends_on.as_deref(), Some("2027-01-27"));
    // 過去に生成した分は動かさない (spec §5.4「ルール変更時の挙動」)。
    assert_eq!(rule.last_generated_on.as_deref(), Some("2026-03-27"));
}

#[test]
fn insert_generated_links_each_row_back_to_its_rule() {
    let conn = fresh();
    let (account_id, _, category_id) = seed(&conn);
    let id = recurring_repo::insert(&conn, &monthly_rent(account_id, category_id)).unwrap();
    let rule = recurring_repo::find_by_id(&conn, id).unwrap();

    let written = recurring_repo::insert_generated(
        &conn,
        &rule,
        &[date(2026, 1, 27), date(2026, 2, 27)],
        NOW,
    )
    .unwrap();

    assert_eq!(written, 2);
    let rows: Vec<(String, i64, Option<i64>)> = conn
        .prepare("SELECT occurred_on, amount, recurring_id FROM transactions ORDER BY occurred_on")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    assert_eq!(
        rows,
        vec![
            ("2026-01-27".to_string(), 85_000, Some(id)),
            ("2026-02-27".to_string(), 85_000, Some(id)),
        ]
    );
}

#[test]
fn insert_generated_writes_transfer_rows_without_a_category() {
    let conn = fresh();
    let (account_id, counter_account_id, _) = seed(&conn);
    let id = recurring_repo::insert(
        &conn,
        &recurring_repo::InsertInput {
            name: "貯金",
            type_: TxType::Transfer,
            amount: 30_000,
            account_id,
            counter_account_id: Some(counter_account_id),
            category_id: None,
            description: "",
            frequency: Frequency::Monthly,
            day_of_month: Some(25),
            day_of_week: None,
            starts_on: "2026-01-25",
            ends_on: None,
        },
    )
    .unwrap();
    let rule = recurring_repo::find_by_id(&conn, id).unwrap();

    recurring_repo::insert_generated(&conn, &rule, &[date(2026, 1, 25)], NOW).unwrap();

    let (type_, counter, category): (String, Option<i64>, Option<i64>) = conn
        .query_row(
            "SELECT type, counter_account_id, category_id FROM transactions",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(type_, "transfer");
    assert_eq!(counter, Some(counter_account_id));
    assert_eq!(category, None);
}

#[test]
fn schedule_parses_the_stored_iso_dates() {
    let conn = fresh();
    let (account_id, _, category_id) = seed(&conn);
    let id = recurring_repo::insert(&conn, &monthly_rent(account_id, category_id)).unwrap();
    let rule: RecurringRule = recurring_repo::find_by_id(&conn, id).unwrap();

    let schedule = rule.schedule().unwrap();
    assert_eq!(schedule.starts_on, date(2026, 1, 27));
    assert_eq!(schedule.ends_on, None);
    assert_eq!(schedule.frequency, Frequency::Monthly);
}

use budget_tracker_lib::commands::recurring::{self as recurring_cmd, RecurringRuleInput};

fn input_expense(account_id: i64, category_id: i64) -> RecurringRuleInput {
    RecurringRuleInput {
        name: "家賃".into(),
        type_: "expense".into(),
        amount: 85_000,
        account_id,
        counter_account_id: None,
        category_id: Some(category_id),
        description: "毎月の家賃".into(),
        frequency: "monthly".into(),
        day_of_month: Some(27),
        day_of_week: None,
        starts_on: "2026-01-27".into(),
        ends_on: None,
    }
}

#[test]
fn create_rule_rejects_an_archived_account() {
    let conn = fresh();
    let (account_id, _, category_id) = seed(&conn);
    conn.execute(
        "UPDATE accounts SET archived_at = ?1 WHERE id = ?2",
        params![NOW, account_id],
    )
    .unwrap();

    let err = recurring_cmd::create_rule_for_conn(&conn, input_expense(account_id, category_id))
        .unwrap_err();
    assert!(err.to_string().contains("archived"), "{err}");
}

#[test]
fn create_rule_rejects_a_category_of_the_wrong_type() {
    let conn = fresh();
    let (account_id, _, _) = seed(&conn);
    conn.execute(
        "INSERT INTO categories(name, type, color, icon, display_order)
         VALUES ('給与', 'income', NULL, NULL, 1)",
        [],
    )
    .unwrap();
    let income_category = conn.last_insert_rowid();

    let err = recurring_cmd::create_rule_for_conn(&conn, input_expense(account_id, income_category))
        .unwrap_err();
    assert!(err.to_string().contains("does not match"), "{err}");
}

#[test]
fn create_rule_rejects_an_unknown_account() {
    let conn = fresh();
    let (_, _, category_id) = seed(&conn);

    assert!(recurring_cmd::create_rule_for_conn(&conn, input_expense(9_999, category_id)).is_err());
}

#[test]
fn list_views_carry_the_next_occurrence() {
    let conn = fresh();
    let (account_id, _, category_id) = seed(&conn);
    recurring_cmd::create_rule_for_conn(&conn, input_expense(account_id, category_id)).unwrap();

    let views = recurring_cmd::list_rule_views_for_conn(&conn, false, date(2026, 2, 1)).unwrap();

    assert_eq!(views.len(), 1);
    assert_eq!(views[0].next_occurrence.as_deref(), Some("2026-02-27"));
}

#[test]
fn update_rule_revalidates_the_new_shape() {
    let conn = fresh();
    let (account_id, _, category_id) = seed(&conn);
    let rule =
        recurring_cmd::create_rule_for_conn(&conn, input_expense(account_id, category_id)).unwrap();

    let broken = RecurringRuleInput {
        day_of_month: Some(40),
        ..input_expense(account_id, category_id)
    };
    assert!(recurring_cmd::update_rule_for_conn(&conn, rule.id, broken).is_err());

    let fixed = RecurringRuleInput {
        amount: 90_000,
        ..input_expense(account_id, category_id)
    };
    let updated = recurring_cmd::update_rule_for_conn(&conn, rule.id, fixed).unwrap();
    assert_eq!(updated.amount, 90_000);
}

#[test]
fn preview_counts_the_backfill_a_past_starts_on_would_create() {
    // 2026-01-27 開始・毎月 27 日。today = 2026-05-01 なら 1〜4 月の 4 件。
    let preview =
        recurring_cmd::preview_for_input(&input_expense(1, 2), date(2026, 5, 1), 100).unwrap();

    assert_eq!(preview.backfill_total, 4);
    assert_eq!(
        preview.backfill,
        vec!["2026-01-27", "2026-02-27", "2026-03-27", "2026-04-27"]
    );
    assert!(!preview.truncated);
}

#[test]
fn preview_truncates_the_backfill_at_the_limit_but_keeps_the_total() {
    let preview =
        recurring_cmd::preview_for_input(&input_expense(1, 2), date(2026, 5, 1), 2).unwrap();

    assert_eq!(preview.backfill_total, 4);
    assert_eq!(preview.backfill, vec!["2026-01-27", "2026-02-27"]);
    assert!(preview.truncated);
}

#[test]
fn preview_lists_the_next_three_upcoming_dates() {
    let preview =
        recurring_cmd::preview_for_input(&input_expense(1, 2), date(2026, 5, 1), 100).unwrap();

    assert_eq!(preview.upcoming, vec!["2026-05-27", "2026-06-27", "2026-07-27"]);
}

#[test]
fn preview_of_a_future_rule_has_no_backfill() {
    let future = RecurringRuleInput {
        starts_on: "2026-09-27".into(),
        ..input_expense(1, 2)
    };
    let preview = recurring_cmd::preview_for_input(&future, date(2026, 5, 1), 100).unwrap();

    assert_eq!(preview.backfill_total, 0);
    assert!(preview.backfill.is_empty());
    assert_eq!(preview.upcoming.first().map(String::as_str), Some("2026-09-27"));
}

#[test]
fn preview_rejects_a_malformed_rule_without_touching_the_database() {
    let broken = RecurringRuleInput {
        frequency: "daily".into(),
        ..input_expense(1, 2)
    };
    assert!(recurring_cmd::preview_for_input(&broken, date(2026, 5, 1), 100).is_err());
}
