use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;
#[cfg(feature = "openapi")]
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "reconcile_origin", rename_all = "snake_case")]
pub enum ReconcileOrigin {
    Settlement,
    Clearing,
    Manual,
}

impl std::fmt::Display for ReconcileOrigin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Settlement => write!(f, "settlement"),
            Self::Clearing => write!(f, "clearing"),
            Self::Manual => write!(f, "manual"),
        }
    }
}

impl FromStr for ReconcileOrigin {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "settlement" => Ok(Self::Settlement),
            "clearing" => Ok(Self::Clearing),
            "manual" => Ok(Self::Manual),
            _ => Err(format!("Unknown ReconcileOrigin variant: {}", s)),
        }
    }
}

impl Default for ReconcileOrigin {
    fn default() -> Self {
        Self::Manual
    }
}
