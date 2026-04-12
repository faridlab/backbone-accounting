use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "journal_source", rename_all = "snake_case")]
pub enum JournalSource {
    Manual,
    Order,
    Payment,
    Settlement,
    Adjustment,
    Import,
    System,
    Recurring,
}

impl std::fmt::Display for JournalSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Manual => write!(f, "manual"),
            Self::Order => write!(f, "order"),
            Self::Payment => write!(f, "payment"),
            Self::Settlement => write!(f, "settlement"),
            Self::Adjustment => write!(f, "adjustment"),
            Self::Import => write!(f, "import"),
            Self::System => write!(f, "system"),
            Self::Recurring => write!(f, "recurring"),
        }
    }
}

impl FromStr for JournalSource {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "manual" => Ok(Self::Manual),
            "order" => Ok(Self::Order),
            "payment" => Ok(Self::Payment),
            "settlement" => Ok(Self::Settlement),
            "adjustment" => Ok(Self::Adjustment),
            "import" => Ok(Self::Import),
            "system" => Ok(Self::System),
            "recurring" => Ok(Self::Recurring),
            _ => Err(format!("Unknown JournalSource variant: {}", s)),
        }
    }
}

impl Default for JournalSource {
    fn default() -> Self {
        Self::Manual
    }
}
