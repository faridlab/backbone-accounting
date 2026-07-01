use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;
#[cfg(feature = "openapi")]
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "reconciliation_type", rename_all = "snake_case")]
pub enum ReconciliationType {
    Bank,
    AccountsReceivable,
    AccountsPayable,
    Intercompany,
    Other,
}

impl std::fmt::Display for ReconciliationType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Bank => write!(f, "bank"),
            Self::AccountsReceivable => write!(f, "accounts_receivable"),
            Self::AccountsPayable => write!(f, "accounts_payable"),
            Self::Intercompany => write!(f, "intercompany"),
            Self::Other => write!(f, "other"),
        }
    }
}

impl FromStr for ReconciliationType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "bank" => Ok(Self::Bank),
            "accounts_receivable" => Ok(Self::AccountsReceivable),
            "accounts_payable" => Ok(Self::AccountsPayable),
            "intercompany" => Ok(Self::Intercompany),
            "other" => Ok(Self::Other),
            _ => Err(format!("Unknown ReconciliationType variant: {}", s)),
        }
    }
}

impl Default for ReconciliationType {
    fn default() -> Self {
        Self::Bank
    }
}
