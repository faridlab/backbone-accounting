//! Manual-journal approval workflow — the draft → submit → approve → post → void lifecycle.
//!
//! Hand-authored behavior (user-owned; survives regeneration). Composes `PostingService` for the
//! actual ledger write: `approve` flips the journal to `approved` then calls `PostingService::
//! post_journal` (which writes the immutable ledger rows atomically); `void` builds a reversal
//! `PostingRequest` and posts it through `PostingService::post` (debit/credit swapped, linked both
//! ways), then stamps the original journal voided. No new write paths — everything flows through the
//! audited posting core.
//!
//! See `docs/business-flows/gl-posting.md` (manual-journal flow) and BRD §3.

use chrono::Utc;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::application::service::posting_service::{
    PostingError, PostingRequest, PostingResult, PostingService,
};

/// Typed workflow failure. `code()` is the stable error string.
#[derive(Debug)]
pub enum JournalWorkflowError {
    NotFound(Uuid),
    InvalidState { id: Uuid, current: String, expected: &'static str },
    NotPosted(Uuid),
    Posting(PostingError),
    Db(sqlx::Error),
}

impl JournalWorkflowError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::NotFound(_) => "journal_not_found",
            Self::InvalidState { .. } => "invalid_journal_state",
            Self::NotPosted(_) => "journal_not_posted",
            Self::Posting(_) => "posting_error",
            Self::Db(_) => "internal_error",
        }
    }
    pub fn http_status(&self) -> u16 {
        match self {
            Self::NotFound(_) => 404,
            Self::Db(_) => 500,
            Self::Posting(e) => e.http_status(),
            _ => 422,
        }
    }
}

impl std::fmt::Display for JournalWorkflowError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound(id) => write!(f, "journal_not_found: {id}"),
            Self::InvalidState { id, current, expected } => {
                write!(f, "invalid_journal_state: {id} is '{current}', expected '{expected}'")
            }
            Self::NotPosted(id) => write!(f, "journal_not_posted: {id}"),
            Self::Posting(e) => write!(f, "posting_error: {e}"),
            Self::Db(e) => write!(f, "db_error: {e}"),
        }
    }
}
impl std::error::Error for JournalWorkflowError {}
impl From<sqlx::Error> for JournalWorkflowError {
    fn from(e: sqlx::Error) -> Self {
        Self::Db(e)
    }
}
impl From<PostingError> for JournalWorkflowError {
    fn from(e: PostingError) -> Self {
        Self::Posting(e)
    }
}

/// The manual-journal workflow service. Owns a pool + a `PostingService` for ledger writes.
#[derive(Clone)]
pub struct JournalWorkflowService {
    db_pool: PgPool,
    posting: PostingService,
}

impl JournalWorkflowService {
    pub fn new(db_pool: PgPool) -> Self {
        Self {
            posting: PostingService::new(db_pool.clone()),
            db_pool,
        }
    }

    /// `draft → pending_approval`. Rejects if the journal is not `draft`.
    pub async fn submit(&self, journal_id: Uuid, company_id: Uuid) -> Result<(), JournalWorkflowError> {
        let res = sqlx::query(
            "UPDATE accounting.journals SET status='pending_approval'::journal_status \
             WHERE id=$1 AND company_id=$2 AND status='draft'::journal_status \
             AND (metadata->>'deleted_at') IS NULL",
        )
        .bind(journal_id)
        .bind(company_id)
        .execute(&self.db_pool)
        .await?;
        if res.rows_affected() == 0 {
            return Err(self.state_error(journal_id, company_id, "draft").await);
        }
        Ok(())
    }

    /// `pending_approval → approved`, then post the journal through `PostingService::post_journal`
    /// (writes the ledger, flips the journal to `posted`). Returns the posting result.
    pub async fn approve(
        &self,
        journal_id: Uuid,
        company_id: Uuid,
        approved_by: Option<Uuid>,
    ) -> Result<PostingResult, JournalWorkflowError> {
        let now = Utc::now();
        let res = sqlx::query(
            "UPDATE accounting.journals SET status='approved'::journal_status, approved_at=$1, approved_by=$2 \
             WHERE id=$3 AND company_id=$4 AND status='pending_approval'::journal_status \
             AND (metadata->>'deleted_at') IS NULL",
        )
        .bind(now)
        .bind(approved_by)
        .bind(journal_id)
        .bind(company_id)
        .execute(&self.db_pool)
        .await?;
        if res.rows_affected() == 0 {
            return Err(self.state_error(journal_id, company_id, "pending_approval").await);
        }
        // Post the now-approved journal to the ledger (atomic; flips status → posted).
        Ok(self.posting.post_journal(journal_id, company_id, approved_by).await?)
    }

