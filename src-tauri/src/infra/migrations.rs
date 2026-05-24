use std::cmp::Ordering;

use include_dir::{Dir, include_dir};
use rusqlite::{Connection, params};

use crate::error::{AppError, AppResult};

static MIGRATIONS_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/migrations");

struct Migration {
    version: u32,
    name: String,
    sql: String,
}

fn load_migrations() -> AppResult<Vec<Migration>> {
    let mut out = Vec::new();
    for file in MIGRATIONS_DIR.files() {
        let stem = file
            .path()
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| AppError::Migration(format!("bad filename: {:?}", file.path())))?;
        // Expect: V<NNN>__<desc>
        let rest = stem
            .strip_prefix('V')
            .ok_or_else(|| AppError::Migration(format!("filename must start with V: {stem}")))?;
        let (num, desc) = rest
            .split_once("__")
            .ok_or_else(|| AppError::Migration(format!("filename must contain __: {stem}")))?;
        let version: u32 = num
            .parse()
            .map_err(|_| AppError::Migration(format!("bad version in {stem}")))?;
        let sql = file
            .contents_utf8()
            .ok_or_else(|| AppError::Migration(format!("non-utf8 sql in {stem}")))?
            .to_string();
        out.push(Migration {
            version,
            name: desc.to_string(),
            sql,
        });
    }
    out.sort_by(|a, b| match a.version.cmp(&b.version) {
        Ordering::Equal => a.name.cmp(&b.name),
        other => other,
    });
    // Detect duplicates.
    for pair in out.windows(2) {
        if pair[0].version == pair[1].version {
            return Err(AppError::Migration(format!(
                "duplicate migration version: V{:03}",
                pair[0].version
            )));
        }
    }
    Ok(out)
}

fn current_version(conn: &Connection) -> AppResult<u32> {
    // app_meta may not exist yet on a fresh DB — that is version 0.
    let table_exists: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='app_meta'",
            [],
            |r| r.get::<_, i64>(0),
        )
        .map(|n| n > 0)?;
    if !table_exists {
        return Ok(0);
    }
    let v: Option<String> = conn
        .query_row(
            "SELECT value FROM app_meta WHERE key = 'schema_version'",
            [],
            |r| r.get(0),
        )
        .map(Some)
        .or_else(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => Ok::<Option<String>, rusqlite::Error>(None),
            other => Err(other),
        })?;
    Ok(v.as_deref()
        .map(str::parse)
        .transpose()
        .map_err(|_| AppError::Migration("schema_version is not an integer".into()))?
        .unwrap_or(0))
}

fn set_version(conn: &Connection, version: u32) -> AppResult<()> {
    conn.execute(
        "INSERT INTO app_meta(key, value) VALUES('schema_version', ?1)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![version.to_string()],
    )?;
    Ok(())
}

/// Apply all pending migrations against `conn`. Returns the new schema_version.
pub fn run(conn: &mut Connection) -> AppResult<u32> {
    let mut version = current_version(conn)?;
    let migrations = load_migrations()?;
    for m in &migrations {
        if m.version <= version {
            continue;
        }
        let tx = conn.transaction()?;
        tx.execute_batch(&m.sql)
            .map_err(|e| AppError::Migration(format!("V{:03} ({}) failed: {e}", m.version, m.name)))?;
        tx.execute(
            "INSERT INTO app_meta(key, value) VALUES('schema_version', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![m.version.to_string()],
        )?;
        tx.commit()?;
        version = m.version;
    }
    set_version(conn, version)?;
    Ok(version)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh() -> Connection {
        Connection::open_in_memory().unwrap()
    }

    #[test]
    fn applies_v001_to_empty_db() {
        let mut conn = fresh();
        let v = run(&mut conn).unwrap();
        assert_eq!(v, 1);
        let names: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .unwrap()
            .query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        for expected in [
            "accounts",
            "app_meta",
            "budgets",
            "categories",
            "recurring_rules",
            "transactions",
        ] {
            assert!(names.iter().any(|n| n == expected), "missing table {expected}");
        }
    }

    #[test]
    fn is_idempotent_on_second_run() {
        let mut conn = fresh();
        let v1 = run(&mut conn).unwrap();
        let v2 = run(&mut conn).unwrap();
        assert_eq!(v1, v2);
        assert_eq!(v2, 1);
    }

    #[test]
    fn enforces_transfer_check_constraint() {
        let mut conn = fresh();
        run(&mut conn).unwrap();
        conn.execute_batch(
            "INSERT INTO accounts(name, kind, created_at, updated_at)
               VALUES('cash','cash','2026-01-01','2026-01-01');
             INSERT INTO categories(name, type) VALUES('Food','expense');",
        )
        .unwrap();
        // transfer with category_id should fail.
        let err = conn.execute(
            "INSERT INTO transactions(occurred_on, type, amount, account_id, counter_account_id, category_id, created_at, updated_at)
              VALUES('2026-01-15','transfer',1000,1,1,1,'2026-01-15','2026-01-15')",
            [],
        );
        assert!(err.is_err(), "transfer with category_id must be rejected");
    }
}
