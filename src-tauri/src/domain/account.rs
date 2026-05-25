use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccountKind {
    Cash,
    Bank,
    CreditCard,
    EMoney,
    Investment,
}

impl AccountKind {
    pub fn as_sql(self) -> &'static str {
        match self {
            AccountKind::Cash => "cash",
            AccountKind::Bank => "bank",
            AccountKind::CreditCard => "credit_card",
            AccountKind::EMoney => "e_money",
            AccountKind::Investment => "investment",
        }
    }

    pub fn parse(raw: &str) -> AppResult<Self> {
        match raw {
            "cash" => Ok(Self::Cash),
            "bank" => Ok(Self::Bank),
            "credit_card" => Ok(Self::CreditCard),
            "e_money" => Ok(Self::EMoney),
            "investment" => Ok(Self::Investment),
            other => Err(AppError::InvalidArgument(format!(
                "account kind must be cash|bank|credit_card|e_money|investment, got '{other}'"
            ))),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub id: i64,
    pub name: String,
    pub kind: AccountKind,
    pub currency: String,
    pub initial_balance: i64,
    pub display_order: i64,
    pub note: String,
    pub archived_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

const MAX_NAME_LEN: usize = 40;
const MAX_NOTE_LEN: usize = 200;

pub fn validate_name(raw: &str) -> AppResult<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(AppError::InvalidArgument("account name is empty".into()));
    }
    if trimmed.chars().count() > MAX_NAME_LEN {
        return Err(AppError::InvalidArgument(format!(
            "account name must be {MAX_NAME_LEN} chars or fewer"
        )));
    }
    Ok(trimmed.to_string())
}

pub fn validate_note(raw: &str) -> AppResult<String> {
    if raw.chars().count() > MAX_NOTE_LEN {
        return Err(AppError::InvalidArgument(format!(
            "note must be {MAX_NOTE_LEN} chars or fewer"
        )));
    }
    Ok(raw.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_all_known_kinds() {
        for s in ["cash", "bank", "credit_card", "e_money", "investment"] {
            assert!(AccountKind::parse(s).is_ok(), "{s} should parse");
        }
    }

    #[test]
    fn rejects_unknown_kind() {
        assert!(matches!(
            AccountKind::parse("crypto").unwrap_err(),
            AppError::InvalidArgument(_)
        ));
    }

    #[test]
    fn validate_name_trims() {
        assert_eq!(validate_name("  楽天銀行  ").unwrap(), "楽天銀行");
    }

    #[test]
    fn validate_name_rejects_empty() {
        assert!(validate_name("").is_err());
        assert!(validate_name("   ").is_err());
    }

    #[test]
    fn validate_note_accepts_empty_and_200_chars() {
        assert!(validate_note("").is_ok());
        assert!(validate_note(&"あ".repeat(200)).is_ok());
        assert!(validate_note(&"あ".repeat(201)).is_err());
    }
}
