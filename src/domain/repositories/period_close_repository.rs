//! PeriodCloseRepository — persistence port for the fiscal-period close.

use async_trait::async_trait;
use chrono::NaiveDate;
use rust_decimal::Decimal;
use uuid::Uuid;

/// Period header for the close guard.
#[derive(Debug, Clone)]
pub struct PeriodRow {
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub status: String,
}

/// One P&L account's debit/credit sums within the period window.
#[derive(Debug, Clone)]
pub struct PlBalanceRow {
    pub account_id: Uuid,
    pub account_type: String,
    pub debit: Decimal,
    pub credit: Decimal,
}

#[async_trait]
pub trait PeriodCloseRepository: Send + Sync {
    async fn find_period(
        &self,
        period_id: Uuid,
        company_id: Uuid,
    ) -> anyhow::Result<Option<PeriodRow>>;

    /// Per-account P&L (revenue/expense/cogs/other) balances within `[start, end]`.
    async fn sum_pl_balances(
        &self,
        company_id: Uuid,
        start: NaiveDate,
        end: NaiveDate,
    ) -> anyhow::Result<Vec<PlBalanceRow>>;

    /// Flip the period status to `closed`.
    async fn mark_closed(&self, period_id: Uuid) -> anyhow::Result<()>;
}
