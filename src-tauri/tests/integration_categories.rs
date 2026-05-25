use budget_tracker_lib::domain::category::CategoryType;
use budget_tracker_lib::infra::migrations;
use budget_tracker_lib::infra::repo::category_repo;
use rusqlite::Connection;

fn fresh_db() -> Connection {
    let mut conn = Connection::open_in_memory().unwrap();
    migrations::run(&mut conn).unwrap();
    conn
}

#[test]
fn insert_then_list_returns_inserted_row() {
    let conn = fresh_db();
    let id = category_repo::insert(
        &conn,
        &category_repo::InsertInput {
            name: "食費",
            type_: CategoryType::Expense,
            color: Some("#FF00AA"),
            icon: None,
            display_order: 0,
        },
    )
    .unwrap();
    let listed = category_repo::list(&conn, &category_repo::ListFilter::default()).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, id);
    assert_eq!(listed[0].name, "食費");
    assert_eq!(listed[0].type_, CategoryType::Expense);
    assert_eq!(listed[0].color.as_deref(), Some("#FF00AA"));
}

#[test]
fn duplicate_name_same_type_returns_conflict() {
    let conn = fresh_db();
    category_repo::insert(
        &conn,
        &category_repo::InsertInput {
            name: "食費",
            type_: CategoryType::Expense,
            color: None,
            icon: None,
            display_order: 0,
        },
    )
    .unwrap();
    let err = category_repo::insert(
        &conn,
        &category_repo::InsertInput {
            name: "食費",
            type_: CategoryType::Expense,
            color: None,
            icon: None,
            display_order: 1,
        },
    )
    .unwrap_err();
    assert!(matches!(
        err,
        budget_tracker_lib::error::AppError::Conflict(_)
    ));
}

#[test]
fn same_name_different_type_is_allowed() {
    let conn = fresh_db();
    category_repo::insert(
        &conn,
        &category_repo::InsertInput {
            name: "その他",
            type_: CategoryType::Expense,
            color: None,
            icon: None,
            display_order: 0,
        },
    )
    .unwrap();
    category_repo::insert(
        &conn,
        &category_repo::InsertInput {
            name: "その他",
            type_: CategoryType::Income,
            color: None,
            icon: None,
            display_order: 0,
        },
    )
    .unwrap();
    let all = category_repo::list(&conn, &category_repo::ListFilter::default()).unwrap();
    assert_eq!(all.len(), 2);
}

#[test]
fn archive_filters_from_default_list() {
    let conn = fresh_db();
    let id = category_repo::insert(
        &conn,
        &category_repo::InsertInput {
            name: "副業",
            type_: CategoryType::Income,
            color: None,
            icon: None,
            display_order: 0,
        },
    )
    .unwrap();
    category_repo::set_archived(&conn, id, Some("2026-05-25T00:00:00Z")).unwrap();

    let visible = category_repo::list(&conn, &category_repo::ListFilter::default()).unwrap();
    assert!(visible.is_empty());

    let all = category_repo::list(
        &conn,
        &category_repo::ListFilter {
            include_archived: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(all.len(), 1);
}

#[test]
fn filter_by_type() {
    let conn = fresh_db();
    for (name, t) in [
        ("給与", CategoryType::Income),
        ("食費", CategoryType::Expense),
    ] {
        category_repo::insert(
            &conn,
            &category_repo::InsertInput {
                name,
                type_: t,
                color: None,
                icon: None,
                display_order: 0,
            },
        )
        .unwrap();
    }
    let only_income = category_repo::list(
        &conn,
        &category_repo::ListFilter {
            type_: Some(CategoryType::Income),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(only_income.len(), 1);
    assert_eq!(only_income[0].name, "給与");
}
