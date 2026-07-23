//! PostingRepository — the persistence port for the GL-posting contract.
//!
//! Domain trait (port). The application `PostingService` depends on this, never on a `PgPool`.
//! The SQLx implementation lives in `infrastructure/persistence/posting_repository.rs`.
//! Methods take/return plain DTOs — no `sqlx::Row` leaks across the boundary.

use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::domain::gl_posting::PostingLine;

/// Per-account snapshot loaded for validation + denormalization.
#[derive(Debug, Clone)]
pub struct PostableAccount {
    pub id: Uuid,
    pub number: String,
    pub name: String,
    pub account_type: String,
    pub subtype: String,
    pub normal_balance: String, // "debit" | "credit"
    pub is_detail: bool,
    pub is_header: bool,
    pub status: String,
    pub current_balance: Decimal,
}

/// A line to write to the ledger. `journal_line_id` is the already-persisted `journal_lines` row
/// this ledger entry back-references — freshly created by a new post, or loaded from an existing
/// draft journal by `post_journal`.
#[derive(Debug, Clone)]
pub struct LedgerEntryInput {
    pub journal_line_id: Uuid,
    pub line: PostingLine,
}

/// Everything `commit_posting` needs to atomically write a fresh Journal + Lines + Ledger +
/// AccountingPost. The adapter opens its own transaction and takes the per-account `FOR UPDATE`
/// lock internally.
#[derive(Debug, Clone)]
pub struct PostingWrite {
    pub company_id: Uuid,
    pub branch_id: Option<Uuid>,
    pub source_type: String,
    pub source_id: Uuid,
    pub source_reference: Option<String>,
    pub posting_date: NaiveDate,
    pub fiscal_period_id: Option<Uuid>,
    pub fiscal_year: i32,
    pub fiscal_month: i32,
    pub currency: String,
    pub posting_type: String,
    pub reverses_post_id: Option<Uuid>,
    pub reverses_journal_id: Option<Uuid>,
    pub description: Option<String>,
    pub idempotency_key: Option<String>,
    pub posted_by: Option<Uuid>,
    pub now: DateTime<Utc>,
    pub lines: Vec<PostingLine>,
}

/// Result of an atomic posting commit. `reused` is true when a concurrent post for the same
/// source won the race and this call returned the winner instead of writing.
#[derive(Debug, Clone)]
pub struct PostingCommit {
    pub post_id: Uuid,
    pub journal_id: Uuid,
    pub total_debit: Decimal,
    pub total_credit: Decimal,
    pub reused: bool,
}

/// The original journal + its lines, used to derive a reversal (debit/credit swapped).
#[derive(Debug, Clone)]
pub struct ReversalSource {
    pub journal_id: Uuid,
    pub lines: Vec<PostingLine>,
}

/// A loaded draft/approved manual journal ready to post.
#[derive(Debug, Clone)]
pub struct ManualJournalForPost {
    pub status: String,
    pub journal_number: String,
    pub branch_id: Option<Uuid>,
    pub posting_date: NaiveDate,
    pub fiscal_period_id: Option<Uuid>,
    pub fiscal_year: i32,
    pub fiscal_month: i32,
    pub currency: String,
    pub description: Option<String>,
    pub source_type: String,
    pub source_id: Uuid,
    /// (journal_line_id, line) — the id is needed to back-reference ledger rows.
    pub lines: Vec<(Uuid, PostingLine)>,
}

/// Everything `commit_manual_journal` needs to write the ledger for an already-loaded + validated
/// approved journal (flips it to posted, writes ledger rows, records the AccountingPost).
#[derive(Debug, Clone)]
pub struct ManualJournalCommit {
    pub journal_id: Uuid,
    pub company_id: Uuid,
    pub branch_id: Option<Uuid>,
    pub journal_number: String,
    pub posting_date: NaiveDate,
    pub fiscal_period_id: Option<Uuid>,
    pub fiscal_year: i32,
    pub fiscal_month: i32,
    pub currency: String,
    pub source_type: String,
    pub source_id: Uuid,
    pub description: Option<String>,
    pub posted_by: Option<Uuid>,
    pub now: DateTime<Utc>,
    pub lines: Vec<LedgerEntryInput>,
}

/// A failed-post audit record.
#[derive(Debug, Clone)]
pub struct FailedPost {
    pub company_id: Uuid,
    pub branch_id: Option<Uuid>,
    pub source_type: String,
    pub source_id: Uuid,
    pub source_reference: Option<String>,
    pub posting_type: String,
    pub currency: String,
    pub total_debit: Decimal,
    pub total_credit: Decimal,
    pub failed_at: DateTime<Utc>,
    pub error_code: String,
    pub error_message: String,
}

/// Persistence port for the GL-posting contract.
#[async_trait]
pub trait PostingRepository: Send + Sync {
    /// Idempotency lookup: the existing posted entry for this source identity, if any.
    async fn find_existing_post(
        &self,
        company_id: Uuid,
        source_type: &str,
        source_id: Uuid,
        posting_type: &str,
        idempotency_key: Option<&str>,
    ) -> anyhow::Result<Option<(Uuid, Uuid)>>;

    /// Load postable accounts by id (for validation). Not locked.
    async fn find_postable_accounts(
        &self,
        company_id: Uuid,
        ids: &[Uuid],
    ) -> anyhow::Result<Vec<PostableAccount>>;

    /// True if any fiscal period covering `date` is closed/locked.
    async fn is_period_closed(&self, company_id: Uuid, date: NaiveDate) -> anyhow::Result<bool>;

    /// The narrowest open fiscal period id covering `date`, if any.
    async fn find_period_id(
        &self,
        company_id: Uuid,
        date: NaiveDate,
    ) -> anyhow::Result<Option<Uuid>>;

    /// Load the original journal + lines for a reversal.
    async fn find_reversal_source(
        &self,
        orig_post_id: Uuid,
        company_id: Uuid,
    ) -> anyhow::Result<Option<ReversalSource>>;

    /// Atomically commit a fresh posting (journal + lines + ledger + balances + accounting_post +
    /// reversal links). Owns the transaction and the per-account `FOR UPDATE` lock. Handles the
    /// partial-unique-index concurrency arbiter internally (returns `reused=true` on a race loss).
    async fn commit_posting(&self, write: PostingWrite) -> anyhow::Result<PostingCommit>;

    /// Load a manual journal + its lines for posting (must exist + match tenant).
    async fn find_manual_journal_for_post(
        &self,
        journal_id: Uuid,
        company_id: Uuid,
    ) -> anyhow::Result<Option<ManualJournalForPost>>;

    /// The existing posted accounting_post for a journal, if already posted (idempotency).
    async fn existing_post_for_journal(
        &self,
        journal_id: Uuid,
        company_id: Uuid,
    ) -> anyhow::Result<Option<Uuid>>;

    /// Atomically post an approved manual journal (ledger rows + flip to posted + accounting_post).
    async fn commit_manual_journal(
        &self,
        commit: ManualJournalCommit,
    ) -> anyhow::Result<PostingCommit>;

    /// Record a failed posting attempt (audit).
    async fn record_failed(&self, failed: FailedPost) -> anyhow::Result<()>;
}
