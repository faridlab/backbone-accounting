use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;
#[cfg(feature = "openapi")]
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "party_type", rename_all = "snake_case")]
pub enum PartyType {
    Customer,
    Supplier,
    Employee,
}

impl std::fmt::Display for PartyType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Customer => write!(f, "customer"),
            Self::Supplier => write!(f, "supplier"),
            Self::Employee => write!(f, "employee"),
        }
    }
}

impl FromStr for PartyType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "customer" => Ok(Self::Customer),
            "supplier" => Ok(Self::Supplier),
            "employee" => Ok(Self::Employee),
            _ => Err(format!("Unknown PartyType variant: {}", s)),
        }
    }
}

impl Default for PartyType {
    fn default() -> Self {
        Self::Customer
    }
}
