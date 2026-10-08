//! Fiscal-period close — zero the P&L accounts into Retained Earnings and lock the period.
//!
//! Hand-authored (user-owned; see `metaphor.codegen.yaml`). Application orchestration over the
//! `PeriodCloseRepository` port (+ `PostingService` for the closing entry) — no `sqlx`/`PgPool`
//! here. Proven by `tests/period_close_golden_cases.rs`.
//!
//! Tenancy (ADR-0029): the `company_id` parameter is the legacy twin — kept so unstripped
//! callers compile and run unchanged; nothing here keys a statement on it (the adapters scope
//! by the ambient org scope).

use std::sync::Arc;

use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::Serialize;
use uuid::Uuid;

use crate::application::service::posting_service::PostingService;
use crate::domain::gl_posting::{PostingError, PostingLine, PostingRequest};
use crate::domain::repositories::period_close_repository::PeriodCloseRepository;

#[derive(Debug, Clone, Serialize)]
pub struct PeriodCloseResult {
    pub period_id: Uuid,
    pub net_income: Decimal,
    /// None when the period had no P&L activity (nothing to close).
    pub closing_post_id: Option<Uuid>,
    pub closing_journal_id: Option<Uuid>,
}

#[derive(Debug)]
pub enum PeriodCloseError {
    PeriodNotFound(Uuid),
    AlreadyClosed,
    /// Another close of the same period holds the claim.
    CloseInProgress,
    /// Lock and reopen act on a closed period only.
    NotClosed { status: String },
    /// A locked period is final: it is neither locked again nor reopened.
    Locked,
    /// Lock and reopen must say why.
    ReasonRequired,
    Posting(PostingError),
    Internal(String),
}
impl std::fmt::Display for PeriodCloseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PeriodCloseError::PeriodNotFound(id) => write!(f, "period_not_found: {id}"),
            PeriodCloseError::AlreadyClosed => write!(f, "period_already_closed"),
            PeriodCloseError::CloseInProgress => write!(f, "period_close_in_progress"),
            PeriodCloseError::NotClosed { status } => {
                write!(f, "period_not_closed: the period is {status}; only a closed period can be locked or reopened")
            }
            PeriodCloseError::Locked => write!(f, "period_locked: a locked period is final"),
            PeriodCloseError::ReasonRequired => write!(f, "reason_required: say why the period is locked or reopened"),
            PeriodCloseError::Posting(e) => write!(f, "posting_error: {e}"),
            PeriodCloseError::Internal(e) => write!(f, "internal_error: {e}"),
        }
    }
}
impl std::error::Error for PeriodCloseError {}
impl From<PostingError> for PeriodCloseError {
    fn from(e: PostingError) -> Self {
        PeriodCloseError::Posting(e)
    }
}

fn internal(e: anyhow::Error) -> PeriodCloseError {
    PeriodCloseError::Internal(e.to_string())
}

#[derive(Clone)]
pub struct PeriodCloseService {
    repo: Arc<dyn PeriodCloseRepository>,
    posting: PostingService,
}

impl PeriodCloseService {
    pub fn new(
        posting_repo: Arc<dyn crate::domain::repositories::posting_repository::PostingRepository>,
        repo: Arc<dyn PeriodCloseRepository>,
    ) -> Self {
        Self {
            posting: PostingService::new(posting_repo),
            repo,
        }
    }

    /// Arm the budget-control consult on the inner posting service (consuming
    /// builder; `None` leaves it unarmed). The closing entry zeroes P&L
    /// accounts against their normal balance, so control is usually left
    /// unarmed here; the builder exists so hosts can choose otherwise.
    pub fn with_budget_control_if_set(
        mut self,
        port: Option<std::sync::Arc<dyn crate::domain::repositories::BudgetControlPort>>,
    ) -> Self {
        self.posting = self.posting.with_budget_control_if_set(port);
        self
    }

