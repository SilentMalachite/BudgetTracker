use std::path::{Path, PathBuf};

use rusqlite::Connection;
use serde::Serialize;

use crate::domain::seed;
use crate::error::{AppError, AppResult};
use crate::infra::keychain::DbKey;
use crate::infra::{db, migrations};

pub const KEYCHAIN_SERVICE: &str = "jp.budget-tracker";
pub const KEYCHAIN_ACCOUNT: &str = "db_key";
pub const DB_FILENAME: &str = "data.db";

pub trait KeyStore: Send + Sync {
    fn get(&self) -> AppResult<Option<DbKey>>;
    fn create(&self) -> AppResult<DbKey>;
    fn delete(&self) -> AppResult<()>;
    /// Park the current, undecodable secret under a stamped sibling entry so
    /// that `delete` never destroys the only copy. Returns that entry's name.
    fn preserve_corrupt(&self) -> AppResult<Option<String>>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryReason {
    KeyMissing,
    DecryptFailed,
    KeyCorrupt,
    KeychainError,
}

pub enum BootOutcome {
    Ready {
        conn: Connection,
        db_path: PathBuf,
    },
    Recovery {
        reason: RecoveryReason,
        data_dir: PathBuf,
        db_path: PathBuf,
    },
}

fn open_and_prepare(db_path: &Path, key: &DbKey) -> AppResult<Connection> {
    let mut conn = db::open_encrypted(db_path, key)?;
    migrations::run(&mut conn)?;
    seed::seed_default_categories_if_needed(&mut conn)?;
    Ok(conn)
}

pub fn boot(data_dir: &Path, keys: &dyn KeyStore) -> AppResult<BootOutcome> {
    std::fs::create_dir_all(data_dir)?;
    let db_path = data_dir.join(DB_FILENAME);
    let db_exists = db_path.exists();

    let key = match keys.get() {
        Ok(v) => v,
        Err(AppError::Corrupt(_)) => {
            return Ok(BootOutcome::Recovery {
                reason: RecoveryReason::KeyCorrupt,
                data_dir: data_dir.to_path_buf(),
                db_path,
            });
        }
        Err(_) => {
            return Ok(BootOutcome::Recovery {
                reason: RecoveryReason::KeychainError,
                data_dir: data_dir.to_path_buf(),
                db_path,
            });
        }
    };

    match (db_exists, key) {
        (false, None) => {
            let key = keys.create()?;
            let conn = open_and_prepare(&db_path, &key)?;
            Ok(BootOutcome::Ready { conn, db_path })
        }
        (false, Some(key)) => {
            let conn = open_and_prepare(&db_path, &key)?;
            Ok(BootOutcome::Ready { conn, db_path })
        }
        (true, None) => Ok(BootOutcome::Recovery {
            reason: RecoveryReason::KeyMissing,
            data_dir: data_dir.to_path_buf(),
            db_path,
        }),
        (true, Some(key)) => match db::open_encrypted(&db_path, &key) {
            Ok(mut conn) => {
                // Readable DB: migration/seed failures must propagate, not
                // become Recovery (that would let recover_* quarantine it).
                migrations::run(&mut conn)?;
                seed::seed_default_categories_if_needed(&mut conn)?;
                Ok(BootOutcome::Ready { conn, db_path })
            }
            Err(err) if db::looks_like_decrypt_failure(&err) => Ok(BootOutcome::Recovery {
                reason: RecoveryReason::DecryptFailed,
                data_dir: data_dir.to_path_buf(),
                db_path,
            }),
            Err(err) => Err(err),
        },
    }
}

pub struct OsKeyStore {
    pub service: String,
    pub account: String,
}

impl KeyStore for OsKeyStore {
    fn get(&self) -> AppResult<Option<DbKey>> {
        crate::infra::keychain::get_key(&self.service, &self.account)
    }
    fn create(&self) -> AppResult<DbKey> {
        crate::infra::keychain::create_key(&self.service, &self.account)
    }
    fn delete(&self) -> AppResult<()> {
        crate::infra::keychain::delete_key(&self.service, &self.account)
    }
    fn preserve_corrupt(&self) -> AppResult<Option<String>> {
        crate::infra::keychain::preserve_corrupt_key(
            &self.service,
            &self.account,
            chrono::Utc::now(),
        )
    }
}
