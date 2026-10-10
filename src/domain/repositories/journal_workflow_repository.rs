//! JournalWorkflowRepository — persistence port for the manual-journal approval lifecycle.
//!
//! Owns the journal status-transition UPDATEs and the header reads the workflow needs. The actual
//! ledger write on approve, and the reversal on void, go through `PostingService` /
//! `PostingRepository` — this port is only for the journal-row state machine.
//!
//! Tenancy (ADR-0029): the `company_id` params are the documented legacy twin — the port keeps
//! its shapes so unstripped callers compile and run unchanged; the adapter never keys a
//! statement on them (the ambient org scope scopes every statement instead, and on a decorated
//! deployment the decorator's fence makes a cross-tenant id miss).

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use uuid::Uuid;

/// Lightweight journal header for the void guard (status + currency).
#[derive(Debug, Clone)]
pub struct JournalStatusRow {
    pub status: String,
    pub currency: String,
}

/// The people a journal's self-approval rule compares the approver against.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct JournalAuthors {
    /// Who moved it from draft to pending approval.
    pub submitted_by: Option<Uuid>,
    /// Who created it (the audit metadata's `created_by`).
    pub created_by: Option<Uuid>,
}

#[async_trait]
pub trait JournalWorkflowRepository: Send + Sync {
    /// Load status + currency for the void guard. None if the journal doesn't exist / wrong tenant.
    async fn find_status(
        &self,
        journal_id: Uuid,
    ) -> anyhow::Result<Option<JournalStatusRow>>;

    /// Current status only (for precise not-found vs wrong-state errors). None if not found.
    async fn current_status(
        &self,
        journal_id: Uuid,
    ) -> anyhow::Result<Option<String>>;

    /// `draft → pending_approval`. Returns false if the journal wasn't `draft` (or not found).
    async fn submit(&self, journal_id: Uuid,) -> anyhow::Result<bool>;

    /// `draft → pending_approval`, recording who submitted it. The default records nobody.
    async fn submit_as(&self, journal_id: Uuid, submitted_by: Option<Uuid>) -> anyhow::Result<bool> {
        let _ = submitted_by;
        self.submit(journal_id).await
    }

    /// Who submitted and who created the journal, for the self-approval rule. The default
    /// knows neither.
    async fn authors(&self, journal_id: Uuid) -> anyhow::Result<JournalAuthors> {
        let _ = journal_id;
        Ok(JournalAuthors::default())
    }

    /// Whether the installation lets a journal's submitter or creator approve it
    /// (`accounting / journal_self_approval = allow`). The default refuses.
    async fn self_approval_allowed(&self) -> anyhow::Result<bool> {
        Ok(false)
    }

    /// `pending_approval → approved`, stamping approver/at. Returns false if not pending.
    async fn approve(
        &self,
        journal_id: Uuid,
        approved_by: Option<Uuid>,
        at: DateTime<Utc>,
    ) -> anyhow::Result<bool>;

    /// `draft|pending_approval → rejected` with a reason. Returns false if neither.
    async fn reject(
        &self,
        journal_id: Uuid,
        reason: &str,
        rejected_by: Option<Uuid>,
        at: DateTime<Utc>,
    ) -> anyhow::Result<bool>;

    /// Stamp a posted journal voided (status, is_voided, voided_at/by, reason).
    async fn mark_voided(
        &self,
        journal_id: Uuid,
        voided_by: Option<Uuid>,
        reason: &str,
        at: DateTime<Utc>,
    ) -> anyhow::Result<()>;

    /// The original posted accounting_post id for a journal (for the reversal). None if not posted.
    async fn original_post(
        &self,
        journal_id: Uuid,
    ) -> anyhow::Result<Option<Uuid>>;
}
