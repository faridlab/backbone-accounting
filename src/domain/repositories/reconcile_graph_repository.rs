//! ReconcileGraphRepository — the persistence port for the reconciliation graph.
//!
//! Domain trait (port), **connection-taking**: every method rides a caller-held
//! `&mut sqlx::PgConnection` so an edge (or its side-effecting unlink) commits atomically
//! with the caller's unit of work — a settlement, a clearing, or the verb's own
//! transaction. This is the same posture as the wire contract (`ReconcileSink` in
//! `backbone-gl-posting`): the connection is the transaction handle, which is why the
//! type appears here despite the domain layer otherwise avoiding `sqlx` in its contract
//! TYPES. Callers must have bound `app.company_id` on the connection (every statement also
//! carries an explicit `company_id` predicate — belt and braces under the strict fence).
//!
//! The SQLx implementation lives in `infrastructure/persistence/reconcile_graph_repository.rs`.

use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::domain::gl_posting::PostingLine;
use crate::domain::reconcile_graph::{
    AccountReconcileFlags, LineLocator, LocatorResolution, MatchingGroup, NewPartial, PartialRow,
    PartyResidual, ReconcileLineSnapshot,
};

/// Metadata needed to derive a reversal of a reconciliation-generated journal.
#[derive(Debug, Clone)]
pub struct JournalReversalMeta {
    pub journal_id: Uuid,
    pub company_id: Uuid,
    pub branch_id: Option<Uuid>,
    pub posting_date: NaiveDate,
    pub currency: String,
    pub source_id: Uuid,
    pub reverses_post_id: Option<Uuid>,
    pub fiscal_period_id: Option<Uuid>,
    pub fiscal_year: i32,
    pub fiscal_month: i32,
}

/// Persistence port for the reconciliation graph. All methods take the caller's connection.
#[async_trait]
pub trait ReconcileGraphRepository: Send + Sync {
    /// Resolve a locator to its single posted line, `FOR UPDATE` locked. Lines matching the
    /// producer identity on the account: 0 → `NotFound`, >1 → `Ambiguous(n)`.
    async fn lock_line_by_locator(
        &self,
        conn: &mut sqlx::PgConnection,
        company_id: Uuid,
        locator: &LineLocator,
    ) -> anyhow::Result<LocatorResolution>;

    /// Load one line by id, `FOR UPDATE` locked (verb-internal use).
    async fn lock_line(
        &self,
        conn: &mut sqlx::PgConnection,
        company_id: Uuid,
        line_id: Uuid,
    ) -> anyhow::Result<Option<ReconcileLineSnapshot>>;

    /// Account facts for the guards (reconcilable + subtype).
    async fn account_flags(
        &self,
        conn: &mut sqlx::PgConnection,
        company_id: Uuid,
        account_id: Uuid,
    ) -> anyhow::Result<Option<AccountReconcileFlags>>;

    /// Company-currency residuals for the given lines, locking them `FOR UPDATE` in
    /// deterministic id order: `base(face) − Σ partial amounts applied on either side`.
    async fn residuals_of(
        &self,
        conn: &mut sqlx::PgConnection,
        company_id: Uuid,
        line_ids: &[Uuid],
    ) -> anyhow::Result<Vec<(Uuid, Decimal)>>;

    /// Lock the given lines `FOR UPDATE` in deterministic id order (write-path discipline
    /// before recomputing component residuals).
    async fn lock_lines(
        &self,
        conn: &mut sqlx::PgConnection,
        company_id: Uuid,
        line_ids: &[Uuid],
    ) -> anyhow::Result<()>;

    /// Insert a partial edge; returns its id.
    async fn insert_partial(
        &self,
        conn: &mut sqlx::PgConnection,
        p: &NewPartial,
    ) -> anyhow::Result<Uuid>;

    /// Stamp the generated exchange-difference journal onto a partial.
    async fn set_exchange_move(
        &self,
        conn: &mut sqlx::PgConnection,
        company_id: Uuid,
        partial_id: Uuid,
        journal_id: Uuid,
    ) -> anyhow::Result<()>;

    /// The connected component (transitive closure over partial edges) of the seed lines.
    async fn component_line_ids(
        &self,
        conn: &mut sqlx::PgConnection,
        company_id: Uuid,
        seeds: &[Uuid],
    ) -> anyhow::Result<Vec<Uuid>>;

    /// All partial edges touching the given lines.
    async fn component_partial_ids(
        &self,
        conn: &mut sqlx::PgConnection,
        company_id: Uuid,
        line_ids: &[Uuid],
    ) -> anyhow::Result<Vec<Uuid>>;

    /// The DISTINCT non-null `full_reconcile_id` stamps carried by the given lines. Empty = nobody
    /// stamped; one = the component already (or partially) belongs to that group; more than one =
    /// divergent stamps (unreachable by construction — a zero-residual line can never gain a new
    /// edge, so completed components cannot merge).
    async fn distinct_group_stamps(
        &self,
        conn: &mut sqlx::PgConnection,
        company_id: Uuid,
        line_ids: &[Uuid],
    ) -> anyhow::Result<Vec<Uuid>>;

