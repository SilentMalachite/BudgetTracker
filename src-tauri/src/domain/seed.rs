use rusqlite::Connection;

use crate::domain::category::CategoryType;
use crate::error::AppResult;
use crate::infra::repo::{category_repo, meta_repo};

const SEED_FLAG_KEY: &str = "categories_seeded";

const EXPENSE_DEFAULTS: &[&str] = &[
    "食費",
    "日用品",
    "交通費",
    "住居費",
    "水道光熱費",
    "通信費",
    "医療費",
    "衣服",
    "交際費",
    "趣味・娯楽",
    "その他",
];

const INCOME_DEFAULTS: &[&str] = &["給与", "賞与", "副業", "その他"];

/// Seed default categories iff the flag is unset and the table is empty.
/// Always sets the flag afterwards to prevent re-seeding after the user
/// deletes everything intentionally.
pub fn seed_default_categories_if_needed(conn: &mut Connection) -> AppResult<bool> {
    if meta_repo::get(conn, SEED_FLAG_KEY)?.is_some() {
        return Ok(false);
    }
    let existing = category_repo::list(
        conn,
        &category_repo::ListFilter {
            include_archived: true,
            ..Default::default()
        },
    )?;
    if !existing.is_empty() {
        meta_repo::set(conn, SEED_FLAG_KEY, "true")?;
        return Ok(false);
    }

    let tx = conn.transaction()?;
    for (order, name) in EXPENSE_DEFAULTS.iter().enumerate() {
        category_repo::insert(
            &tx,
            &category_repo::InsertInput {
                name,
                type_: CategoryType::Expense,
                color: None,
                icon: None,
                display_order: order as i64,
            },
        )?;
    }
    for (order, name) in INCOME_DEFAULTS.iter().enumerate() {
        category_repo::insert(
            &tx,
            &category_repo::InsertInput {
                name,
                type_: CategoryType::Income,
                color: None,
                icon: None,
                display_order: order as i64,
            },
        )?;
    }
    meta_repo::set(&tx, SEED_FLAG_KEY, "true")?;
    tx.commit()?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infra::migrations;

    fn fresh() -> Connection {
        let mut c = Connection::open_in_memory().unwrap();
        migrations::run(&mut c).unwrap();
        c
    }

    #[test]
    fn seeds_when_table_empty_and_flag_unset() {
        let mut conn = fresh();
        let seeded = seed_default_categories_if_needed(&mut conn).unwrap();
        assert!(seeded);
        let cats = category_repo::list(&conn, &category_repo::ListFilter::default()).unwrap();
        assert_eq!(cats.len(), EXPENSE_DEFAULTS.len() + INCOME_DEFAULTS.len());
    }

    #[test]
    fn does_not_seed_twice_after_user_all_delete_simulation() {
        let mut conn = fresh();
        assert!(seed_default_categories_if_needed(&mut conn).unwrap());
        // Test-only simulation of a user-managed "delete all categories" state.
        // Production category removal must stay logical via archived_at.
        conn.execute_batch("DELETE FROM categories;").unwrap();
        assert!(!seed_default_categories_if_needed(&mut conn).unwrap());
        let cats = category_repo::list(&conn, &category_repo::ListFilter::default()).unwrap();
        assert!(cats.is_empty());
    }

    #[test]
    fn does_not_seed_if_user_already_has_categories() {
        let mut conn = fresh();
        category_repo::insert(
            &conn,
            &category_repo::InsertInput {
                name: "自分カテゴリ",
                type_: CategoryType::Expense,
                color: None,
                icon: None,
                display_order: 0,
            },
        )
        .unwrap();
        let seeded = seed_default_categories_if_needed(&mut conn).unwrap();
        assert!(!seeded);
        let cats = category_repo::list(&conn, &category_repo::ListFilter::default()).unwrap();
        assert_eq!(cats.len(), 1);
    }
}
