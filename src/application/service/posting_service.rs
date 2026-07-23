//! GL-posting service — the inbound port of the GL-posting contract.
//!
//! Hand-authored behavior (NOT generated). This is the **application** orchestration layer: it
//! loads data via the `PostingRepository` port, validates with the pure domain rules
//! (`domain::services::posting_rules`), commits atomically back through the port, and publishes
//! contract events. It holds NO `PgPool` and runs NO `sqlx` — all persistence lives in
//! `infrastructure::persistence::posting_repository::SqlxPostingRepository`.
//!
//! Implements double-entry posting per `docs/erp/gl-posting-contract.md` and the golden cases in
//! `docs/business-flows/golden-cases.md`.
//!
//! This file is user-owned (see `metaphor.codegen.yaml`) and survives regeneration.

use std::sync::Arc;

use chrono::{Datelike, Utc};
use rust_decimal::Decimal;

use crate::domain::repositories::posting_repository::{
    FailedPost, LedgerEntryInput, ManualJournalCommit, PostingCommit, PostingRepository,
    PostingWrite,
};
use crate::domain::services::posting_rules;

// The domain contract types are imported (and re-exported for call-site compatibility) from the
// domain layer. `pub use` also brings them into local scope.
pub use crate::domain::gl_posting::{
    AccountingPostFailed, AccountingPostPosted, PostingError, PostingEvent, PostingEventSink,
    PostingLine, PostingRequest, PostingResult,
};

/// Map a persistence (`anyhow`) failure into the posting error type.
fn internal(e: anyhow::Error) -> PostingError {
    PostingError::Internal(e.to_string())
}

/// Default sink — emits structured tracing events.
pub struct LoggingSink;

impl PostingEventSink for LoggingSink {
    fn publish(&self, event: PostingEvent) {
        match &event {
            PostingEvent::AccountingPostPosted(e) => tracing::info!(
                target: "accounting.events", post_id = %e.post_id, journal_id = %e.journal_id,
                source_type = %e.source_type, "AccountingPostPosted"
            ),
            PostingEvent::AccountingPostFailed(e) => tracing::warn!(
                target: "accounting.events", source_type = %e.source_type, code = %e.error_code,
                "AccountingPostFailed"
            ),
        }
    }
}

/// The GL-posting service. Depends on the `PostingRepository` port (+ an event sink).
#[derive(Clone)]
pub struct PostingService {
    repo: Arc<dyn PostingRepository>,
    sink: Arc<dyn PostingEventSink>,
}

impl PostingService {
    pub fn new(repo: Arc<dyn PostingRepository>) -> Self {
        Self { repo, sink: Arc::new(LoggingSink) }
    }

    /// Construct with a custom event sink (real bus adapter or a test recorder).
    pub fn with_sink(repo: Arc<dyn PostingRepository>, sink: Arc<dyn PostingEventSink>) -> Self {
        Self { repo, sink }
    }

