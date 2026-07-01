use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;
#[cfg(feature = "openapi")]
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "journal_type", rename_all = "snake_case")]
pub enum JournalType {
    General,
    Sales,
    Purchase,
    CashReceipt,
    CashDisbursement,
    Payroll,
    Adjusting,
    Closing,
    Reversing,
    Opening,
}

impl std::fmt::Display for JournalType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::General => write!(f, "general"),
            Self::Sales => write!(f, "sales"),
            Self::Purchase => write!(f, "purchase"),
            Self::CashReceipt => write!(f, "cash_receipt"),
            Self::CashDisbursement => write!(f, "cash_disbursement"),
            Self::Payroll => write!(f, "payroll"),
            Self::Adjusting => write!(f, "adjusting"),
            Self::Closing => write!(f, "closing"),
            Self::Reversing => write!(f, "reversing"),
            Self::Opening => write!(f, "opening"),
        }
    }
}

impl FromStr for JournalType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "general" => Ok(Self::General),
            "sales" => Ok(Self::Sales),
            "purchase" => Ok(Self::Purchase),
            "cash_receipt" => Ok(Self::CashReceipt),
            "cash_disbursement" => Ok(Self::CashDisbursement),
            "payroll" => Ok(Self::Payroll),
            "adjusting" => Ok(Self::Adjusting),
            "closing" => Ok(Self::Closing),
            "reversing" => Ok(Self::Reversing),
            "opening" => Ok(Self::Opening),
            _ => Err(format!("Unknown JournalType variant: {}", s)),
        }
    }
}

impl Default for JournalType {
    fn default() -> Self {
        Self::General
    }
}
