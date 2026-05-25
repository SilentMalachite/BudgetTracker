use budget_tracker_lib::domain::account::AccountKind;
use budget_tracker_lib::infra::migrations;
use budget_tracker_lib::infra::repo::account_repo;
use rusqlite::Connection;

const NOW: &str = "2026-05-25T00:00:00+00:00";

fn fresh() -> Connection {
    let mut c = Connection::open_in_memory().unwrap();
    migrations::run(&mut c).unwrap();
    c
}

#[test]
fn insert_then_find_returns_inserted_row() {
    let conn = fresh();
    let id = account_repo::insert(
        &conn,
        &account_repo::InsertInput {
            name: "三井住友銀行",
            kind: AccountKind::Bank,
            currency: "JPY",
            initial_balance: 100_000,
            display_order: 0,
            note: "メイン口座",
            now: NOW,
        },
    )
    .unwrap();
    let acc = account_repo::find_by_id(&conn, id).unwrap();
    assert_eq!(acc.name, "三井住友銀行");
    assert_eq!(acc.kind, AccountKind::Bank);
    assert_eq!(acc.initial_balance, 100_000);
    assert_eq!(acc.note, "メイン口座");
    assert_eq!(acc.created_at, NOW);
    assert_eq!(acc.updated_at, NOW);
}

#[test]
fn list_excludes_archived_by_default() {
    let conn = fresh();
    let id = account_repo::insert(
        &conn,
        &account_repo::InsertInput {
            name: "旧口座",
            kind: AccountKind::Cash,
            currency: "JPY",
            initial_balance: 0,
            display_order: 0,
            note: "",
            now: NOW,
        },
    )
    .unwrap();
    account_repo::set_archived(&conn, id, Some(NOW), NOW).unwrap();
    assert!(account_repo::list(&conn, false).unwrap().is_empty());
    assert_eq!(account_repo::list(&conn, true).unwrap().len(), 1);
}

#[test]
fn update_changes_updated_at() {
    let conn = fresh();
    let id = account_repo::insert(
        &conn,
        &account_repo::InsertInput {
            name: "PayPay",
            kind: AccountKind::EMoney,
            currency: "JPY",
            initial_balance: 0,
            display_order: 0,
            note: "",
            now: NOW,
        },
    )
    .unwrap();
    let later = "2026-06-01T00:00:00+00:00";
    account_repo::update(
        &conn,
        id,
        &account_repo::UpdatePatch {
            note: Some("チャージ用"),
            ..Default::default()
        },
        later,
    )
    .unwrap();
    let acc = account_repo::find_by_id(&conn, id).unwrap();
    assert_eq!(acc.note, "チャージ用");
    assert_eq!(acc.updated_at, later);
    assert_eq!(acc.created_at, NOW);
}

#[test]
fn next_display_order_starts_at_zero_and_increments() {
    let conn = fresh();
    assert_eq!(account_repo::next_display_order(&conn).unwrap(), 0);
    account_repo::insert(
        &conn,
        &account_repo::InsertInput {
            name: "A",
            kind: AccountKind::Cash,
            currency: "JPY",
            initial_balance: 0,
            display_order: 0,
            note: "",
            now: NOW,
        },
    )
    .unwrap();
    assert_eq!(account_repo::next_display_order(&conn).unwrap(), 1);
}