    /// `draft|pending_approval → rejected` with a reason. Writes no ledger rows.
    pub async fn reject(
        &self,
        journal_id: Uuid,
        company_id: Uuid,
        reason: String,
        rejected_by: Option<Uuid>,
    ) -> Result<(), JournalWorkflowError> {
        let now = Utc::now();
        let res = sqlx::query(
            "UPDATE accounting.journals SET status='rejected'::journal_status, rejected_at=$1, \
             rejected_by=$2, rejection_reason=$3 \
             WHERE id=$4 AND company_id=$5 AND status IN ('draft','pending_approval') \
             AND (metadata->>'deleted_at') IS NULL",
        )
        .bind(now)
        .bind(rejected_by)
        .bind(&reason)
        .bind(journal_id)
        .bind(company_id)
        .execute(&self.db_pool)
        .await?;
        if res.rows_affected() == 0 {
            return Err(self.state_error(journal_id, company_id, "draft or pending_approval").await);
        }
        Ok(())
    }

    /// `posted → voided`: posts a reversal (debit/credit swapped, linked both ways) through
    /// `PostingService::post`, then stamps the original journal voided. Per R7 the reversal lands in
    /// the current open period, not the original's.
    pub async fn void(
        &self,
        journal_id: Uuid,
        company_id: Uuid,
        voided_by: Option<Uuid>,
        reason: String,
    ) -> Result<PostingResult, JournalWorkflowError> {
        // Must be posted.
        let row = sqlx::query(
            r#"SELECT status::text AS status, currency FROM accounting.journals
               WHERE id=$1 AND company_id=$2 AND (metadata->>'deleted_at') IS NULL"#,
        )
        .bind(journal_id)
        .bind(company_id)
        .fetch_optional(&self.db_pool)
        .await?;
        let row = row.ok_or(JournalWorkflowError::NotFound(journal_id))?;
        let status: String = row.get("status");
        if status != "posted" {
            return Err(JournalWorkflowError::InvalidState {
                id: journal_id,
                current: status,
                expected: "posted",
            });
        }
        let currency: String = row.get("currency");

        // Locate the original posted accounting_post to reverse.
        let orig_post_id: Uuid = sqlx::query_scalar(
            r#"SELECT id FROM accounting.accounting_posts
               WHERE journal_id=$1 AND company_id=$2 AND posting_status='posted'::posting_status
                 AND posting_type='original' LIMIT 1"#,
        )
        .bind(journal_id)
        .bind(company_id)
        .fetch_optional(&self.db_pool)
        .await?
        .ok_or(JournalWorkflowError::NotPosted(journal_id))?;

        // Build + post the reversal. build_reversal_lines swaps debit/credit off the original
        // journal's lines and links both ways.
        let req = PostingRequest {
            company_id,
            branch_id: None,
            source_type: "manual".to_string(),
            source_id: journal_id,
            source_reference: None,
            posting_date: Utc::now().date_naive(),
            currency,
            posting_type: "reversal".to_string(),
            reverses_post_id: Some(orig_post_id),
            description: Some(format!("Void of journal {journal_id}: {reason}")),
            lines: Vec::new(), // populated by build_reversal_lines inside post()
            idempotency_key: Some(format!("void:{journal_id}")),
        };
        let result = self.posting.post(req, voided_by).await?;

        // Stamp the original journal voided.
        let now = Utc::now();
        sqlx::query(
            "UPDATE accounting.journals SET status='voided'::journal_status, is_voided=TRUE, \
             voided_at=$1, voided_by=$2, void_reason=$3 WHERE id=$4 AND company_id=$5",
        )
        .bind(now)
        .bind(voided_by)
        .bind(&reason)
        .bind(journal_id)
        .bind(company_id)
        .execute(&self.db_pool)
        .await?;

        Ok(result)
    }

    /// Resolve a zero-rows-affected UPDATE into a precise error: not-found vs wrong-state.
    async fn state_error(
        &self,
        journal_id: Uuid,
        company_id: Uuid,
        expected: &'static str,
    ) -> JournalWorkflowError {
        let exists: Option<String> = sqlx::query_scalar(
            "SELECT status::text FROM accounting.journals WHERE id=$1 AND company_id=$2 AND (metadata->>'deleted_at') IS NULL",
        )
        .bind(journal_id)
        .bind(company_id)
        .fetch_optional(&self.db_pool)
        .await
        .ok()
        .flatten();
        match exists {
            None => JournalWorkflowError::NotFound(journal_id),
            Some(current) => JournalWorkflowError::InvalidState {
                id: journal_id,
                current,
                expected,
            },
        }
    }
}
