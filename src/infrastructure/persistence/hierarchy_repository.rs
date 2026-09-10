//! SqlxHierarchyRepository — recursive-CTE adapter for the ancestor-chain port.
//!
//! One query serves all three hierarchical entities; the table + code column are substituted from
//! the `HierarchyTable` enum's constants (never user input).
//!
//! Tenancy (ADR-0029): the module carries no tenancy of its own — the composing service's
//! tenancy decorator owns org scoping. The port's `company_id` lane is the documented legacy
//! twin: it keeps its shape for unstripped callers, but no statement keys on a tenant column.
//! The read rides the request-dedicated connection when the composing service bound one
//! (carrying the decorator's fence variables), plainly on the pool otherwise. An undecorated
//! deployment gets an unfenced module.

use sqlx::{PgPool, Row};
use uuid::Uuid;

// The multi-row read twin lives only in the legacy `company_scope` module. Its connection
// discipline is what this adapter needs — request-dedicated connection when the composing
// service bound one, plain pool otherwise. The helper's legacy task-local branch is never
// taken: this module sets no legacy scope of its own (ADR-0029).
use backbone_orm::company_scope::fetch_all_rows_scoped;

use crate::domain::repositories::hierarchy_repository::{
    HierarchyNode, HierarchyRepository, HierarchyTable,
};

pub struct SqlxHierarchyRepository {
    pool: PgPool,
}

impl SqlxHierarchyRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl HierarchyRepository for SqlxHierarchyRepository {
    async fn ancestors(
        &self,
        table: HierarchyTable,
        _company_id: Uuid,
        id: Uuid,
    ) -> anyhow::Result<Vec<HierarchyNode>> {
        // Recursive walk up parent_id from the node to the root. `depth` counts hops from the
        // requested node (0 = self); ORDER BY depth DESC yields root-first.
        // Table + code column are compile-time constants from the enum — safe to interpolate.
        let sql = format!(
            r#"WITH RECURSIVE chain AS (
                   SELECT id, parent_id, {code} AS code, name, level, 0 AS depth
                     FROM {table}
                    WHERE id = $1 AND (metadata->>'deleted_at') IS NULL
                   UNION ALL
                   SELECT t.id, t.parent_id, t.{code}, t.name, t.level, c.depth + 1
                     FROM {table} t JOIN chain c ON t.id = c.parent_id
                    WHERE (t.metadata->>'deleted_at') IS NULL
               )
               SELECT id, parent_id, code, name, level FROM chain ORDER BY depth DESC"#,
            table = table.table(),
            code = table.code_column(),
        );

        let rows = fetch_all_rows_scoped(&self.pool, sqlx::query(&sql).bind(id)).await?;

        Ok(rows
            .iter()
            .map(|r| HierarchyNode {
                id: r.get("id"),
                parent_id: r.get("parent_id"),
                code: r.get("code"),
                name: r.get("name"),
                level: r.get("level"),
            })
            .collect())
    }
}
