//! PeriodCloseRepository — persistence port for the fiscal-period close.
//!
//! Tenancy (ADR-0029): the `company_id` params are the documented legacy twin — the port keeps
//! its shapes so unstripped callers compile and run unchanged; the adapter never keys a
//! statement on them (the ambient org scope scopes every statement instead, and on a decorated
//! deployment the decorator's fence makes a cross-tenant id miss).

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
    /// When the period was last reopened; `None` until its first reopen. It
    /// names the close cycle, so each close after a reopen posts its own
    /// closing entry.
    pub reopened_at: Option<chrono::DateTime<chrono::Utc>>,
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
    ) -> anyhow::Result<Option<PeriodRow>>;

    /// Per-account P&L (revenue/expense/cogs/other) balances within `[start, end]`.
    async fn sum_pl_balances(
        &self,
        start: NaiveDate,
        end: NaiveDate,
    ) -> anyhow::Result<Vec<PlBalanceRow>>;

    /// Claim the period for a close: `open` or `adjusting` → `closing`, in one
    /// conditional write. Returns the status it left, or `None` when the period
    /// was not in a closable state at the moment of the write, so two closes
    /// racing for one period cannot both proceed to post a closing entry.
    async fn begin_close(&self, period_id: Uuid, actor: Option<Uuid>) -> anyhow::Result<Option<String>>;

    /// Finish a claimed close: `closing` → `closed`. `false` when the period
    /// was no longer `closing`.
    async fn finish_close(&self, period_id: Uuid, actor: Option<Uuid>) -> anyhow::Result<bool>;

    /// Give a failed close's claim back: `closing` → the status it left.
    async fn abort_close(&self, period_id: Uuid, back_to: &str) -> anyhow::Result<()>;

    /// Make a closed period final: `closed` → `locked`, recording who and why.
    /// `false` when the period was not `closed`.
    async fn lock(&self, period_id: Uuid, actor: Option<Uuid>, reason: &str) -> anyhow::Result<bool>;

    /// Reopen a closed period: `closed` → `open`, recording who and why.
    /// `false` when the period was not `closed`.
    async fn reopen(&self, period_id: Uuid, actor: Option<Uuid>, reason: &str) -> anyhow::Result<bool>;
}
