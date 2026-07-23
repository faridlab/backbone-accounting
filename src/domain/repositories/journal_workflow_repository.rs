//! JournalWorkflowRepository — persistence port for the manual-journal approval lifecycle.
//!
//! Owns the journal status-transition UPDATEs and the header reads the workflow needs. The actual
//! ledger write on approve, and the reversal on void, go through `PostingService` /
//! `PostingRepository` — this port is only for the journal-row state machine.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use uuid::Uuid;

/// Lightweight journal header for the void guard (status + currency).
#[derive(Debug, Clone)]
pub struct JournalStatusRow {
    pub status: String,
    pub currency: String,
}

#[async_trait]
pub trait JournalWorkflowRepository: Send + Sync {
    /// Load status + currency for the void guard. None if the journal doesn't exist / wrong tenant.
    async fn find_status(
        &self,
        journal_id: Uuid,
        company_id: Uuid,
    ) -> anyhow::Result<Option<JournalStatusRow>>;

    /// Current status only (for precise not-found vs wrong-state errors). None if not found.
    async fn current_status(
        &self,
        journal_id: Uuid,
        company_id: Uuid,
    ) -> anyhow::Result<Option<String>>;

    /// `draft → pending_approval`. Returns false if the journal wasn't `draft` (or not found).
    async fn submit(&self, journal_id: Uuid, company_id: Uuid) -> anyhow::Result<bool>;

    /// `pending_approval → approved`, stamping approver/at. Returns false if not pending.
    async fn approve(
        &self,
        journal_id: Uuid,
        company_id: Uuid,
        approved_by: Option<Uuid>,
        at: DateTime<Utc>,
    ) -> anyhow::Result<bool>;

    /// `draft|pending_approval → rejected` with a reason. Returns false if neither.
    async fn reject(
        &self,
        journal_id: Uuid,
        company_id: Uuid,
        reason: &str,
        rejected_by: Option<Uuid>,
        at: DateTime<Utc>,
    ) -> anyhow::Result<bool>;

    /// Stamp a posted journal voided (status, is_voided, voided_at/by, reason).
    async fn mark_voided(
        &self,
        journal_id: Uuid,
        company_id: Uuid,
        voided_by: Option<Uuid>,
        reason: &str,
        at: DateTime<Utc>,
    ) -> anyhow::Result<()>;

    /// The original posted accounting_post id for a journal (for the reversal). None if not posted.
    async fn original_post(
        &self,
        journal_id: Uuid,
        company_id: Uuid,
    ) -> anyhow::Result<Option<Uuid>>;
}
