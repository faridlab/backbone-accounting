//! Fiscal-period close — zero the P&L accounts into Retained Earnings and lock the period.
//!
//! Hand-authored behavior (user-owned; see `metaphor.codegen.yaml`). Proven by
//! `tests/period_close_golden_cases.rs`. Composes the module's own GL-posting contract: it
//! builds a balanced closing entry and posts it through `PostingService`, then flips the
//! `FiscalPeriod` to `closed`.

use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::Serialize;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::application::service::posting_service::{
    PostingError, PostingLine, PostingRequest, PostingService,
};

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
    Db(sqlx::Error),
}
impl std::fmt::Display for PeriodCloseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PeriodCloseError::PeriodNotFound(id) => write!(f, "period_not_found: {id}"),
            PeriodCloseError::AlreadyClosed => write!(f, "period_already_closed"),
            PeriodCloseError::Posting(e) => write!(f, "posting_error: {e}"),
            PeriodCloseError::Db(e) => write!(f, "db_error: {e}"),
        }
    }
}
impl std::error::Error for PeriodCloseError {}
impl From<sqlx::Error> for PeriodCloseError {
    fn from(e: sqlx::Error) -> Self {
        PeriodCloseError::Db(e)
    }
}

#[derive(Clone)]
pub struct PeriodCloseService {
    db_pool: PgPool,
    posting: PostingService,
}

impl PeriodCloseService {
    pub fn new(db_pool: PgPool) -> Self {
        Self {
            posting: PostingService::new(db_pool.clone()),
            db_pool,
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
        // Period must exist and be open.
        let period = sqlx::query(
            "SELECT start_date, end_date, status::text AS status FROM accounting.fiscal_periods WHERE id=$1 AND company_id=$2",
        )
        .bind(period_id)
        .bind(company_id)
        .fetch_optional(&self.db_pool)
        .await?
        .ok_or(PeriodCloseError::PeriodNotFound(period_id))?;
        let start_date: NaiveDate = period.get("start_date");
        let end_date: NaiveDate = period.get("end_date");
        let status: String = period.get("status");
        if status == "closed" || status == "locked" {
            return Err(PeriodCloseError::AlreadyClosed);
        }

        // Per-account P&L balances within the period.
        let rows = sqlx::query(
            r#"SELECT l.account_id AS id, a.account_type::text AS at,
                      COALESCE(SUM(l.debit_amount),0) AS dr, COALESCE(SUM(l.credit_amount),0) AS cr
               FROM accounting.ledgers l
               JOIN accounting.accounts a ON a.id = l.account_id
               WHERE l.company_id=$1
                 AND l.posting_date BETWEEN $2 AND $3
                 AND a.account_type::text IN ('revenue','other_income','expense','cogs','other_expense')
               GROUP BY l.account_id, a.account_type"#,
        )
        .bind(company_id)
        .bind(start_date)
        .bind(end_date)
        .fetch_all(&self.db_pool)
        .await?;

        // Build closing lines: debit revenue balances, credit expense balances.
        let mut lines: Vec<PostingLine> = Vec::new();
        let mut revenue_total = Decimal::ZERO;
        let mut expense_total = Decimal::ZERO;
        for r in &rows {
            let at: String = r.get("at");
            let dr: Decimal = r.get("dr");
            let cr: Decimal = r.get("cr");
            let account_id: Uuid = r.get("id");
            match at.as_str() {
                "revenue" | "other_income" => {
                    let bal = cr - dr; // credit-normal balance
                    if bal != Decimal::ZERO {
                        revenue_total += bal;
                        lines.push(close_line(account_id, bal, Decimal::ZERO)); // debit to zero it
                    }
                }
                _ => {
                    let bal = dr - cr; // debit-normal balance (expense/cogs/other_expense)
                    if bal != Decimal::ZERO {
                        expense_total += bal;
                        lines.push(close_line(account_id, Decimal::ZERO, bal)); // credit to zero it
                    }
                }
            }
        }

        let net_income = revenue_total - expense_total;

        // Nothing to close.
        if lines.is_empty() {
            self.mark_closed(period_id).await?;
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
        let mut req = PostingRequest::original(company_id, "manual", period_id, end_date);
        req.description = Some("Period close".to_string());
        req.lines = lines;
        let result = self
            .posting
            .post(req, None)
            .await
            .map_err(PeriodCloseError::Posting)?;

        // Lock the period.
        self.mark_closed(period_id).await?;

        Ok(PeriodCloseResult {
            period_id,
            net_income,
            closing_post_id: Some(result.post_id),
            closing_journal_id: Some(result.journal_id),
        })
    }

    async fn mark_closed(&self, period_id: Uuid) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE accounting.fiscal_periods SET status='closed'::period_status WHERE id=$1")
            .bind(period_id)
            .execute(&self.db_pool)
            .await?;
        Ok(())
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
