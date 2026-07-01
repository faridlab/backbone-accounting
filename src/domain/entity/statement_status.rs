use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;
#[cfg(feature = "openapi")]
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "statement_status", rename_all = "snake_case")]
pub enum StatementStatus {
    Draft,
    Generated,
    Reviewed,
    Approved,
    Published,
    Archived,
}

impl std::fmt::Display for StatementStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Draft => write!(f, "draft"),
            Self::Generated => write!(f, "generated"),
            Self::Reviewed => write!(f, "reviewed"),
            Self::Approved => write!(f, "approved"),
            Self::Published => write!(f, "published"),
            Self::Archived => write!(f, "archived"),
        }
    }
}

impl FromStr for StatementStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "draft" => Ok(Self::Draft),
            "generated" => Ok(Self::Generated),
            "reviewed" => Ok(Self::Reviewed),
            "approved" => Ok(Self::Approved),
            "published" => Ok(Self::Published),
            "archived" => Ok(Self::Archived),
            _ => Err(format!("Unknown StatementStatus variant: {}", s)),
        }
    }
}

impl Default for StatementStatus {
    fn default() -> Self {
        Self::Draft
    }
}
