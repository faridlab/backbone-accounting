use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "account_subtype", rename_all = "snake_case")]
pub enum AccountSubtype {
    CurrentAsset,
    NonCurrentAsset,
    FixedAsset,
    AccumulatedDepreciation,
    CurrentLiability,
    NonCurrentLiability,
    PaidInCapital,
    RetainedEarnings,
    OperatingRevenue,
    OperatingExpense,
    DirectCost,
    Bank,
    Cash,
    AccountsReceivable,
    AccountsPayable,
    Tax,
    Inventory,
}

impl std::fmt::Display for AccountSubtype {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CurrentAsset => write!(f, "current_asset"),
            Self::NonCurrentAsset => write!(f, "non_current_asset"),
            Self::FixedAsset => write!(f, "fixed_asset"),
            Self::AccumulatedDepreciation => write!(f, "accumulated_depreciation"),
            Self::CurrentLiability => write!(f, "current_liability"),
            Self::NonCurrentLiability => write!(f, "non_current_liability"),
            Self::PaidInCapital => write!(f, "paid_in_capital"),
            Self::RetainedEarnings => write!(f, "retained_earnings"),
            Self::OperatingRevenue => write!(f, "operating_revenue"),
            Self::OperatingExpense => write!(f, "operating_expense"),
            Self::DirectCost => write!(f, "direct_cost"),
            Self::Bank => write!(f, "bank"),
            Self::Cash => write!(f, "cash"),
            Self::AccountsReceivable => write!(f, "accounts_receivable"),
            Self::AccountsPayable => write!(f, "accounts_payable"),
            Self::Tax => write!(f, "tax"),
            Self::Inventory => write!(f, "inventory"),
        }
    }
}

impl FromStr for AccountSubtype {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "current_asset" => Ok(Self::CurrentAsset),
            "non_current_asset" => Ok(Self::NonCurrentAsset),
            "fixed_asset" => Ok(Self::FixedAsset),
            "accumulated_depreciation" => Ok(Self::AccumulatedDepreciation),
            "current_liability" => Ok(Self::CurrentLiability),
            "non_current_liability" => Ok(Self::NonCurrentLiability),
            "paid_in_capital" => Ok(Self::PaidInCapital),
            "retained_earnings" => Ok(Self::RetainedEarnings),
            "operating_revenue" => Ok(Self::OperatingRevenue),
            "operating_expense" => Ok(Self::OperatingExpense),
            "direct_cost" => Ok(Self::DirectCost),
            "bank" => Ok(Self::Bank),
            "cash" => Ok(Self::Cash),
            "accounts_receivable" => Ok(Self::AccountsReceivable),
            "accounts_payable" => Ok(Self::AccountsPayable),
            "tax" => Ok(Self::Tax),
            "inventory" => Ok(Self::Inventory),
            _ => Err(format!("Unknown AccountSubtype variant: {}", s)),
        }
    }
}

impl Default for AccountSubtype {
    fn default() -> Self {
        Self::CurrentAsset
    }
}
