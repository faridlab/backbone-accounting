use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "period_status", rename_all = "snake_case")]
pub enum PeriodStatus {
    Open,
    Closing,
    Closed,
    Locked,
    Adjusting,
}

impl std::fmt::Display for PeriodStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Open => write!(f, "open"),
            Self::Closing => write!(f, "closing"),
            Self::Closed => write!(f, "closed"),
            Self::Locked => write!(f, "locked"),
            Self::Adjusting => write!(f, "adjusting"),
        }
    }
}

impl FromStr for PeriodStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "open" => Ok(Self::Open),
            "closing" => Ok(Self::Closing),
            "closed" => Ok(Self::Closed),
            "locked" => Ok(Self::Locked),
            "adjusting" => Ok(Self::Adjusting),
            _ => Err(format!("Unknown PeriodStatus variant: {}", s)),
        }
    }
}

impl Default for PeriodStatus {
    fn default() -> Self {
        Self::Open
    }
}
