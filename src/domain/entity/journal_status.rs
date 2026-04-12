use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "journal_status", rename_all = "snake_case")]
pub enum JournalStatus {
    Draft,
    PendingApproval,
    Approved,
    Rejected,
    Posted,
    Voided,
}

impl std::fmt::Display for JournalStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Draft => write!(f, "draft"),
            Self::PendingApproval => write!(f, "pending_approval"),
            Self::Approved => write!(f, "approved"),
            Self::Rejected => write!(f, "rejected"),
            Self::Posted => write!(f, "posted"),
            Self::Voided => write!(f, "voided"),
        }
    }
}

impl FromStr for JournalStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "draft" => Ok(Self::Draft),
            "pending_approval" => Ok(Self::PendingApproval),
            "approved" => Ok(Self::Approved),
            "rejected" => Ok(Self::Rejected),
            "posted" => Ok(Self::Posted),
            "voided" => Ok(Self::Voided),
            _ => Err(format!("Unknown JournalStatus variant: {}", s)),
        }
    }
}

impl Default for JournalStatus {
    fn default() -> Self {
        Self::Draft
    }
}
