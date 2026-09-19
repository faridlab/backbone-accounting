//! Bank reconciliation matching — match imported statement lines against unreconciled ledger
//! entries on a bank account, mark the matches reconciled, and persist a Reconciliation +
//! ReconciliationItems.
//!
//! Hand-authored (user-owned; see `metaphor.codegen.yaml`). Application orchestration over the
//! `BankReconciliationRepository` port — no `sqlx`/`PgPool` here. Matching is greedy by exact
//! signed amount; timing/partial matches are a later enhancement. Proven by
//! `tests/reconciliation_golden_cases.rs`.
//!
//! Tenancy (ADR-0029): the `ReconcileRequest::company_id` threaded through the port calls is
//! the legacy twin — kept so unstripped callers compile and run unchanged; nothing here keys a
//! statement on it (the adapter scopes by the ambient org scope).

use std::sync::Arc;

use chrono::{NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::repositories::bank_reconciliation_repository::{
    BankReconciliationRepository, MatchedPair, ReconciliationCommit, UnmatchedBook,
    UnmatchedStatement,
};

/// One line imported from a bank statement.
#[derive(Debug, Clone, Deserialize)]
pub struct StatementLine {
    pub date: NaiveDate,
    pub amount: Decimal, // signed: receipts +, payments −
    pub reference: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ReconcileRequest {
    pub company_id: Uuid,
    pub account_id: Uuid,
    pub period_start: NaiveDate,
    pub period_end: NaiveDate,
    pub statement_date: NaiveDate,
    pub statement_lines: Vec<StatementLine>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReconcileResult {
    pub reconciliation_id: Uuid,
    pub matched_count: i32,
    pub unmatched_book: i32,
    pub unmatched_statement: i32,
    pub closing_book_balance: Decimal,
    pub closing_statement_balance: Decimal,
    pub difference: Decimal,
    pub is_balanced: bool,
}

#[derive(Debug)]
pub enum ReconcileError {
    AccountNotFound(Uuid),
    Internal(String),
}
impl std::fmt::Display for ReconcileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReconcileError::AccountNotFound(id) => write!(f, "account_not_found: {id}"),
            ReconcileError::Internal(e) => write!(f, "internal_error: {e}"),
        }
    }
}
impl std::error::Error for ReconcileError {}

fn internal(e: anyhow::Error) -> ReconcileError {
    ReconcileError::Internal(e.to_string())
}

/// The comparable form of a reference, or `None` when there is nothing to
/// compare. Case and surrounding whitespace are not part of a reference:
/// the same cheque number reaches the book and the statement through
/// different hands.
fn references_pair(reference: Option<&str>) -> Option<String> {
    let trimmed = reference?.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_lowercase())
}

/// One book entry during matching (mutable `matched` flag).
struct BookEntry {
    ledger_id: Uuid,
    amount: Decimal,
    reference: Option<String>,
    matched: bool,
}

#[derive(Clone)]
pub struct BankReconciliationService {
    repo: Arc<dyn BankReconciliationRepository>,
}

impl BankReconciliationService {
    pub fn new(repo: Arc<dyn BankReconciliationRepository>) -> Self {
        Self { repo }
    }

    pub async fn reconcile(
        &self,
        req: ReconcileRequest,
    ) -> Result<ReconcileResult, ReconcileError> {
        let Some((account_number, account_name)) = self
            .repo
            .find_bank_account(req.account_id)
            .await
            .map_err(internal)?
        else {
            return Err(ReconcileError::AccountNotFound(req.account_id));
        };

        let rows = self
            .repo
            .find_unreconciled_book(
                req.account_id,
                req.period_start,
                req.statement_date,
            )
            .await
            .map_err(internal)?;
        let mut book: Vec<BookEntry> = rows
            .into_iter()
            .map(|r| BookEntry {
                ledger_id: r.ledger_id,
                amount: r.amount,
                reference: r.reference,
                matched: false,
            })
            .collect();

        // Greedy match, reference first: a statement line takes the unmatched book
        // entry that carries the same reference and the same amount, and only
        // falls back to the first unmatched entry of that amount when no
        // reference pairs them.
        //
        // Amount alone is not an identity. Two receipts of the same value in one
        // period are ordinary, and matching them by arrival order pairs the wrong
        // ledger row with the wrong statement line: the totals still balance, so
        // nothing looks wrong, while each row is stamped reconciled against a
        // line it has nothing to do with. The reference is the one field that
        // says which is which.
        let mut matched: Vec<MatchedPair> = Vec::new();
        let mut unmatched_stmt: Vec<UnmatchedStatement> = Vec::new();
        for line in &req.statement_lines {
            let by_reference = references_pair(line.reference.as_deref()).and_then(|wanted| {
                book.iter()
                    .position(|b| {
                        !b.matched
                            && b.amount == line.amount
                            && references_pair(b.reference.as_deref())
                                .is_some_and(|have| have == wanted)
                    })
            });
            let hit = match by_reference {
                Some(i) => Some(i),
                None => book
                    .iter()
                    .position(|b| !b.matched && b.amount == line.amount),
            };
            match hit.map(|i| &mut book[i]) {
                Some(b) => {
                    b.matched = true;
                    matched.push(MatchedPair {
                        ledger_id: b.ledger_id,
                        statement_reference: line.reference.clone(),
                    });
                }
                None => unmatched_stmt.push(UnmatchedStatement {
                    reference: line.reference.clone(),
                    amount: line.amount,
                }),
            }
        }
        let unmatched_book: Vec<UnmatchedBook> = book
            .iter()
            .filter(|b| !b.matched)
            .map(|b| UnmatchedBook {
                ledger_id: b.ledger_id,
                amount: b.amount,
            })
            .collect();

        let closing_book_balance = self
            .repo
            .closing_book_balance(req.account_id, req.statement_date)
            .await
            .map_err(internal)?;
        let closing_statement_balance: Decimal = req.statement_lines.iter().map(|l| l.amount).sum();
        let difference = closing_book_balance - closing_statement_balance;
        let is_balanced = unmatched_book.is_empty() && unmatched_stmt.is_empty();

        // Capture counts before the vectors move into the commit.
        let matched_count = matched.len() as i32;
        let unmatched_book_count = unmatched_book.len() as i32;
        let unmatched_statement_count = unmatched_stmt.len() as i32;

        let number = format!(
            "REC-{}-{}",
            req.statement_date.format("%Y%m%d"),
            &Uuid::new_v4().to_string()[..8]
        );
        let status = if is_balanced {
            "completed"
        } else {
            "in_progress"
        };

        let commit = ReconciliationCommit {
            company_id: req.company_id,
            account_id: req.account_id,
            account_number,
            account_name,
            reconciliation_number: number,
            period_start: req.period_start,
            period_end: req.period_end,
            statement_date: req.statement_date,
            closing_book_balance,
            closing_statement_balance,
            matched_count,
            difference,
            is_balanced,
            status: status.to_string(),
            matched,
            unmatched_book,
            unmatched_statement: unmatched_stmt,
            now: Utc::now(),
        };
        let reconciliation_id = self
            .repo
            .commit_reconciliation(commit)
            .await
            .map_err(internal)?;

        Ok(ReconcileResult {
            reconciliation_id,
            matched_count,
            unmatched_book: unmatched_book_count,
            unmatched_statement: unmatched_statement_count,
            closing_book_balance,
            closing_statement_balance,
            difference,
            is_balanced,
        })
    }
}
