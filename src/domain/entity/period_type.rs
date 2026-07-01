use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;
#[cfg(feature = "openapi")]
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "period_type", rename_all = "snake_case")]
pub enum PeriodType {
    Monthly,
    Quarterly,
    Yearly,
    Custom,
}

impl std::fmt::Display for PeriodType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Monthly => write!(f, "monthly"),
            Self::Quarterly => write!(f, "quarterly"),
            Self::Yearly => write!(f, "yearly"),
            Self::Custom => write!(f, "custom"),
        }
    }
}

impl FromStr for PeriodType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "monthly" => Ok(Self::Monthly),
            "quarterly" => Ok(Self::Quarterly),
            "yearly" => Ok(Self::Yearly),
            "custom" => Ok(Self::Custom),
            _ => Err(format!("Unknown PeriodType variant: {}", s)),
        }
    }
}

impl Default for PeriodType {
    fn default() -> Self {
        Self::Monthly
    }
}
