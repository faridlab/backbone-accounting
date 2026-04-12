use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "reconciliation_status", rename_all = "snake_case")]
pub enum ReconciliationStatus {
    InProgress,
    PendingReview,
    Reviewed,
    Completed,
    Cancelled,
}

impl std::fmt::Display for ReconciliationStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InProgress => write!(f, "in_progress"),
            Self::PendingReview => write!(f, "pending_review"),
            Self::Reviewed => write!(f, "reviewed"),
            Self::Completed => write!(f, "completed"),
            Self::Cancelled => write!(f, "cancelled"),
        }
    }
}

impl FromStr for ReconciliationStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "in_progress" => Ok(Self::InProgress),
            "pending_review" => Ok(Self::PendingReview),
            "reviewed" => Ok(Self::Reviewed),
            "completed" => Ok(Self::Completed),
            "cancelled" => Ok(Self::Cancelled),
            _ => Err(format!("Unknown ReconciliationStatus variant: {}", s)),
        }
    }
}

impl Default for ReconciliationStatus {
    fn default() -> Self {
        Self::InProgress
    }
}
