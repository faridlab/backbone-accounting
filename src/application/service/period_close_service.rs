//! Fiscal-period close — zero the P&L accounts into Retained Earnings and lock the period.
//!
//! Hand-authored (user-owned; see `metaphor.codegen.yaml`). Application orchestration over the
//! `PeriodCloseRepository` port (+ `PostingService` for the closing entry) — no `sqlx`/`PgPool`
//! here. Proven by `tests/period_close_golden_cases.rs`.

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
    Posting(PostingError),
    Internal(String),
}
impl std::fmt::Display for PeriodCloseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PeriodCloseError::PeriodNotFound(id) => write!(f, "period_not_found: {id}"),
            PeriodCloseError::AlreadyClosed => write!(f, "period_already_closed"),
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
    pub fn new(posting_repo: Arc<dyn crate::domain::repositories::posting_repository::PostingRepository>, repo: Arc<dyn PeriodCloseRepository>) -> Self {
        Self {
            posting: PostingService::new(posting_repo),
            repo,
        }
    }

    /// Close `period_id`: post a closing entry that zeroes revenue/expense into
    /// `retained_earnings_account_id`, then mark the period closed.
    pub async fn close_period(
        &self,
        company_id: Uuid,
        period_id: Uuid,
        retained_earnings_account_id: Uuid,
    ) -> Result<PeriodCloseResult, PeriodCloseError> {
        let Some(period) = self.repo.find_period(period_id, company_id).await.map_err(internal)? else {
            return Err(PeriodCloseError::PeriodNotFound(period_id));
        };
        if period.status == "closed" || period.status == "locked" {
            return Err(PeriodCloseError::AlreadyClosed);
        }

        let rows = self
            .repo
            .sum_pl_balances(company_id, period.start_date, period.end_date)
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
            self.repo.mark_closed(period_id).await.map_err(internal)?;
            return Ok(PeriodCloseResult {
                period_id,
                net_income,
                closing_post_id: None,
                closing_journal_id: None,
            });
        }

        // Balancing line to Retained Earnings (equity, credit-normal): profit → credit, loss → debit.
        if net_income > Decimal::ZERO {
            lines.push(close_line(retained_earnings_account_id, Decimal::ZERO, net_income));
        } else if net_income < Decimal::ZERO {
            lines.push(close_line(retained_earnings_account_id, -net_income, Decimal::ZERO));
        }

        // Post the closing entry (period still open) through the GL-posting contract.
        let mut req = PostingRequest::original(company_id, "manual", period_id, period.end_date);
        req.description = Some("Period close".to_string());
        req.lines = lines;
        let result = self.posting.post(req, None).await?;

        self.repo.mark_closed(period_id).await.map_err(internal)?;

        Ok(PeriodCloseResult {
            period_id,
            net_income,
            closing_post_id: Some(result.post_id),
            closing_journal_id: Some(result.journal_id),
        })
    }
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
