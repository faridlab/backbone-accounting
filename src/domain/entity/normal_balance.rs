use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "normal_balance", rename_all = "snake_case")]
pub enum NormalBalance {
    Debit,
    Credit,
}

impl std::fmt::Display for NormalBalance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Debit => write!(f, "debit"),
            Self::Credit => write!(f, "credit"),
        }
    }
}

impl FromStr for NormalBalance {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "debit" => Ok(Self::Debit),
            "credit" => Ok(Self::Credit),
            _ => Err(format!("Unknown NormalBalance variant: {}", s)),
        }
    }
}

impl Default for NormalBalance {
    fn default() -> Self {
        Self::Debit
    }
}