    /// Record a balanced posting. Idempotent on (company, source_type, source_id, posting_type)
    /// — or on `idempotency_key` when the producer sets one.
    pub async fn post(
        &self,
        mut req: PostingRequest,
        posted_by: Option<uuid::Uuid>,
    ) -> Result<PostingResult, PostingError> {
        // Idempotency: return the existing posted entry for the same source identity.
        if let Some((post_id, journal_id)) = self
            .repo
            .find_existing_post(
                req.company_id,
                &req.source_type,
                req.source_id,
                &req.posting_type,
                req.idempotency_key.as_deref(),
            )
            .await
            .map_err(internal)?
        {
            return Ok(PostingResult {
                post_id,
                journal_id,
                posting_status: "posted".to_string(),
                idempotent_reuse: true,
            });
        }

        // Reversal derives its (swapped) lines from the original journal.
        let reverses_journal_id = if req.posting_type == "reversal" {
            Some(self.build_reversal_lines(&mut req).await?)
        } else {
            None
        };

        // Validate; on failure record a failed AccountingPost (audit) and return the code.
        if let Err(e) = self.validate(&req).await {
            let _ = self.record_failed(&req, &e).await;
            return Err(e);
        }

        let now = Utc::now();
        let fiscal_period_id = self
            .repo
            .find_period_id(req.company_id, req.posting_date)
            .await
            .map_err(internal)?;

        let write = PostingWrite {
            company_id: req.company_id,
            branch_id: req.branch_id,
            source_type: req.source_type.clone(),
            source_id: req.source_id,
            source_reference: req.source_reference.clone(),
            posting_date: req.posting_date,
            fiscal_period_id,
            fiscal_year: req.posting_date.year(),
            fiscal_month: req.posting_date.month() as i32,
            currency: req.currency.clone(),
            posting_type: req.posting_type.clone(),
            reverses_post_id: req.reverses_post_id,
            reverses_journal_id,
            description: req.description.clone(),
            idempotency_key: req.idempotency_key.clone(),
            posted_by,
            now,
            lines: req.lines.clone(),
        };

        let commit = self.repo.commit_posting(write).await.map_err(internal)?;
        self.emit(&req, &commit);

        Ok(PostingResult {
            post_id: commit.post_id,
            journal_id: commit.journal_id,
            posting_status: "posted".to_string(),
            idempotent_reuse: commit.reused,
        })
    }

    /// Post an existing **approved** manual journal to the ledger. Idempotent: a journal already
    /// `posted` returns its existing post. Used by the journal approval workflow (approve → post).
    pub async fn post_journal(
        &self,
        journal_id: uuid::Uuid,
        company_id: uuid::Uuid,
        posted_by: Option<uuid::Uuid>,
    ) -> Result<PostingResult, PostingError> {
        let Some(ctx) = self
            .repo
            .find_manual_journal_for_post(journal_id, company_id)
            .await
            .map_err(internal)?
        else {
            return Err(PostingError::Conflict(format!("journal {journal_id} not found")));
        };

        if ctx.status == "posted" {
            let existing = self
                .repo
                .existing_post_for_journal(journal_id, company_id)
                .await
                .map_err(internal)?;
            return Ok(PostingResult {
                post_id: existing.unwrap_or_default(),
                journal_id,
                posting_status: "posted".to_string(),
                idempotent_reuse: true,
            });
        }
        if ctx.status != "approved" {
            return Err(PostingError::Conflict(format!(
                "journal {journal_id} is '{}', must be 'approved' to post",
                ctx.status
            )));
        }

        // Validate the stored lines (pure rules against loaded accounts + period-open flag).
        let lines: Vec<PostingLine> = ctx.lines.iter().map(|(_, l)| l.clone()).collect();
        let line_ids: Vec<uuid::Uuid> = lines.iter().map(|l| l.account_id).collect();
        let val_req = PostingRequest {
            company_id,
            branch_id: ctx.branch_id,
            source_type: ctx.source_type.clone(),
            source_id: ctx.source_id,
            source_reference: None,
            posting_date: ctx.posting_date,
            currency: ctx.currency.clone(),
            posting_type: "original".to_string(),
            reverses_post_id: None,
            description: ctx.description.clone(),
            lines: lines.clone(),
            idempotency_key: None,
        };
        if let Err(e) = self.validate_lines(&val_req, &line_ids).await {
            let _ = self.record_failed(&val_req, &e).await;
            return Err(e);
        }

        let now = Utc::now();
        let commit_input = ManualJournalCommit {
            journal_id,
            company_id,
            branch_id: ctx.branch_id,
            journal_number: ctx.journal_number,
            posting_date: ctx.posting_date,
            fiscal_period_id: ctx.fiscal_period_id,
            fiscal_year: ctx.fiscal_year,
            fiscal_month: ctx.fiscal_month,
            currency: ctx.currency,
            source_type: ctx.source_type.clone(),
            source_id: ctx.source_id,
            description: ctx.description.clone(),
            posted_by,
            now,
            lines: ctx
                .lines
                .into_iter()
                .map(|(id, line)| LedgerEntryInput { journal_line_id: id, line })
                .collect(),
        };

        let commit = self.repo.commit_manual_journal(commit_input).await.map_err(internal)?;

        self.sink.publish(PostingEvent::AccountingPostPosted(AccountingPostPosted {
            post_id: commit.post_id,
            journal_id: commit.journal_id,
            company_id,
            source_type: ctx.source_type,
            source_id: ctx.source_id,
            total_debit: commit.total_debit,
            total_credit: commit.total_credit,
            occurred_at: now,
        }));

        Ok(PostingResult {
            post_id: commit.post_id,
            journal_id: commit.journal_id,
            posting_status: "posted".to_string(),
            idempotent_reuse: commit.reused,
        })
    }

