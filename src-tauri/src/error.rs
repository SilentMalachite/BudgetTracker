use serde::{Serialize, Serializer};

#[derive(Debug, Clone, Serialize)]
pub struct ImportRowError {
    pub index: u32,
    pub entity: String,
    pub message: String,
}

impl std::fmt::Display for ImportRowError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}[{}]: {}", self.entity, self.index, self.message)
    }
}

impl ImportRowError {
    pub fn join_all(errors: &[Self]) -> String {
        errors
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("; ")
    }
}

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

    #[error("corrupt data: {0}")]
    Corrupt(String),

    #[error("not found: {0}")]
    NotFound(String),

    #[error("invalid argument: {0}")]
    InvalidArgument(String),

    #[error("conflict: {0}")]
    Conflict(String),

    #[error("unavailable: {0}")]
    Unavailable(String),

    #[error("import validation failed: {0}")]
    ImportValidation(String),
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

    #[test]
    fn conflict_displays_message() {
        let err = AppError::Conflict("name already exists".into());
        assert_eq!(err.to_string(), "conflict: name already exists");
    }

    #[test]
    fn not_found_displays_message() {
        let err = AppError::NotFound("category 42".into());
        assert_eq!(err.to_string(), "not found: category 42");
    }

    #[test]
    fn unavailable_error_displays_message() {
        assert_eq!(
            AppError::Unavailable("recovery".into()).to_string(),
            "unavailable: recovery"
        );
    }

    #[test]
    fn import_row_errors_join_entity_index_and_message() {
        let joined = ImportRowError::join_all(&[
            ImportRowError {
                index: 2,
                entity: "transaction".into(),
                message: "amount must be positive".into(),
            },
            ImportRowError {
                index: 4,
                entity: "transfer".into(),
                message: "source and destination must differ".into(),
            },
        ]);
        assert_eq!(
            joined,
            "transaction[2]: amount must be positive; transfer[4]: source and destination must differ"
        );
        let err = AppError::ImportValidation(joined);
        assert!(err.to_string().contains("transaction["));
        assert_eq!(
            err.to_string(),
            "import validation failed: transaction[2]: amount must be positive; transfer[4]: source and destination must differ"
        );
    }

    #[test]
    fn import_validation_serializes_to_string() {
        let err = AppError::ImportValidation("transaction[2]: amount must be positive".into());
        let json = serde_json::to_string(&err).unwrap();
        assert_eq!(
            json,
            "\"import validation failed: transaction[2]: amount must be positive\""
        );
    }
}
