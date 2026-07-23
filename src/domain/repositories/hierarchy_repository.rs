//! HierarchyRepository — persistence port for reading an entity's ancestor chain.
//!
//! Serves the three hierarchical entities (account, cost_center, fiscal_period), which all share
//! `id`, `company_id`, `parent_id`, `level`, `name`, plus a code column. One generic port + one
//! recursive-CTE adapter serve all three; the `HierarchyTable` enum supplies the table + code
//! column names (hardcoded constants — never user input, so no injection surface).

use async_trait::async_trait;
use uuid::Uuid;

/// One node in an ancestor chain (root → self).
#[derive(Debug, Clone, serde::Serialize)]
pub struct HierarchyNode {
    pub id: Uuid,
    pub parent_id: Option<Uuid>,
    pub code: Option<String>,
    pub name: String,
    pub level: i32,
}

/// Which hierarchical entity to walk. Carries the (table, code column) the adapter builds its
/// recursive CTE against.
#[derive(Debug, Clone, Copy)]
pub enum HierarchyTable {
    Account,
    CostCenter,
    FiscalPeriod,
}

impl HierarchyTable {
    /// Fully-qualified table name.
    pub fn table(self) -> &'static str {
        match self {
            Self::Account => "accounting.accounts",
            Self::CostCenter => "accounting.cost_centers",
            Self::FiscalPeriod => "accounting.fiscal_periods",
        }
    }
    /// The human-facing code column for this entity (account_number / code / period_code).
    pub fn code_column(self) -> &'static str {
        match self {
            Self::Account => "account_number",
            Self::CostCenter => "code",
            Self::FiscalPeriod => "period_code",
        }
    }
}

#[async_trait]
pub trait HierarchyRepository: Send + Sync {
    /// Return the ancestor chain from the root down to (and including) `id`, root-first.
    /// Empty if `id` is not found / wrong tenant.
    async fn ancestors(
        &self,
        table: HierarchyTable,
        company_id: Uuid,
        id: Uuid,
    ) -> anyhow::Result<Vec<HierarchyNode>>;
}
