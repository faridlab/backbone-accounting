//! Manual-journal approval workflow — the draft → submit → approve → post → void lifecycle.
//!
//! Hand-authored (user-owned; survives regeneration). Application orchestration over two ports:
//! `JournalWorkflowRepository` for the journal status machine, and `PostingService` (backed by
//! `PostingRepository`) for the actual ledger write. No `sqlx` / `PgPool` here.
//!
//! See `docs/business-flows/gl-posting.md` (manual-journal flow) and BRD §3.

use std::sync::Arc;

use chrono::Utc;
use uuid::Uuid;

use crate::domain::gl_posting::{PostingError, PostingRequest};
use crate::domain::repositories::journal_workflow_repository::JournalWorkflowRepository;
use crate::domain::repositories::posting_repository::PostingRepository;
use crate::application::service::posting_service::PostingService;

/// Typed workflow failure. `code()` is the stable error string.
#[derive(Debug)]
pub enum JournalWorkflowError {
    NotFound(Uuid),
    InvalidState { id: Uuid, current: String, expected: &'static str },
    NotPosted(Uuid),
    Posting(PostingError),
    Internal(String),
}

impl JournalWorkflowError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::NotFound(_) => "journal_not_found",
            Self::InvalidState { .. } => "invalid_journal_state",
            Self::NotPosted(_) => "journal_not_posted",
            Self::Posting(_) => "posting_error",
            Self::Internal(_) => "internal_error",
        }
    }
    pub fn http_status(&self) -> u16 {
        match self {
            Self::NotFound(_) => 404,
            Self::Internal(_) => 500,
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
            Self::Internal(e) => write!(f, "internal_error: {e}"),
        }
    }
}
impl std::error::Error for JournalWorkflowError {}
impl From<PostingError> for JournalWorkflowError {
    fn from(e: PostingError) -> Self {
        Self::Posting(e)
    }
}

fn internal(e: anyhow::Error) -> JournalWorkflowError {
    JournalWorkflowError::Internal(e.to_string())
}

/// The manual-journal workflow service.
#[derive(Clone)]
pub struct JournalWorkflowService {
    workflow: Arc<dyn JournalWorkflowRepository>,
    posting: PostingService,
}

impl JournalWorkflowService {
    pub fn new(posting_repo: Arc<dyn PostingRepository>, workflow: Arc<dyn JournalWorkflowRepository>) -> Self {
        Self {
            posting: PostingService::new(posting_repo),
            workflow,
        }
    }

    /// `draft → pending_approval`. Rejects if the journal is not `draft`.
    pub async fn submit(&self, journal_id: Uuid, company_id: Uuid) -> Result<(), JournalWorkflowError> {
        let ok = self.workflow.submit(journal_id, company_id).await.map_err(internal)?;
        if !ok {
            return Err(self.state_error(journal_id, company_id, "draft").await);
        }
        Ok(())
    }

    /// `pending_approval → approved`, then post the journal through `PostingService` (writes the
    /// ledger, flips the journal to `posted`). Returns the posting result.
    pub async fn approve(
        &self,
        journal_id: Uuid,
        company_id: Uuid,
        approved_by: Option<Uuid>,
    ) -> Result<crate::domain::gl_posting::PostingResult, JournalWorkflowError> {
        let now = Utc::now();
        let ok = self
            .workflow
            .approve(journal_id, company_id, approved_by, now)
            .await
            .map_err(internal)?;
        if !ok {
            return Err(self.state_error(journal_id, company_id, "pending_approval").await);
        }
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
        let ok = self
            .workflow
            .reject(journal_id, company_id, &reason, rejected_by, now)
            .await
            .map_err(internal)?;
        if !ok {
            return Err(self.state_error(journal_id, company_id, "draft or pending_approval").await);
        }
        Ok(())
    }

    /// `posted → voided`: posts a reversal through `PostingService`, then stamps the journal voided.
    pub async fn void(
        &self,
        journal_id: Uuid,
        company_id: Uuid,
        voided_by: Option<Uuid>,
        reason: String,
    ) -> Result<crate::domain::gl_posting::PostingResult, JournalWorkflowError> {
        let Some(status) = self.workflow.find_status(journal_id, company_id).await.map_err(internal)? else {
            return Err(JournalWorkflowError::NotFound(journal_id));
        };
        if status.status != "posted" {
            return Err(JournalWorkflowError::InvalidState {
                id: journal_id,
                current: status.status,
                expected: "posted",
            });
        }

        let Some(orig_post_id) = self.workflow.original_post(journal_id, company_id).await.map_err(internal)? else {
            return Err(JournalWorkflowError::NotPosted(journal_id));
        };

        let req = PostingRequest {
            company_id,
            branch_id: None,
            source_type: "manual".to_string(),
            source_id: journal_id,
            source_reference: None,
            posting_date: Utc::now().date_naive(),
            currency: status.currency,
            posting_type: "reversal".to_string(),
            reverses_post_id: Some(orig_post_id),
            description: Some(format!("Void of journal {journal_id}: {reason}")),
            lines: Vec::new(), // populated by the reversal source read inside post()
            idempotency_key: Some(format!("void:{journal_id}")),
        };
        let result = self.posting.post(req, voided_by).await?;

        let now = Utc::now();
        self.workflow
            .mark_voided(journal_id, company_id, voided_by, &reason, now)
            .await
            .map_err(internal)?;
        Ok(result)
    }

    /// Resolve a falsy transition into a precise error: not-found vs wrong-state.
    async fn state_error(
        &self,
        journal_id: Uuid,
        company_id: Uuid,
        expected: &'static str,
    ) -> JournalWorkflowError {
        match self.workflow.current_status(journal_id, company_id).await {
            Ok(None) => JournalWorkflowError::NotFound(journal_id),
            Ok(Some(current)) => JournalWorkflowError::InvalidState {
                id: journal_id,
                current,
                expected,
            },
            Err(e) => internal(e),
        }
    }
}
