//! Port for the chart install engine's persistence needs.
//!
//! All methods ride a caller-held transaction on which the caller has already bound
//! `app.company_id` (strict-fence posture: inserts need it for the RLS WITH CHECK,
//! reads need it to see rows at all). Hand-authored; see `metaphor.codegen.yaml`.

use crate::domain::chart_dataset::{ChartAccountDef, ChartDataset};
use uuid::Uuid;

/// One engine-owned account row, fully derived (tree facts resolved against the dataset).
#[derive(Debug, Clone)]
pub struct ChartAccountRow {
    pub id: Uuid,
    pub company_id: Uuid,
    pub def: ChartAccountDef,
    pub parent_id: Option<Uuid>,
    pub level: i32,
    pub path: String,
    pub is_header: bool,
    pub is_detail: bool,
    pub chart_code: String,
    pub chart_version: String,
}

/// A non-deleted account that collides with a dataset row on number or code.
#[derive(Debug, Clone)]
pub struct OverlappingAccount {
    pub id: Uuid,
    pub account_number: String,
    pub account_code: String,
    pub chart_code: Option<String>,
}

/// How a single upsert landed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpsertOutcome {
    /// No prior row with this id — fresh insert.
    Inserted,
    /// Our own prior row, updated in place (user-owned fields untouched).
    Updated,
    /// Our own prior row that had been soft-deleted, restored to life.
    Resurrected,
}

#[async_trait::async_trait]
pub trait ChartInstallRepository: Send + Sync {
    /// True when the company has ANY journal line — the engine refuses to install
    /// onto books that already carry postings.
    async fn company_has_postings(
        &self,
        tx: &mut sqlx::PgConnection,
        company_id: Uuid,
    ) -> anyhow::Result<bool>;

    /// Non-deleted accounts whose number or code collides with a dataset row.
    /// The engine classifies each as "ours" (deterministic id + own chart_code) or a conflict.
    async fn overlapping_accounts(
        &self,
        tx: &mut sqlx::PgConnection,
        company_id: Uuid,
        dataset: &ChartDataset,
    ) -> anyhow::Result<Vec<OverlappingAccount>>;

    /// Insert-or-update one engine-owned row keyed by its deterministic id. Engine-owned
    /// fields are always rewritten; user-owned fields (name, status, balances, bank
    /// details, tax/budget settings) are never touched. A soft-deleted own row is
    /// resurrected (deleted_at cleared).
    async fn upsert_account(
        &self,
        tx: &mut sqlx::PgConnection,
        row: &ChartAccountRow,
    ) -> anyhow::Result<UpsertOutcome>;
}
