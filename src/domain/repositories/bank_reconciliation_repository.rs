//! BankReconciliationRepository — persistence port for bank reconciliation.
//!
//! Owns the reads (bank account, unreconciled book entries, closing balance) and the atomic
//! persist (reconciliation row + matched/unmatched items + ledger reconcile marks). The matching
//! algorithm itself is pure domain logic that stays in `BankReconciliationService`.

use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use uuid::Uuid;

/// One unreconciled book (ledger) entry available to match against.
#[derive(Debug, Clone)]
pub struct BookEntryRow {
    pub ledger_id: Uuid,
    pub amount: Decimal, // signed net (debit − credit)
    pub reference: Option<String>,
}

/// A matched pair (book ledger entry ↔ statement line reference).
#[derive(Debug, Clone)]
pub struct MatchedPair {
    pub ledger_id: Uuid,
    pub statement_reference: Option<String>,
}

/// An unmatched book entry (outstanding).
#[derive(Debug, Clone)]
pub struct UnmatchedBook {
    pub ledger_id: Uuid,
    pub amount: Decimal,
}

/// An unmatched statement line (outstanding / adjustment candidate).
#[derive(Debug, Clone)]
pub struct UnmatchedStatement {
    pub reference: Option<String>,
    pub amount: Decimal,
}

/// Everything needed to atomically persist a reconciliation + its items + ledger marks.
#[derive(Debug, Clone)]
pub struct ReconciliationCommit {
    pub company_id: Uuid,
    pub account_id: Uuid,
    pub account_number: String,
    pub account_name: String,
    pub reconciliation_number: String,
    pub period_start: NaiveDate,
    pub period_end: NaiveDate,
    pub statement_date: NaiveDate,
    pub closing_book_balance: Decimal,
    pub closing_statement_balance: Decimal,
    pub matched_count: i32,
    pub difference: Decimal,
    pub is_balanced: bool,
    pub status: String, // "completed" | "in_progress"
    pub matched: Vec<MatchedPair>,
    pub unmatched_book: Vec<UnmatchedBook>,
    pub unmatched_statement: Vec<UnmatchedStatement>,
    pub now: DateTime<Utc>,
}

#[async_trait]
pub trait BankReconciliationRepository: Send + Sync {
    async fn find_bank_account(
        &self,
        account_id: Uuid,
        company_id: Uuid,
    ) -> anyhow::Result<Option<(String, String)>>; // (account_number, account_name)

    async fn find_unreconciled_book(
        &self,
        company_id: Uuid,
        account_id: Uuid,
        period_start: NaiveDate,
        statement_date: NaiveDate,
    ) -> anyhow::Result<Vec<BookEntryRow>>;

    async fn closing_book_balance(
        &self,
        company_id: Uuid,
        account_id: Uuid,
        as_of: NaiveDate,
    ) -> anyhow::Result<Decimal>;

    /// Atomically persist the reconciliation, its items, and mark matched ledger rows reconciled.
    async fn commit_reconciliation(&self, commit: ReconciliationCommit) -> anyhow::Result<Uuid>;
}
