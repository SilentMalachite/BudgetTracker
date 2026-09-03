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

    recurring_repo::set_active(&conn, id, false, "2026-05-25").unwrap();

    assert!(recurring_repo::list(&conn, false).unwrap().is_empty());
    assert_eq!(recurring_repo::list(&conn, true).unwrap().len(), 1);
}

#[test]
fn set_active_never_deletes_the_row() {
    let conn = fresh();
    let (account_id, _, category_id) = seed(&conn);
    let id = recurring_repo::insert(&conn, &monthly_rent(account_id, category_id)).unwrap();

    recurring_repo::set_active(&conn, id, false, "2026-05-25").unwrap();

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

/// 展開でスキップされたルールには、ユーザーが自力で抜け出せる道が要る。
/// 参照先がアーカイブされた「後」の編集は、`update_transaction` と同じく
/// 「今その行が指している参照先」だけは許す。そうでないと口座を戻す以外に
/// 直しようがない (ルールの削除も無い)。
#[test]
fn a_rule_can_still_be_edited_after_its_account_was_archived() {
    let conn = fresh();
    let (account_id, counter_account_id, category_id) = seed(&conn);
    let rule =
        recurring_cmd::create_rule_for_conn(&conn, input_expense(account_id, category_id)).unwrap();
    conn.execute(
        "UPDATE accounts SET archived_at = ?1 WHERE id = ?2",
        params![NOW, account_id],
    )
    .unwrap();

    // 参照先を変えない編集は通る。
    let updated = recurring_cmd::update_rule_for_conn(
        &conn,
        rule.id,
        RecurringRuleInput {
            amount: 90_000,
            ..input_expense(account_id, category_id)
        },
    )
    .unwrap();
    assert_eq!(updated.amount, 90_000);

    // 生きている口座への付け替えも通る。
    let repointed = recurring_cmd::update_rule_for_conn(
        &conn,
        rule.id,
        RecurringRuleInput {
            account_id: counter_account_id,
            ..input_expense(account_id, category_id)
        },
    )
    .unwrap();
    assert_eq!(repointed.account_id, counter_account_id);
}

/// アーカイブ済みのカテゴリを指したままでも編集できる。種別の不一致は従来どおり拒否。
#[test]
fn a_rule_can_still_be_edited_after_its_category_was_archived() {
    let conn = fresh();
    let (account_id, _, category_id) = seed(&conn);
    let rule =
        recurring_cmd::create_rule_for_conn(&conn, input_expense(account_id, category_id)).unwrap();
    conn.execute(
        "UPDATE categories SET archived_at = ?1 WHERE id = ?2",
        params![NOW, category_id],
    )
    .unwrap();

    let updated = recurring_cmd::update_rule_for_conn(
        &conn,
        rule.id,
        RecurringRuleInput {
            amount: 90_000,
            ..input_expense(account_id, category_id)
        },
    )
    .unwrap();
    assert_eq!(updated.amount, 90_000);
}

/// 「今の参照先だから許す」であって「アーカイブ済みなら何でも許す」ではない。
/// ルールが指していない別のアーカイブ済み口座には付け替えられない。
#[test]
fn an_update_cannot_repoint_a_rule_onto_a_different_archived_account() {
    let conn = fresh();
    let (account_id, counter_account_id, category_id) = seed(&conn);
    let rule =
        recurring_cmd::create_rule_for_conn(&conn, input_expense(account_id, category_id)).unwrap();
    conn.execute(
        "UPDATE accounts SET archived_at = ?1 WHERE id = ?2",
        params![NOW, counter_account_id],
    )
    .unwrap();

    let err = recurring_cmd::update_rule_for_conn(
        &conn,
        rule.id,
        RecurringRuleInput {
            account_id: counter_account_id,
            ..input_expense(account_id, category_id)
        },
    )
    .unwrap_err();
    assert!(err.to_string().contains("archived"), "{err}");
}

#[test]
fn preview_counts_the_backfill_a_past_starts_on_would_create() {
    // 2026-01-27 開始・毎月 27 日。today = 2026-05-01 なら 1〜4 月の 4 件。
    let preview =
        recurring_cmd::preview_for_input(&input_expense(1, 2), None, date(2026, 5, 1), 100).unwrap();

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
        recurring_cmd::preview_for_input(&input_expense(1, 2), None, date(2026, 5, 1), 2).unwrap();

    assert_eq!(preview.backfill_total, 4);
    assert_eq!(preview.backfill, vec!["2026-01-27", "2026-02-27"]);
    assert!(preview.truncated);
}

#[test]
fn preview_lists_the_next_three_upcoming_dates() {
    let preview =
        recurring_cmd::preview_for_input(&input_expense(1, 2), None, date(2026, 5, 1), 100).unwrap();

    assert_eq!(preview.upcoming, vec!["2026-05-27", "2026-06-27", "2026-07-27"]);
}

#[test]
fn preview_of_a_future_rule_has_no_backfill() {
    let future = RecurringRuleInput {
        starts_on: "2026-09-27".into(),
        ..input_expense(1, 2)
    };
    let preview = recurring_cmd::preview_for_input(&future, None, date(2026, 5, 1), 100).unwrap();

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
    assert!(recurring_cmd::preview_for_input(&broken, None, date(2026, 5, 1), 100).is_err());
}

/// 編集中のルールは `last_generated_on` を窓の左端 (排他) として数える。
/// これを渡さないと、すでに生成し終えた過去まで「今すぐ生成されます」に数え上げて
/// しまい、プレビューが起きもしない backfill を報告する。
#[test]
fn preview_after_a_watermark_counts_only_the_dates_still_missing() {
    let input = input_expense(1, 2);

    // 窓を開いたまま数えると 1〜4 月の 4 件。
    let from_scratch =
        recurring_cmd::preview_for_input(&input, None, date(2026, 5, 1), 100).unwrap();
    assert_eq!(from_scratch.backfill_total, 4);

    // 3/27 まで生成済みなら、残るのは 4/27 の 1 件だけ。
    let after_watermark =
        recurring_cmd::preview_for_input(&input, Some(date(2026, 3, 27)), date(2026, 5, 1), 100)
            .unwrap();
    assert_eq!(after_watermark.backfill_total, 1);
    assert_eq!(after_watermark.backfill, vec!["2026-04-27"]);

    // today まで調べ終えたルールは backfill ゼロ。編集しても過去には遡らない。
    let caught_up =
        recurring_cmd::preview_for_input(&input, Some(date(2026, 5, 1)), date(2026, 5, 1), 100)
            .unwrap();
    assert_eq!(caught_up.backfill_total, 0);
    assert!(caught_up.backfill.is_empty());
    // upcoming は watermark ではなく today から数えるので窓を狭めても変わらない。
    assert_eq!(
        caught_up.upcoming,
        vec!["2026-05-27", "2026-06-27", "2026-07-27"]
    );
}

/// プレビューと展開が食い違えないことを保存済みルールで固定する。プレビューは
/// 「保存したら何件生まれるか」の唯一の根拠なので、ここがずれたら確認は無意味。
#[test]
fn preview_with_the_stored_watermark_matches_what_expansion_generates() {
    let conn = fresh();
    let (account_id, _, category_id) = seed(&conn);
    let rule =
        recurring_cmd::create_rule_for_conn(&conn, input_expense(account_id, category_id)).unwrap();

    // 3/1 までを展開して watermark を進める (1/27・2/27 の 2 件)。
    recurring_cmd::expand_due_recurring_for_conn(&conn, date(2026, 3, 1), NOW).unwrap();

    let stored = recurring_repo::find_by_id(&conn, rule.id).unwrap();
    let after = stored.generated_through().unwrap();
    let today = date(2026, 5, 1);

    let preview =
        recurring_cmd::preview_for_input(&input_expense(account_id, category_id), after, today, 100)
            .unwrap();
    let expanded = recurring_cmd::expand_due_recurring_for_conn(&conn, today, NOW).unwrap();

    assert_eq!(preview.backfill_total, expanded.generated);
    assert_eq!(preview.backfill_total, 2);
    assert_eq!(preview.backfill, vec!["2026-03-27", "2026-04-27"]);
}

/// 健全なルールの watermark は展開のたびに today まで進む (190fddb)。窓が
/// `(today, today]` になるので、周期や発生日を編集しても今の期間には何も生えない。
/// 編集モーダルの「もう 1 件生成されることがあります」を外す根拠がこれ。
#[test]
fn editing_the_schedule_of_a_caught_up_rule_generates_nothing_today() {
    let conn = fresh();
    let (account_id, _, category_id) = seed(&conn);
    let rule =
        recurring_cmd::create_rule_for_conn(&conn, input_expense(account_id, category_id)).unwrap();

    let today = date(2026, 5, 1);
    recurring_cmd::expand_due_recurring_for_conn(&conn, today, NOW).unwrap();
    let before = tx_count(&conn);

    // 発生日を「今日より前の日」に付け替える。過去に遡るなら 5/15 ではなく 4/15 が生える。
    recurring_cmd::update_rule_for_conn(
        &conn,
        rule.id,
        RecurringRuleInput {
            day_of_month: Some(15),
            ..input_expense(account_id, category_id)
        },
    )
    .unwrap();

    let result = recurring_cmd::expand_due_recurring_for_conn(&conn, today, NOW).unwrap();

    assert_eq!(result.generated, 0);
    assert_eq!(tx_count(&conn), before);
}

fn tx_count(conn: &Connection) -> i64 {
    conn.query_row("SELECT COUNT(*) FROM transactions", [], |r| r.get(0))
        .unwrap()
}

fn occurred_dates(conn: &Connection) -> Vec<String> {
    conn.prepare("SELECT occurred_on FROM transactions ORDER BY occurred_on")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(|r| r.unwrap())
        .collect()
}

#[test]
fn expansion_generates_every_missed_date_from_starts_on() {
    let conn = fresh();
    let (account_id, _, category_id) = seed(&conn);
    recurring_cmd::create_rule_for_conn(&conn, input_expense(account_id, category_id)).unwrap();

    let result =
        recurring_cmd::expand_due_recurring_for_conn(&conn, date(2026, 4, 1), NOW).unwrap();

    // 1/27, 2/27, 3/27 の 3 件。4/27 はまだ来ていない。
    assert_eq!(result.generated, 3);
    assert_eq!(tx_count(&conn), 3);
    assert_eq!(result.rules.len(), 1);
    assert_eq!(result.rules[0].generated, 3);
    // watermark は最後の発生日 (3/27) ではなく「どこまで調べ終えたか」= today。
    // 3/27 で止めると、あとでルールの発生日を変えたとき窓が 3/27 まで遡ってしまう。
    assert_eq!(result.rules[0].last_generated_on, "2026-04-01");
    assert!(result.skipped.is_empty());
}

#[test]
fn expanding_twice_on_the_same_day_generates_nothing_the_second_time() {
    let conn = fresh();
    let (account_id, _, category_id) = seed(&conn);
    recurring_cmd::create_rule_for_conn(&conn, input_expense(account_id, category_id)).unwrap();

    recurring_cmd::expand_due_recurring_for_conn(&conn, date(2026, 4, 1), NOW).unwrap();
    let second =
        recurring_cmd::expand_due_recurring_for_conn(&conn, date(2026, 4, 1), NOW).unwrap();

    assert_eq!(second.generated, 0);
    assert!(second.rules.is_empty());
    assert_eq!(tx_count(&conn), 3);
}

/// 1 件も生成しなかった健全なルールも watermark を today まで進める。
///
/// 進めないと watermark が時計から遅れ続け、あとでルールを編集したときに窓が
/// 締めた期間まで遡ってしまう
/// (`editing_a_rule_does_not_backfill_into_the_settled_period` が実害)。
/// 窓は `(last_generated_on, today]` で、`occurrences_between` が返す日付は
/// 必ず `today` 以下なので、today まで進めても発生日を飛ばすことはない。
#[test]
fn a_healthy_rule_that_generated_nothing_still_advances_its_watermark() {
    let conn = fresh();
    let (account_id, _, category_id) = seed(&conn);
    let rule =
        recurring_cmd::create_rule_for_conn(&conn, input_expense(account_id, category_id)).unwrap();

    // starts_on (2026-01-27) より前なので 1 件も発生しない。
    let result =
        recurring_cmd::expand_due_recurring_for_conn(&conn, date(2026, 1, 10), NOW).unwrap();

    assert_eq!(result.generated, 0);
    // 0 件のルールは「生成した内訳」には出さない (UI のバナーは実仕事だけ数える)。
    assert!(result.rules.is_empty());
    assert!(result.skipped.is_empty());

    let stored = recurring_repo::find_by_id(&conn, rule.id).unwrap();
    assert_eq!(stored.last_generated_on.as_deref(), Some("2026-01-10"));

    // watermark を進めても、これから来る 1/27 は取りこぼさない。
    let later =
        recurring_cmd::expand_due_recurring_for_conn(&conn, date(2026, 1, 31), NOW).unwrap();
    assert_eq!(later.generated, 1);
    assert_eq!(occurred_dates(&conn), vec!["2026-01-27"]);
}

/// ルールの編集は「これから先の生成」にだけ効く。
///
/// watermark が最後の発生日 (3/27) で止まっていると、4/15 に「毎月 1 日」へ
/// 付け替えた瞬間、次の窓 `(3/27, 4/15]` が 4/1 を含んでしまい、ユーザーが
/// 精算済みと考えている日に取引が生える。
#[test]
fn editing_a_rule_does_not_backfill_into_the_settled_period() {
    let conn = fresh();
    let (account_id, _, category_id) = seed(&conn);
    let rule =
        recurring_cmd::create_rule_for_conn(&conn, input_expense(account_id, category_id)).unwrap();

    // 4/1 に展開。毎月 27 日なので 1/27・2/27・3/27 の 3 件。
    recurring_cmd::expand_due_recurring_for_conn(&conn, date(2026, 4, 1), NOW).unwrap();
    assert_eq!(
        occurred_dates(&conn),
        vec!["2026-01-27", "2026-02-27", "2026-03-27"]
    );

    // 4/15 に「毎月 1 日」へ付け替える。
    recurring_cmd::update_rule_for_conn(
        &conn,
        rule.id,
        RecurringRuleInput {
            day_of_month: Some(1),
            ..input_expense(account_id, category_id)
        },
    )
    .unwrap();

    let result =
        recurring_cmd::expand_due_recurring_for_conn(&conn, date(2026, 4, 15), NOW).unwrap();

    // 4/1 は 4/1 の展開時点で「発生しない日」だった。編集がそれを蒸し返さない。
    assert_eq!(
        occurred_dates(&conn),
        vec!["2026-01-27", "2026-02-27", "2026-03-27"]
    );
    assert_eq!(result.generated, 0);

    // 次の 5/1 は新しいスケジュールどおり生成される。
    let next =
        recurring_cmd::expand_due_recurring_for_conn(&conn, date(2026, 5, 2), NOW).unwrap();
    assert_eq!(next.generated, 1);
    assert_eq!(
        occurred_dates(&conn),
        vec!["2026-01-27", "2026-02-27", "2026-03-27", "2026-05-01"]
    );
}

/// import_json は watermark を検証しないので、today より先の値を持つ行が入りうる。
/// 展開はそれを today まで引き戻してはならない (引き戻すと同じ日を二重生成する)。
#[test]
fn a_watermark_already_in_the_future_is_not_pulled_back() {
    let conn = fresh();
    let (account_id, _, category_id) = seed(&conn);
    let rule =
        recurring_cmd::create_rule_for_conn(&conn, input_expense(account_id, category_id)).unwrap();
    recurring_repo::set_last_generated_on(&conn, rule.id, "2026-12-31").unwrap();

    let result =
        recurring_cmd::expand_due_recurring_for_conn(&conn, date(2026, 4, 1), NOW).unwrap();

    assert_eq!(result.generated, 0);
    assert_eq!(tx_count(&conn), 0);
    let stored = recurring_repo::find_by_id(&conn, rule.id).unwrap();
    assert_eq!(stored.last_generated_on.as_deref(), Some("2026-12-31"));
}

#[test]
fn expanding_day_by_day_matches_one_big_catch_up() {
    let stepwise = fresh();
    let (a1, _, c1) = seed(&stepwise);
    recurring_cmd::create_rule_for_conn(&stepwise, input_expense(a1, c1)).unwrap();
    for day in 1..=90u64 {
        let today = date(2026, 1, 1)
            .checked_add_days(chrono::Days::new(day))
            .unwrap();
        recurring_cmd::expand_due_recurring_for_conn(&stepwise, today, NOW).unwrap();
    }

    let at_once = fresh();
    let (a2, _, c2) = seed(&at_once);
    recurring_cmd::create_rule_for_conn(&at_once, input_expense(a2, c2)).unwrap();
    recurring_cmd::expand_due_recurring_for_conn(
        &at_once,
        date(2026, 1, 1).checked_add_days(chrono::Days::new(90)).unwrap(),
        NOW,
    )
    .unwrap();

    let dates = |conn: &Connection| -> Vec<String> {
        conn.prepare("SELECT occurred_on FROM transactions ORDER BY occurred_on")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect()
    };
    // 両方とも空なら比較は素通りしてしまう。日々の展開が実際に 3 件書いたことを
    // 先に固定して、冪等性の主張が空振りで通らないようにする。
    assert_eq!(
        dates(&stepwise),
        vec!["2026-01-27", "2026-02-27", "2026-03-27"]
    );
    assert_eq!(dates(&stepwise), dates(&at_once));
}

/// 唯一 monthly でしか展開を確かめていなかった穴を塞ぐ。weekly は
/// `classify_skip` の周期/日付列ペアリングと `occurrences_between` の weekly
/// 分岐の両方を通るので、実コマンド経由で厳密な日付まで固定する。
///
/// `starts_on` を木曜 (2026-01-01) にしつつ `day_of_week` は月曜 (1) を指定する。
/// 曜日をわざとずらすことで、`occurrences_between` が `day_of_week` を無視して
/// `starts_on` の曜日にフォールバックする実装に戻っても、この生成日で必ず落ちる。
#[test]
fn expansion_generates_every_weekly_occurrence_on_the_chosen_weekday() {
    let conn = fresh();
    let (account_id, _, category_id) = seed(&conn);
    let weekly = RecurringRuleInput {
        frequency: "weekly".into(),
        day_of_month: None,
        day_of_week: Some(1), // 月曜 (chrono: 0=日曜..6=土曜)
        starts_on: "2026-01-01".into(), // 木曜。day_of_week とわざと不一致にする。
        ..input_expense(account_id, category_id)
    };
    recurring_cmd::create_rule_for_conn(&conn, weekly).unwrap();

    let result =
        recurring_cmd::expand_due_recurring_for_conn(&conn, date(2026, 3, 10), NOW).unwrap();

    assert_eq!(result.generated, 10);
    assert_eq!(result.rules.len(), 1);
    assert!(result.skipped.is_empty());
    let dates: Vec<String> = conn
        .prepare("SELECT occurred_on FROM transactions ORDER BY occurred_on")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    assert_eq!(
        dates,
        vec![
            "2026-01-05",
            "2026-01-12",
            "2026-01-19",
            "2026-01-26",
            "2026-02-02",
            "2026-02-09",
            "2026-02-16",
            "2026-02-23",
            "2026-03-02",
            "2026-03-09",
        ]
    );
}

/// yearly も同じ穴。`occurrences_between` の yearly 分岐は月を `starts_on` から、
/// 日を `day_of_month` から取るので、年をまたいで同じ月日にだけ発生することを固定する。
#[test]
fn expansion_generates_every_yearly_occurrence_on_the_chosen_month_and_day() {
    let conn = fresh();
    let (account_id, _, category_id) = seed(&conn);
    let yearly = RecurringRuleInput {
        frequency: "yearly".into(),
        day_of_month: Some(15),
        day_of_week: None,
        starts_on: "2024-06-15".into(),
        ..input_expense(account_id, category_id)
    };
    recurring_cmd::create_rule_for_conn(&conn, yearly).unwrap();

    // today = 2027-01-01。2027-06-15 はまだ来ていないので 3 件 (2024〜2026)。
    let result =
        recurring_cmd::expand_due_recurring_for_conn(&conn, date(2027, 1, 1), NOW).unwrap();

    assert_eq!(result.generated, 3);
    assert_eq!(result.rules.len(), 1);
    assert!(result.skipped.is_empty());
    let dates: Vec<String> = conn
        .prepare("SELECT occurred_on FROM transactions ORDER BY occurred_on")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    assert_eq!(dates, vec!["2024-06-15", "2025-06-15", "2026-06-15"]);
}

/// 2 口座が異なる正常な振替ルールも、既存テストは件数と type しか見ていなかった。
/// `classify_skip` の transfer 分岐 (counter_account_id 必須・別口座・category_id
/// なし) を実コマンド経由で通し、生成日まで固定する。
#[test]
fn expansion_generates_a_valid_transfer_rules_occurrences() {
    let conn = fresh();
    let (account_id, counter_account_id, _) = seed(&conn);
    recurring_cmd::create_rule_for_conn(
        &conn,
        RecurringRuleInput {
            name: "貯金".into(),
            type_: "transfer".into(),
            amount: 30_000,
            account_id,
            counter_account_id: Some(counter_account_id),
            category_id: None,
            description: String::new(),
            frequency: "monthly".into(),
            day_of_month: Some(25),
            day_of_week: None,
            starts_on: "2026-01-25".into(),
            ends_on: None,
        },
    )
    .unwrap();

    let result =
        recurring_cmd::expand_due_recurring_for_conn(&conn, date(2026, 4, 1), NOW).unwrap();

    assert_eq!(result.generated, 3);
    assert_eq!(result.rules.len(), 1);
    assert!(result.skipped.is_empty());
    let dates: Vec<String> = conn
        .prepare("SELECT occurred_on FROM transactions ORDER BY occurred_on")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    assert_eq!(dates, vec!["2026-01-25", "2026-02-25", "2026-03-25"]);

    let type_: String = conn
        .query_row("SELECT type FROM transactions LIMIT 1", [], |r| r.get(0))
        .unwrap();
    assert_eq!(type_, "transfer");
}

#[test]
fn an_archived_account_skips_only_that_rule_and_holds_its_watermark() {
    let conn = fresh();
    let (account_id, _, category_id) = seed(&conn);
    let rule =
        recurring_cmd::create_rule_for_conn(&conn, input_expense(account_id, category_id)).unwrap();
    conn.execute(
        "UPDATE accounts SET archived_at = ?1 WHERE id = ?2",
        params![NOW, account_id],
    )
    .unwrap();

    let result =
        recurring_cmd::expand_due_recurring_for_conn(&conn, date(2026, 4, 1), NOW).unwrap();

    assert_eq!(result.generated, 0);
    assert_eq!(tx_count(&conn), 0);
    assert_eq!(result.skipped.len(), 1);
    assert_eq!(result.skipped[0].rule_id, rule.id);
    // 見送った期間は次回に持ち越す。
    let stored = recurring_repo::find_by_id(&conn, rule.id).unwrap();
    assert_eq!(stored.last_generated_on, None);
}

#[test]
fn unarchiving_lets_the_next_expansion_backfill_the_held_period() {
    let conn = fresh();
    let (account_id, _, category_id) = seed(&conn);
    recurring_cmd::create_rule_for_conn(&conn, input_expense(account_id, category_id)).unwrap();
    conn.execute(
        "UPDATE accounts SET archived_at = ?1 WHERE id = ?2",
        params![NOW, account_id],
    )
    .unwrap();
    recurring_cmd::expand_due_recurring_for_conn(&conn, date(2026, 4, 1), NOW).unwrap();

    conn.execute(
        "UPDATE accounts SET archived_at = NULL WHERE id = ?1",
        params![account_id],
    )
    .unwrap();
    let result =
        recurring_cmd::expand_due_recurring_for_conn(&conn, date(2026, 4, 1), NOW).unwrap();

    assert_eq!(result.generated, 3);
    assert_eq!(tx_count(&conn), 3);
}

#[test]
fn an_archived_category_skips_the_rule() {
    let conn = fresh();
    let (account_id, _, category_id) = seed(&conn);
    recurring_cmd::create_rule_for_conn(&conn, input_expense(account_id, category_id)).unwrap();
    conn.execute(
        "UPDATE categories SET archived_at = ?1 WHERE id = ?2",
        params![NOW, category_id],
    )
    .unwrap();

    let result =
        recurring_cmd::expand_due_recurring_for_conn(&conn, date(2026, 4, 1), NOW).unwrap();

    assert_eq!(result.generated, 0);
    assert_eq!(result.skipped.len(), 1);
}

/// 停止は spec の論理削除 (`active = 0`)。再開したときに、停止していた期間の
/// 家賃をまとめて後付けで課金してはならない。
#[test]
fn resuming_a_paused_rule_does_not_backfill_the_paused_months() {
    let conn = fresh();
    let (account_id, _, category_id) = seed(&conn);
    let rule =
        recurring_cmd::create_rule_for_conn(&conn, input_expense(account_id, category_id)).unwrap();

    // 1〜2 月分を展開して watermark を 2026-02-27 にする。
    recurring_cmd::expand_due_recurring_for_conn(&conn, date(2026, 2, 28), NOW).unwrap();
    assert_eq!(tx_count(&conn), 2);

    // 3 月頭に停止し、4 か月後に再開する。
    recurring_repo::set_active(&conn, rule.id, false, "2026-03-01").unwrap();
    recurring_repo::set_active(&conn, rule.id, true, "2026-07-15").unwrap();

    let result =
        recurring_cmd::expand_due_recurring_for_conn(&conn, date(2026, 7, 15), NOW).unwrap();

    // 停止中に飛ばした 3/27・4/27・5/27・6/27 は生成されない。
    assert_eq!(result.generated, 0);
    assert_eq!(tx_count(&conn), 2);
    // 再開時点で watermark は今日まで進んでいる。
    let stored = recurring_repo::find_by_id(&conn, rule.id).unwrap();
    assert_eq!(stored.last_generated_on.as_deref(), Some("2026-07-15"));
}

/// 一度も生成していないルールは preview が約束した starts_on からの backfill を
/// 保つ。停止→再開が「保存直後の生成」を取り消してはならない。
#[test]
fn resuming_a_rule_that_never_generated_still_backfills_from_starts_on() {
    let conn = fresh();
    let (account_id, _, category_id) = seed(&conn);
    let rule =
        recurring_cmd::create_rule_for_conn(&conn, input_expense(account_id, category_id)).unwrap();

    recurring_repo::set_active(&conn, rule.id, false, "2026-02-01").unwrap();
    recurring_repo::set_active(&conn, rule.id, true, "2026-04-01").unwrap();

    let stored = recurring_repo::find_by_id(&conn, rule.id).unwrap();
    assert_eq!(stored.last_generated_on, None);

    let result =
        recurring_cmd::expand_due_recurring_for_conn(&conn, date(2026, 4, 1), NOW).unwrap();

    assert_eq!(result.generated, 3);
    assert_eq!(tx_count(&conn), 3);
}

/// 停止そのものは watermark を動かさない。停止中に日付が進んでも、再開するまでは
/// 「どこまで生成したか」の記録がずれない。
#[test]
fn pausing_a_rule_leaves_the_watermark_untouched() {
    let conn = fresh();
    let (account_id, _, category_id) = seed(&conn);
    let rule =
        recurring_cmd::create_rule_for_conn(&conn, input_expense(account_id, category_id)).unwrap();
    recurring_cmd::expand_due_recurring_for_conn(&conn, date(2026, 2, 28), NOW).unwrap();

    recurring_repo::set_active(&conn, rule.id, false, "2026-06-01").unwrap();

    let stored = recurring_repo::find_by_id(&conn, rule.id).unwrap();
    assert!(!stored.active);
    // 2/28 の展開が watermark を today (2/28) まで進めている。停止はそこから
    // 一切動かさない。6/1 に引きずられないことがこのテストの主張。
    assert_eq!(stored.last_generated_on.as_deref(), Some("2026-02-28"));
}

#[test]
fn an_inactive_rule_is_not_expanded_at_all() {
    let conn = fresh();
    let (account_id, _, category_id) = seed(&conn);
    let rule =
        recurring_cmd::create_rule_for_conn(&conn, input_expense(account_id, category_id)).unwrap();
    recurring_repo::set_active(&conn, rule.id, false, "2026-05-25").unwrap();

    let result =
        recurring_cmd::expand_due_recurring_for_conn(&conn, date(2026, 4, 1), NOW).unwrap();

    assert_eq!(result.generated, 0);
    assert!(result.rules.is_empty());
    // 停止中は「要修正」ではないので警告にも出さない。
    assert!(result.skipped.is_empty());
}

#[test]
fn one_broken_rule_does_not_block_a_healthy_one() {
    let conn = fresh();
    let (account_id, counter_account_id, category_id) = seed(&conn);
    recurring_cmd::create_rule_for_conn(&conn, input_expense(account_id, category_id)).unwrap();
    recurring_cmd::create_rule_for_conn(
        &conn,
        RecurringRuleInput {
            name: "貯金".into(),
            type_: "transfer".into(),
            amount: 30_000,
            account_id,
            counter_account_id: Some(counter_account_id),
            category_id: None,
            description: String::new(),
            frequency: "monthly".into(),
            day_of_month: Some(25),
            day_of_week: None,
            starts_on: "2026-01-25".into(),
            ends_on: None,
        },
    )
    .unwrap();
    conn.execute(
        "UPDATE accounts SET archived_at = ?1 WHERE id = ?2",
        params![NOW, counter_account_id],
    )
    .unwrap();

    let result =
        recurring_cmd::expand_due_recurring_for_conn(&conn, date(2026, 4, 1), NOW).unwrap();

    assert_eq!(result.generated, 3);
    assert_eq!(result.rules.len(), 1);
    assert_eq!(result.skipped.len(), 1);
}

#[test]
fn generated_transfers_stay_out_of_the_income_expense_totals() {
    let conn = fresh();
    let (account_id, counter_account_id, _) = seed(&conn);
    recurring_cmd::create_rule_for_conn(
        &conn,
        RecurringRuleInput {
            name: "貯金".into(),
            type_: "transfer".into(),
            amount: 30_000,
            account_id,
            counter_account_id: Some(counter_account_id),
            category_id: None,
            description: String::new(),
            frequency: "monthly".into(),
            day_of_month: Some(25),
            day_of_week: None,
            starts_on: "2026-01-25".into(),
            ends_on: None,
        },
    )
    .unwrap();

    recurring_cmd::expand_due_recurring_for_conn(&conn, date(2026, 2, 1), NOW).unwrap();

    let counted: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM transactions WHERE type IN ('income','expense')",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(counted, 0);
    assert_eq!(tx_count(&conn), 1);
}

#[test]
fn a_rule_with_a_broken_type_and_category_pairing_is_skipped_not_fatal() {
    let conn = fresh();
    let (account_id, _, category_id) = seed(&conn);
    recurring_cmd::create_rule_for_conn(&conn, input_expense(account_id, category_id)).unwrap();

    // import_json のように、コマンド層をバイパスして直接 INSERT する。
    // type='expense' なのに category_id が NULL は transactions の CHECK に反する組み合わせ。
    conn.execute(
        "INSERT INTO recurring_rules(name, type, amount, account_id, counter_account_id,
                                     category_id, description, frequency, day_of_month,
                                     day_of_week, starts_on, ends_on, last_generated_on, active)
         VALUES ('壊れたルール', 'expense', 1000, ?1, NULL, NULL, '', 'monthly', 27, NULL,
                 '2026-01-27', NULL, NULL, 1)",
        params![account_id],
    )
    .unwrap();
    let broken_id = conn.last_insert_rowid();

    let result =
        recurring_cmd::expand_due_recurring_for_conn(&conn, date(2026, 4, 1), NOW).unwrap();

    assert_eq!(result.generated, 3);
    assert_eq!(tx_count(&conn), 3);
    assert_eq!(result.rules.len(), 1);
    assert_eq!(result.skipped.len(), 1);
    assert_eq!(result.skipped[0].rule_id, broken_id);
    assert!(matches!(
        result.skipped[0].reason,
        recurring_cmd::SkipReason::MalformedRule
    ));

    let stored = recurring_repo::find_by_id(&conn, broken_id).unwrap();
    assert_eq!(stored.last_generated_on, None);
}