    async fn validate(&self, req: &PostingRequest) -> Result<(), PostingError> {
        let ids: Vec<uuid::Uuid> = req.lines.iter().map(|l| l.account_id).collect();
        self.validate_lines(req, &ids).await
    }

    async fn validate_lines(
        &self,
        req: &PostingRequest,
        ids: &[uuid::Uuid],
    ) -> Result<(), PostingError> {
        let rows = self.repo.find_postable_accounts(req.company_id, ids).await.map_err(internal)?;
        let accounts: std::collections::HashMap<uuid::Uuid, _> =
            rows.into_iter().map(|a| (a.id, a)).collect();
        let period_closed = self.repo.is_period_closed(req.company_id, req.posting_date).await.map_err(internal)?;
        posting_rules::validate(&req.lines, &accounts, period_closed)
    }

    // ---- helpers ------------------------------------------------------------

    /// Emit the posted event (only on a fresh commit, not an idempotent reuse).
    fn emit(&self, req: &PostingRequest, commit: &PostingCommit) {
        if commit.reused {
            return;
        }
        self.sink.publish(PostingEvent::AccountingPostPosted(AccountingPostPosted {
            post_id: commit.post_id,
            journal_id: commit.journal_id,
            company_id: req.company_id,
            source_type: req.source_type.clone(),
            source_id: req.source_id,
            total_debit: commit.total_debit,
            total_credit: commit.total_credit,
            occurred_at: Utc::now(),
        }));
    }

    /// Load the original journal's swapped reversal lines into `req.lines`; return the original
    /// journal id (for the reversal links).
    async fn build_reversal_lines(
        &self,
        req: &mut PostingRequest,
    ) -> Result<uuid::Uuid, PostingError> {
        let orig_post_id = req
            .reverses_post_id
            .ok_or_else(|| PostingError::Conflict("reversal requires reverses_post_id".into()))?;
        let source = self
            .repo
            .find_reversal_source(orig_post_id, req.company_id)
            .await
            .map_err(internal)?
            .ok_or_else(|| PostingError::Conflict("original posting not found or not posted".into()))?;
        req.lines = source.lines;
        Ok(source.journal_id)
    }

    async fn record_failed(&self, req: &PostingRequest, err: &PostingError) -> Result<(), PostingError> {
        let total_debit: Decimal = req.lines.iter().map(|l| l.debit).sum();
        let total_credit: Decimal = req.lines.iter().map(|l| l.credit).sum();
        let now = Utc::now();
        self.repo
            .record_failed(FailedPost {
                company_id: req.company_id,
                branch_id: req.branch_id,
                source_type: req.source_type.clone(),
                source_id: req.source_id,
                source_reference: req.source_reference.clone(),
                posting_type: req.posting_type.clone(),
                currency: req.currency.clone(),
                total_debit,
                total_credit,
                failed_at: now,
                error_code: err.code().to_string(),
                error_message: err.to_string(),
            })
            .await
            .map_err(internal)?;
        self.sink.publish(PostingEvent::AccountingPostFailed(AccountingPostFailed {
            company_id: req.company_id,
            source_type: req.source_type.clone(),
            source_id: req.source_id,
            error_code: err.code().to_string(),
            error_message: err.to_string(),
            occurred_at: now,
        }));
        Ok(())
    }
}
