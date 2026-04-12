use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "account_type", rename_all = "snake_case")]
pub enum AccountType {
    Asset,
    Liability,
    Equity,
    Revenue,
    Expense,
    Cogs,
    OtherIncome,
    OtherExpense,
}

impl std::fmt::Display for AccountType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Asset => write!(f, "asset"),
            Self::Liability => write!(f, "liability"),
            Self::Equity => write!(f, "equity"),
            Self::Revenue => write!(f, "revenue"),
            Self::Expense => write!(f, "expense"),
            Self::Cogs => write!(f, "cogs"),
            Self::OtherIncome => write!(f, "other_income"),
            Self::OtherExpense => write!(f, "other_expense"),
        }
    }
}

impl FromStr for AccountType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "asset" => Ok(Self::Asset),
            "liability" => Ok(Self::Liability),
            "equity" => Ok(Self::Equity),
            "revenue" => Ok(Self::Revenue),
            "expense" => Ok(Self::Expense),
            "cogs" => Ok(Self::Cogs),
            "other_income" => Ok(Self::OtherIncome),
            "other_expense" => Ok(Self::OtherExpense),
            _ => Err(format!("Unknown AccountType variant: {}", s)),
        }
    }
}

impl Default for AccountType {
    fn default() -> Self {
        Self::Asset
    }
}
