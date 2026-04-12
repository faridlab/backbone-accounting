use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "posting_source_type", rename_all = "snake_case")]
pub enum PostingSourceType {
    Order,
    Payment,
    Settlement,
    Refund,
    Expense,
    Inventory,
    Manual,
}

impl std::fmt::Display for PostingSourceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Order => write!(f, "order"),
            Self::Payment => write!(f, "payment"),
            Self::Settlement => write!(f, "settlement"),
            Self::Refund => write!(f, "refund"),
            Self::Expense => write!(f, "expense"),
            Self::Inventory => write!(f, "inventory"),
            Self::Manual => write!(f, "manual"),
        }
    }
}

impl FromStr for PostingSourceType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "order" => Ok(Self::Order),
            "payment" => Ok(Self::Payment),
            "settlement" => Ok(Self::Settlement),
            "refund" => Ok(Self::Refund),
            "expense" => Ok(Self::Expense),
            "inventory" => Ok(Self::Inventory),
            "manual" => Ok(Self::Manual),
            _ => Err(format!("Unknown PostingSourceType variant: {}", s)),
        }
    }
}

impl Default for PostingSourceType {
    fn default() -> Self {
        Self::Order
    }
}