#[test]
fn a_rule_pointing_at_a_missing_account_is_skipped_not_fatal() {
    let conn = fresh();
    let (account_id, _, category_id) = seed(&conn);
    recurring_cmd::create_rule_for_conn(&conn, input_expense(account_id, category_id)).unwrap();

    // 実運用では FK が ON なのでこの状態は起こらないが、破損データ (import_json 由来の
    // 孤立行など) を想定して、この 1 件の INSERT だけ FK を切って作る。
    conn.pragma_update(None, "foreign_keys", "OFF").unwrap();
    conn.execute(
        "INSERT INTO recurring_rules(name, type, amount, account_id, counter_account_id,
                                     category_id, description, frequency, day_of_month,
                                     day_of_week, starts_on, ends_on, last_generated_on, active)
         VALUES ('存在しない口座', 'expense', 1000, 9999, NULL, ?1, '', 'monthly', 27, NULL,
                 '2026-01-27', NULL, NULL, 1)",
        params![category_id],
    )
    .unwrap();
    let broken_id = conn.last_insert_rowid();

    let result =
        recurring_cmd::expand_due_recurring_for_conn(&conn, date(2026, 4, 1), NOW).unwrap();

    assert_eq!(result.generated, 3);
    assert_eq!(tx_count(&conn), 3);
    assert_eq!(result.rules.len(), 1);
    assert_eq!(result.skipped.len(), 1);
    assert_eq!(result.skipped[0].rule_id, broken_id);
    assert!(matches!(
        result.skipped[0].reason,
        recurring_cmd::SkipReason::MalformedRule
    ));
}