    /// Close `period_id`: post a closing entry that zeroes revenue/expense into
    /// `retained_earnings_account_id`, then mark the period closed.
    pub async fn close_period(
        &self,
        period_id: Uuid,
        retained_earnings_account_id: Uuid,
    ) -> Result<PeriodCloseResult, PeriodCloseError> {
        self.close_period_as(period_id, retained_earnings_account_id, None).await
    }

    /// [`close_period`](Self::close_period), recording `actor` as the person
    /// who started and finished the close.
    ///
    /// The period is claimed (`closing`) before the closing entry is posted,
    /// so a second close of the same period is refused instead of posting a
    /// second closing entry. If posting fails the claim is given back and the
    /// period returns to the status it had.
    pub async fn close_period_as(
        &self,
        period_id: Uuid,
        retained_earnings_account_id: Uuid,
        actor: Option<Uuid>,
    ) -> Result<PeriodCloseResult, PeriodCloseError> {
        let period = self.period(period_id).await?;
        match period.status.as_str() {
            "closed" | "locked" => return Err(PeriodCloseError::AlreadyClosed),
            "closing" => return Err(PeriodCloseError::CloseInProgress),
            _ => {}
        }
        let Some(prior_status) = self.repo.begin_close(period_id, actor).await.map_err(internal)? else {
            // Lost the claim between the read and the write: say what won.
            return Err(match self.period(period_id).await?.status.as_str() {
                "closing" => PeriodCloseError::CloseInProgress,
                _ => PeriodCloseError::AlreadyClosed,
            });
        };

        match self.post_closing_entry(period_id, &period, retained_earnings_account_id).await {
            Ok(result) => {
                if !self.repo.finish_close(period_id, actor).await.map_err(internal)? {
                    return Err(PeriodCloseError::Internal(
                        "the period left `closing` while its close was posting".into(),
                    ));
                }
                Ok(result)
            }
            Err(e) => {
                self.repo.abort_close(period_id, &prior_status).await.map_err(internal)?;
                Err(e)
            }
        }
    }

    /// Make a closed period final. A locked period refuses every posting and
    /// cannot be reopened.
    pub async fn lock_period(
        &self,
        period_id: Uuid,
        actor: Option<Uuid>,
        reason: &str,
    ) -> Result<(), PeriodCloseError> {
        let reason = required_reason(reason)?;
        self.require_closed(period_id).await?;
        if self.repo.lock(period_id, actor, reason).await.map_err(internal)? {
            return Ok(());
        }
        // Another move won between the read and the write.
        self.require_closed(period_id).await?;
        Err(PeriodCloseError::Internal("the period could not be locked".into()))
    }

    /// Reopen a closed period so it takes postings again. The closing entry
    /// already posted stays: a later close posts only what changed since.
    pub async fn reopen_period(
        &self,
        period_id: Uuid,
        actor: Option<Uuid>,
        reason: &str,
    ) -> Result<(), PeriodCloseError> {
        let reason = required_reason(reason)?;
        self.require_closed(period_id).await?;
        if self.repo.reopen(period_id, actor, reason).await.map_err(internal)? {
            return Ok(());
        }
        self.require_closed(period_id).await?;
        Err(PeriodCloseError::Internal("the period could not be reopened".into()))
    }

    async fn period(
        &self,
        period_id: Uuid,
    ) -> Result<crate::domain::repositories::period_close_repository::PeriodRow, PeriodCloseError> {
        self.repo
            .find_period(period_id)
            .await
            .map_err(internal)?
            .ok_or(PeriodCloseError::PeriodNotFound(period_id))
    }

    async fn require_closed(&self, period_id: Uuid) -> Result<(), PeriodCloseError> {
        match self.period(period_id).await?.status.as_str() {
            "closed" => Ok(()),
            "locked" => Err(PeriodCloseError::Locked),
            other => Err(PeriodCloseError::NotClosed { status: other.to_string() }),
        }
    }

