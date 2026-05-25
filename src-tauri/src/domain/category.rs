use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CategoryType {
    Income,
    Expense,
}

impl CategoryType {
    pub fn as_sql(self) -> &'static str {
        match self {
            CategoryType::Income => "income",
            CategoryType::Expense => "expense",
        }
    }

    pub fn parse(raw: &str) -> AppResult<Self> {
        match raw {
            "income" => Ok(Self::Income),
            "expense" => Ok(Self::Expense),
            other => Err(AppError::InvalidArgument(format!(
                "category type must be 'income' or 'expense', got '{other}'"
            ))),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Category {
    pub id: i64,
    pub name: String,
    #[serde(rename = "type")]
    pub type_: CategoryType,
    pub color: Option<String>,
    pub icon: Option<String>,
    pub display_order: i64,
    pub archived_at: Option<String>,
}

const MAX_NAME_LEN: usize = 40;

pub fn validate_name(raw: &str) -> AppResult<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(AppError::InvalidArgument("category name is empty".into()));
    }
    if trimmed.chars().count() > MAX_NAME_LEN {
        return Err(AppError::InvalidArgument(format!(
            "category name must be {MAX_NAME_LEN} chars or fewer"
        )));
    }
    Ok(trimmed.to_string())
}

pub fn validate_color(raw: &str) -> AppResult<String> {
    let ok =
        raw.len() == 7 && raw.starts_with('#') && raw[1..].chars().all(|c| c.is_ascii_hexdigit());
    if !ok {
        return Err(AppError::InvalidArgument(format!(
            "color must match #RRGGBB hex, got '{raw}'"
        )));
    }
    Ok(raw.to_ascii_uppercase())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::AppError;

    #[test]
    fn parses_known_types() {
        assert_eq!(CategoryType::parse("income").unwrap(), CategoryType::Income);
        assert_eq!(
            CategoryType::parse("expense").unwrap(),
            CategoryType::Expense
        );
    }

    #[test]
    fn rejects_unknown_type() {
        let err = CategoryType::parse("transfer").unwrap_err();
        assert!(matches!(err, AppError::InvalidArgument(_)));
    }

    #[test]
    fn validate_name_trims_and_accepts_jp() {
        assert_eq!(validate_name("  食費  ").unwrap(), "食費");
    }

    #[test]
    fn validate_name_rejects_empty_and_whitespace_only() {
        assert!(validate_name("").is_err());
        assert!(validate_name("   ").is_err());
    }

    #[test]
    fn validate_name_rejects_over_40_chars() {
        let name = "あ".repeat(41);
        assert!(validate_name(&name).is_err());
    }

    #[test]
    fn validate_color_accepts_lower_and_uppercase() {
        assert_eq!(validate_color("#ff00aa").unwrap(), "#FF00AA");
        assert_eq!(validate_color("#FF00AA").unwrap(), "#FF00AA");
    }

    #[test]
    fn validate_color_rejects_short_and_non_hex() {
        assert!(validate_color("#FFF").is_err());
        assert!(validate_color("FF00AA").is_err());
        assert!(validate_color("#GGGGGG").is_err());
    }
}