#[test]
fn a_stored_self_transfer_rule_is_skipped_not_generated() {
    let conn = fresh();
    let (account_id, _, category_id) = seed(&conn);
    recurring_cmd::create_rule_for_conn(&conn, input_expense(account_id, category_id)).unwrap();

    // import_json のように、コマンド層をバイパスして直接 INSERT する。
    // 同じ口座への振替は validate_rule_input が弾くが、transactions の CHECK は
    // counter_account_id が NULL でないことしか見ないので、生成は素通りしてしまう。
    conn.execute(
        "INSERT INTO recurring_rules(name, type, amount, account_id, counter_account_id,
                                     category_id, description, frequency, day_of_month,
                                     day_of_week, starts_on, ends_on, last_generated_on, active)
         VALUES ('自分への振替', 'transfer', 1000, ?1, ?1, NULL, '', 'monthly', 27, NULL,
                 '2026-01-27', NULL, NULL, 1)",
        params![account_id],
    )
    .unwrap();
    let broken_id = conn.last_insert_rowid();

    let result =
        recurring_cmd::expand_due_recurring_for_conn(&conn, date(2026, 4, 1), NOW).unwrap();

    assert_eq!(result.generated, 3);
    assert_eq!(tx_count(&conn), 3);
    assert_eq!(result.rules.len(), 1);
    assert_eq!(result.skipped.len(), 1);
    assert_eq!(result.skipped[0].rule_id, broken_id);
    assert!(matches!(
        result.skipped[0].reason,
        recurring_cmd::SkipReason::MalformedRule
    ));

    // 直すまで見送り続ける。watermark も進めない。
    let stored = recurring_repo::find_by_id(&conn, broken_id).unwrap();
    assert_eq!(stored.last_generated_on, None);
}

