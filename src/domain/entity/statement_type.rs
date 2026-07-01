use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;
#[cfg(feature = "openapi")]
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "statement_type", rename_all = "snake_case")]
pub enum StatementType {
    BalanceSheet,
    IncomeStatement,
    CashFlow,
    TrialBalance,
    EquityStatement,
}

impl std::fmt::Display for StatementType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BalanceSheet => write!(f, "balance_sheet"),
            Self::IncomeStatement => write!(f, "income_statement"),
            Self::CashFlow => write!(f, "cash_flow"),
            Self::TrialBalance => write!(f, "trial_balance"),
            Self::EquityStatement => write!(f, "equity_statement"),
        }
    }
}

impl FromStr for StatementType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "balance_sheet" => Ok(Self::BalanceSheet),
            "income_statement" => Ok(Self::IncomeStatement),
            "cash_flow" => Ok(Self::CashFlow),
            "trial_balance" => Ok(Self::TrialBalance),
            "equity_statement" => Ok(Self::EquityStatement),
            _ => Err(format!("Unknown StatementType variant: {}", s)),
        }
    }
}

impl Default for StatementType {
    fn default() -> Self {
        Self::BalanceSheet
    }
}