    /// Post the entry that zeroes the period's P&L into retained earnings.
    /// `closing_post_id` is `None` when the period had no P&L activity.
    async fn post_closing_entry(
        &self,
        period_id: Uuid,
        period: &crate::domain::repositories::period_close_repository::PeriodRow,
        retained_earnings_account_id: Uuid,
    ) -> Result<PeriodCloseResult, PeriodCloseError> {
        let rows = self
            .repo
            .sum_pl_balances(period.start_date, period.end_date)
            .await
            .map_err(internal)?;

        // Build closing lines: debit revenue balances, credit expense balances.
        let mut lines: Vec<PostingLine> = Vec::new();
        let mut revenue_total = Decimal::ZERO;
        let mut expense_total = Decimal::ZERO;
        for r in &rows {
            match r.account_type.as_str() {
                "revenue" | "other_income" => {
                    let bal = r.credit - r.debit; // credit-normal balance
                    if bal != Decimal::ZERO {
                        revenue_total += bal;
                        lines.push(close_line(r.account_id, bal, Decimal::ZERO));
                    }
                }
                _ => {
                    let bal = r.debit - r.credit; // debit-normal balance (expense/cogs/other_expense)
                    if bal != Decimal::ZERO {
                        expense_total += bal;
                        lines.push(close_line(r.account_id, Decimal::ZERO, bal));
                    }
                }
            }
        }

        let net_income = revenue_total - expense_total;

        if lines.is_empty() {
            return Ok(PeriodCloseResult {
                period_id,
                net_income,
                closing_post_id: None,
                closing_journal_id: None,
            });
        }

        // Balancing line to Retained Earnings (equity, credit-normal): profit → credit, loss → debit.
        if net_income > Decimal::ZERO {
            lines.push(close_line(
                retained_earnings_account_id,
                Decimal::ZERO,
                net_income,
            ));
        } else if net_income < Decimal::ZERO {
            lines.push(close_line(
                retained_earnings_account_id,
                -net_income,
                Decimal::ZERO,
            ));
        }

        // Post the closing entry (period still open) through the GL-posting contract.
        // The close posts through the shared posting request, which still carries the legacy
        // company twin for unstripped consumers. Read it from the ambient org scope — the same
        // source the module's repositories echo — rather than taking it from the caller.
        let company_id = backbone_orm::org_scope::current_org_scope()
            .and_then(|s| s.legacy_company_id())
            .unwrap_or_default();
        let mut req = PostingRequest::original(company_id, "manual", period_id, period.end_date);
        req.description = Some("Period close".to_string());
        // One closing entry per close cycle. Without a key the post dedups on
        // its source (this period), so the close after a reopen would get the
        // first close's entry back and leave the reopened P&L unclosed. The
        // cycle is named by the last reopen, so a retry within one cycle still
        // collapses onto the entry it already posted.
        req.idempotency_key = Some(format!(
            "period-close:{period_id}:{}",
            period
                .reopened_at
                .map(|t| t.timestamp_micros().to_string())
                .unwrap_or_else(|| "initial".to_string())
        ));
        req.lines = lines;
        let result = self.posting.post(req, None).await?;

        Ok(PeriodCloseResult {
            period_id,
            net_income,
            closing_post_id: Some(result.post_id),
            closing_journal_id: Some(result.journal_id),
        })
    }
}

/// A lock or reopen reason, trimmed; blank is refused.
fn required_reason(reason: &str) -> Result<&str, PeriodCloseError> {
    let reason = reason.trim();
    if reason.is_empty() {
        return Err(PeriodCloseError::ReasonRequired);
    }
    Ok(reason)
}

fn close_line(account_id: Uuid, debit: Decimal, credit: Decimal) -> PostingLine {
    PostingLine {
        account_id,
        debit,
        credit,
        party_type: None,
        party_id: None,
        cost_center_id: None,
        project_id: None,
        department_id: None,
        description: Some("Period close".to_string()),
    }
}
