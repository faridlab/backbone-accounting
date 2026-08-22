//! ReportingRepository — persistence port for financial-statement reads.
//!
//! Pure reads; no report write path exists (reports are computed on the fly, never stored —
//! the `financial_statement` entity is not the compute target). All shaping (normal-side
//! signing, A=L+E tying, tree rollups, aging buckets) is pure domain logic that stays in
//! `ReportingService`.

use async_trait::async_trait;
use chrono::NaiveDate;
use rust_decimal::Decimal;
use uuid::Uuid;

/// One detail account's debit/credit activity within a date window.
#[derive(Debug, Clone)]
pub struct AccountSumRow {
    pub account_id: Uuid,
    pub account_type: String,
    pub account_number: String,
    pub name: String,
    pub debit: Decimal,
    pub credit: Decimal,
}

/// One node of the chart of accounts (detail or header) — the tree the sums roll up through.
#[derive(Debug, Clone)]
pub struct AccountNodeRow {
    pub id: Uuid,
    pub parent_id: Option<Uuid>,
    pub account_number: String,
    pub name: String,
    pub account_type: String,
    pub level: i32,
    pub is_header: bool,
    pub is_detail: bool,
}

/// One chronological ledger line inside a GL window (the `ledgers` projection carries the
/// denormalized account/journal info and the materialized running balance).
#[derive(Debug, Clone)]
pub struct GlLineRow {
    pub account_id: Uuid,
    pub account_number: String,
    pub account_name: String,
    pub journal_number: String,
    pub transaction_date: NaiveDate,
    pub posting_date: NaiveDate,
    pub description: String,
    pub reference: Option<String>,
    pub currency: String,
    pub debit: Decimal,
    pub credit: Decimal,
    pub balance_before: Decimal,
    pub balance_after: Decimal,
    pub party_type: Option<String>,
    pub party_id: Option<Uuid>,
    pub source_type: Option<String>,
    pub source_reference: Option<String>,
    pub is_reconciled: bool,
}

/// One AR/AP line for a party (from `journal_lines`, the residual id space), with the
/// line's residual as of the report date.
#[derive(Debug, Clone)]
pub struct PartyLedgerRow {
    pub line_id: Uuid,
    pub account_id: Uuid,
    pub account_number: String,
    pub account_name: String,
    pub journal_number: String,
    pub transaction_date: NaiveDate,
    pub description: Option<String>,
    pub debit: Decimal,
    pub credit: Decimal,
    pub currency: String,
    pub residual: Decimal,
}

/// One open (residual > 0 as of the report date) AR/AP line, party-stamped — the grain the
/// aged report buckets.
#[derive(Debug, Clone)]
pub struct AgedItemRow {
    pub party_type: String,
    pub party_id: Uuid,
    pub account_id: Uuid,
    pub account_number: String,
    pub transaction_date: NaiveDate,
    pub source_reference: Option<String>,
    pub currency: String,
    pub residual: Decimal,
}

#[async_trait]
pub trait ReportingRepository: Send + Sync {
    /// Per-detail-account debit/credit sums. `lo = None` → since inception; otherwise `>= lo`.
    /// `<= hi` always.
    async fn account_sums(
        &self,
        company_id: Uuid,
        lo: Option<NaiveDate>,
        hi: NaiveDate,
    ) -> anyhow::Result<Vec<AccountSumRow>>;

    /// The full chart of accounts (headers + details) for tree rollups.
    async fn account_directory(&self, company_id: Uuid) -> anyhow::Result<Vec<AccountNodeRow>>;

    /// Chronological ledger lines within `[lo, hi]` (lo = None → since inception), optionally
    /// narrowed to one account. Ordered by account number, then posting date, then sequence.
    async fn gl_lines(
        &self,
        company_id: Uuid,
        account_id: Option<Uuid>,
        lo: Option<NaiveDate>,
        hi: NaiveDate,
        limit: i64,
        offset: i64,
    ) -> anyhow::Result<Vec<GlLineRow>>;

    /// Every party-stamped line for one party through `as_of`, each with its residual as of
    /// that date (partials dated after `as_of` don't count yet).
    async fn party_ledger_lines(
        &self,
        company_id: Uuid,
        party_type: &str,
        party_id: Uuid,
        as_of: NaiveDate,
    ) -> anyhow::Result<Vec<PartyLedgerRow>>;

    /// Open AR or AP items across all parties as of `as_of`, for the given account subtype
    /// (`accounts_receivable` / `accounts_payable`). Residual as of `as_of`.
    async fn aged_open_items(
        &self,
        company_id: Uuid,
        account_subtype: &str,
        as_of: NaiveDate,
    ) -> anyhow::Result<Vec<AgedItemRow>>;
}