#[test]
fn a_stored_weekly_rule_without_a_weekday_is_skipped_not_generated() {
    let conn = fresh();
    let (account_id, _, category_id) = seed(&conn);
    recurring_cmd::create_rule_for_conn(&conn, input_expense(account_id, category_id)).unwrap();

    // frequency='weekly' なのに day_of_week が NULL。occurrences_between は
    // starts_on の曜日で補うので、ユーザーが選んでいない曜日で生成されてしまう。
    conn.execute(
        "INSERT INTO recurring_rules(name, type, amount, account_id, counter_account_id,
                                     category_id, description, frequency, day_of_month,
                                     day_of_week, starts_on, ends_on, last_generated_on, active)
         VALUES ('曜日の無い毎週', 'expense', 1000, ?1, NULL, ?2, '', 'weekly', NULL, NULL,
                 '2026-01-27', NULL, NULL, 1)",
        params![account_id, category_id],
    )
    .unwrap();
    let broken_id = conn.last_insert_rowid();

    let result =
        recurring_cmd::expand_due_recurring_for_conn(&conn, date(2026, 4, 1), NOW).unwrap();

    assert_eq!(result.generated, 3);
    assert_eq!(tx_count(&conn), 3);
    assert_eq!(result.rules.len(), 1);
    assert_eq!(result.skipped.len(), 1);
    assert_eq!(result.skipped[0].rule_id, broken_id);
    assert!(matches!(
        result.skipped[0].reason,
        recurring_cmd::SkipReason::MalformedRule
    ));

    let stored = recurring_repo::find_by_id(&conn, broken_id).unwrap();
    assert_eq!(stored.last_generated_on, None);
}

