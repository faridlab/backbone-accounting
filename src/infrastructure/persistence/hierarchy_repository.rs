//! SqlxHierarchyRepository — recursive-CTE adapter for the ancestor-chain port.
//!
//! One query serves all three hierarchical entities; the table + code column are substituted from
//! the `HierarchyTable` enum's constants (never user input).

use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::domain::repositories::hierarchy_repository::{HierarchyNode, HierarchyRepository, HierarchyTable};

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
        company_id: Uuid,
        id: Uuid,
    ) -> anyhow::Result<Vec<HierarchyNode>> {
        // Recursive walk up parent_id from the node to the root. `depth` counts hops from the
        // requested node (0 = self); ORDER BY depth DESC yields root-first.
        // Table + code column are compile-time constants from the enum — safe to interpolate.
        let sql = format!(
            r#"WITH RECURSIVE chain AS (
                   SELECT id, parent_id, {code} AS code, name, level, 0 AS depth
                     FROM {table}
                    WHERE id = $1 AND company_id = $2 AND (metadata->>'deleted_at') IS NULL
                   UNION ALL
                   SELECT t.id, t.parent_id, t.{code}, t.name, t.level, c.depth + 1
                     FROM {table} t JOIN chain c ON t.id = c.parent_id
                    WHERE t.company_id = $2 AND (t.metadata->>'deleted_at') IS NULL
               )
               SELECT id, parent_id, code, name, level FROM chain ORDER BY depth DESC"#,
            table = table.table(),
            code = table.code_column(),
        );

        let rows = sqlx::query(&sql)
            .bind(id)
            .bind(company_id)
            .fetch_all(&self.pool)
            .await?;

        Ok(rows
            .into_iter()
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