    /// Create a full-reconcile group row; returns its id.
    async fn create_full_group(
        &self,
        conn: &mut sqlx::PgConnection,
        company_id: Uuid,
        exchange_total: Decimal,
        now: DateTime<Utc>,
    ) -> anyhow::Result<Uuid>;

    /// Attach a group: stamp `full_reconcile_id` + `is_reconciled` + `reconciled_at` on the
    /// lines and `full_reconcile_id` on the partials.
    async fn attach_group(
        &self,
        conn: &mut sqlx::PgConnection,
        company_id: Uuid,
        group_id: Uuid,
        line_ids: &[Uuid],
        partial_ids: &[Uuid],
        now: DateTime<Utc>,
    ) -> anyhow::Result<()>;

    /// Clear the reconciled flags (full_reconcile_id, is_reconciled, reconciled_at) on lines.
    async fn clear_line_flags(
        &self,
        conn: &mut sqlx::PgConnection,
        company_id: Uuid,
        line_ids: &[Uuid],
    ) -> anyhow::Result<()>;

    /// Load one partial by id.
    async fn load_partial(
        &self,
        conn: &mut sqlx::PgConnection,
        company_id: Uuid,
        partial_id: Uuid,
    ) -> anyhow::Result<Option<PartialRow>>;

    /// All partials directly between two lines (either orientation).
    async fn partials_between(
        &self,
        conn: &mut sqlx::PgConnection,
        company_id: Uuid,
        a_id: Uuid,
        b_id: Uuid,
    ) -> anyhow::Result<Vec<PartialRow>>;

    /// Partials derived from a parent edge (`source_id = parent id` — e.g. the second edge an
    /// exchange difference creates).
    async fn derived_partials(
        &self,
        conn: &mut sqlx::PgConnection,
        company_id: Uuid,
        parent_partial_id: Uuid,
    ) -> anyhow::Result<Vec<PartialRow>>;

    /// Journals a partial generated (stamped `source_type='reconciliation'`,
    /// `source_id ∈ {parent, derived}` — the hook future cash-basis moves ride).
    async fn generated_journal_ids(
        &self,
        conn: &mut sqlx::PgConnection,
        company_id: Uuid,
        partial_ids: &[Uuid],
    ) -> anyhow::Result<Vec<Uuid>>;

    /// A journal's lines with their ids (reversal derivation).
    async fn journal_lines_with_ids(
        &self,
        conn: &mut sqlx::PgConnection,
        company_id: Uuid,
        journal_id: Uuid,
    ) -> anyhow::Result<Vec<(Uuid, PostingLine)>>;

    /// A journal's reversal metadata.
    async fn journal_reversal_meta(
        &self,
        conn: &mut sqlx::PgConnection,
        company_id: Uuid,
        journal_id: Uuid,
    ) -> anyhow::Result<Option<JournalReversalMeta>>;

    /// Delete the given partials (the write service has already reversed their generated moves).
    async fn delete_partials(
        &self,
        conn: &mut sqlx::PgConnection,
        company_id: Uuid,
        partial_ids: &[Uuid],
    ) -> anyhow::Result<()>;

    /// Partial ids still attached to a group (survivor check before dissolve).
    async fn group_partial_ids(
        &self,
        conn: &mut sqlx::PgConnection,
        company_id: Uuid,
        group_id: Uuid,
    ) -> anyhow::Result<Vec<Uuid>>;

    /// Dissolve an emptied group row.
    async fn dissolve_group(
        &self,
        conn: &mut sqlx::PgConnection,
        company_id: Uuid,
        group_id: Uuid,
    ) -> anyhow::Result<()>;

    /// The same-account, same-source posted counterpart with the opposite reversing flag,
    /// for the reverse-then-reconcile pairing.
    async fn reversal_counterpart(
        &self,
        conn: &mut sqlx::PgConnection,
        company_id: Uuid,
        line: &ReconcileLineSnapshot,
    ) -> anyhow::Result<Option<ReconcileLineSnapshot>>;

    /// Open AR/AP lines (with computed residuals) for one party on one account — the aging read.
    async fn residuals_for_party(
        &self,
        conn: &mut sqlx::PgConnection,
        company_id: Uuid,
        account_id: Uuid,
        party_type: &str,
        party_id: Uuid,
    ) -> anyhow::Result<Vec<PartyResidual>>;

    /// The matching-group read for one line (connected component + derived label).
    async fn matching_group(
        &self,
        conn: &mut sqlx::PgConnection,
        company_id: Uuid,
        line_id: Uuid,
    ) -> anyhow::Result<MatchingGroup>;

    /// True if any fiscal period covering `date` is closed/locked.
    async fn period_closed(
        &self,
        conn: &mut sqlx::PgConnection,
        company_id: Uuid,
        date: NaiveDate,
    ) -> anyhow::Result<bool>;
}
