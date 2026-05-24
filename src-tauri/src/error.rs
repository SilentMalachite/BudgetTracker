use serde::{Serialize, Serializer};

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),

    #[error("keychain error: {0}")]
    Keychain(#[from] keyring::Error),

    #[error("base64 decode error: {0}")]
    Base64(#[from] base64::DecodeError),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("migration error: {0}")]
    Migration(String),

    #[error("not found: {0}")]
    NotFound(String),

    #[error("invalid argument: {0}")]
    InvalidArgument(String),
}

pub type AppResult<T> = Result<T, AppError>;

impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migration_error_displays_message() {
        let err = AppError::Migration("V002 broken".into());
        assert_eq!(err.to_string(), "migration error: V002 broken");
    }

    #[test]
    fn error_serializes_to_string() {
        let err = AppError::InvalidArgument("amount must be > 0".into());
        let json = serde_json::to_string(&err).unwrap();
        assert_eq!(json, "\"invalid argument: amount must be > 0\"");
    }
}
