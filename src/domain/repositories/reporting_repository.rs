//! ReportingRepository — persistence port for financial-statement reads.
//!
//! One read serves Trial Balance / Balance Sheet / Income Statement: per-detail-account debit &
//! credit sums over a date window. All report shaping (normal-side signing, A=L+E tying) is pure
//! domain logic that stays in `ReportingService`.

use async_trait::async_trait;
use chrono::NaiveDate;
use rust_decimal::Decimal;
use uuid::Uuid;

/// One detail account's debit/credit activity within a date window.
#[derive(Debug, Clone)]
pub struct AccountSumRow {
    pub account_type: String,
    pub account_number: String,
    pub name: String,
    pub debit: Decimal,
    pub credit: Decimal,
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
}
