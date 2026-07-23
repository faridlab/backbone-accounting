//! HierarchyService — application port over `HierarchyRepository`.
//!
//! Read-only ancestor-chain lookup for the three hierarchical entities. No business rules — the
//! service exists so the presentation layer depends on an application service, not a domain port
//! directly (per the 4-layer convention).

use std::sync::Arc;

use uuid::Uuid;

use crate::domain::repositories::hierarchy_repository::{
    HierarchyNode, HierarchyRepository, HierarchyTable,
};

#[derive(Clone)]
pub struct HierarchyService {
    repo: Arc<dyn HierarchyRepository>,
}

impl HierarchyService {
    pub fn new(repo: Arc<dyn HierarchyRepository>) -> Self {
        Self { repo }
    }

    /// Ancestor chain root → `id` for the given entity kind.
    pub async fn ancestors(
        &self,
        table: HierarchyTable,
        company_id: Uuid,
        id: Uuid,
    ) -> anyhow::Result<Vec<HierarchyNode>> {
        self.repo.ancestors(table, company_id, id).await
    }
}