#[test]
fn a_stored_monthly_rule_carrying_a_weekday_is_skipped_not_generated() {
    let conn = fresh();
    let (account_id, _, category_id) = seed(&conn);
    recurring_cmd::create_rule_for_conn(&conn, input_expense(account_id, category_id)).unwrap();

    // monthly なのに day_of_week も持っている。どちらを守るべきか決められない。
    conn.execute(
        "INSERT INTO recurring_rules(name, type, amount, account_id, counter_account_id,
                                     category_id, description, frequency, day_of_month,
                                     day_of_week, starts_on, ends_on, last_generated_on, active)
         VALUES ('曜日つきの毎月', 'expense', 1000, ?1, NULL, ?2, '', 'monthly', 27, 3,
                 '2026-01-27', NULL, NULL, 1)",
        params![account_id, category_id],
    )
    .unwrap();
    let broken_id = conn.last_insert_rowid();

    let result =
        recurring_cmd::expand_due_recurring_for_conn(&conn, date(2026, 4, 1), NOW).unwrap();

    assert_eq!(result.generated, 3);
    assert_eq!(result.skipped.len(), 1);
    assert_eq!(result.skipped[0].rule_id, broken_id);
    assert!(matches!(
        result.skipped[0].reason,
        recurring_cmd::SkipReason::MalformedRule
    ));
}
