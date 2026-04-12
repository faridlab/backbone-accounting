use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "reconciliation_item_status", rename_all = "snake_case")]
pub enum ReconciliationItemStatus {
    Unmatched,
    Matched,
    PartialMatch,
    Adjusted,
    WrittenOff,
    Outstanding,
}

impl std::fmt::Display for ReconciliationItemStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unmatched => write!(f, "unmatched"),
            Self::Matched => write!(f, "matched"),
            Self::PartialMatch => write!(f, "partial_match"),
            Self::Adjusted => write!(f, "adjusted"),
            Self::WrittenOff => write!(f, "written_off"),
            Self::Outstanding => write!(f, "outstanding"),
        }
    }
}

impl FromStr for ReconciliationItemStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "unmatched" => Ok(Self::Unmatched),
            "matched" => Ok(Self::Matched),
            "partial_match" => Ok(Self::PartialMatch),
            "adjusted" => Ok(Self::Adjusted),
            "written_off" => Ok(Self::WrittenOff),
            "outstanding" => Ok(Self::Outstanding),
            _ => Err(format!("Unknown ReconciliationItemStatus variant: {}", s)),
        }
    }
}

impl Default for ReconciliationItemStatus {
    fn default() -> Self {
        Self::Unmatched
    }
}
