use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;
#[cfg(feature = "openapi")]
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "posting_type", rename_all = "snake_case")]
pub enum PostingType {
    Original,
    Reversal,
    Adjustment,
    Correction,
}

impl std::fmt::Display for PostingType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Original => write!(f, "original"),
            Self::Reversal => write!(f, "reversal"),
            Self::Adjustment => write!(f, "adjustment"),
            Self::Correction => write!(f, "correction"),
        }
    }
}

impl FromStr for PostingType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "original" => Ok(Self::Original),
            "reversal" => Ok(Self::Reversal),
            "adjustment" => Ok(Self::Adjustment),
            "correction" => Ok(Self::Correction),
            _ => Err(format!("Unknown PostingType variant: {}", s)),
        }
    }
}

impl Default for PostingType {
    fn default() -> Self {
        Self::Original
    }
}
