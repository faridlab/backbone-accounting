//! Bank reconciliation matching — match imported statement lines against unreconciled ledger
//! entries on a bank account, mark the matches reconciled, and persist a Reconciliation +
//! ReconciliationItems.
//!
//! Hand-authored behavior (user-owned; see `metaphor.codegen.yaml`). Proven by
//! `tests/reconciliation_golden_cases.rs`. Matching is greedy by exact signed amount
//! (ledger net = debit − credit for the bank asset account) — timing/partial matches are a
//! later enhancement. Named `bank_reconciliation_service` to avoid the generated CRUD
//! `reconciliation_service`.

use chrono::{NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use uuid::Uuid;

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
    Db(sqlx::Error),
}
impl std::fmt::Display for ReconcileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReconcileError::AccountNotFound(id) => write!(f, "account_not_found: {id}"),
            ReconcileError::Db(e) => write!(f, "db_error: {e}"),
        }
    }
}
impl std::error::Error for ReconcileError {}
impl From<sqlx::Error> for ReconcileError {
    fn from(e: sqlx::Error) -> Self {
        ReconcileError::Db(e)
    }
}

struct BookEntry {
    ledger_id: Uuid,
    amount: Decimal, // signed net (debit − credit)
    reference: Option<String>,
    matched: bool,
}

#[derive(Clone)]
pub struct BankReconciliationService {
    db_pool: PgPool,
}

impl BankReconciliationService {
    pub fn new(db_pool: PgPool) -> Self {
        Self { db_pool }
    }

    pub async fn reconcile(&self, req: ReconcileRequest) -> Result<ReconcileResult, ReconcileError> {
        // Account (denormalized fields for the Reconciliation record).
        let acct = sqlx::query("SELECT account_number, name FROM accounts WHERE id=$1 AND company_id=$2")
            .bind(req.account_id)
            .bind(req.company_id)
            .fetch_optional(&self.db_pool)
            .await?
            .ok_or(ReconcileError::AccountNotFound(req.account_id))?;
        let account_number: String = acct.get("account_number");
        let account_name: String = acct.get("name");

        // Unreconciled book entries in the period.
        let rows = sqlx::query(
            r#"SELECT id, debit_amount, credit_amount, reference
               FROM ledgers
               WHERE company_id=$1 AND account_id=$2 AND is_reconciled=FALSE
                 AND posting_date BETWEEN $3 AND $4
               ORDER BY posting_date, sequence_number"#,
        )
        .bind(req.company_id)
        .bind(req.account_id)
        .bind(req.period_start)
        .bind(req.statement_date)
        .fetch_all(&self.db_pool)
        .await?;
        let mut book: Vec<BookEntry> = rows
            .into_iter()
            .map(|r| {
                let d: Decimal = r.get("debit_amount");
                let c: Decimal = r.get("credit_amount");
                BookEntry { ledger_id: r.get("id"), amount: d - c, reference: r.get("reference"), matched: false }
            })
            .collect();

        // Greedy match: each statement line to the first unmatched book entry of equal amount.
        let mut matched: Vec<(Uuid, StatementLine)> = Vec::new();
        let mut unmatched_stmt: Vec<StatementLine> = Vec::new();
        for line in &req.statement_lines {
            match book.iter_mut().find(|b| !b.matched && b.amount == line.amount) {
                Some(b) => {
                    b.matched = true;
                    matched.push((b.ledger_id, line.clone()));
                }
                None => unmatched_stmt.push(line.clone()),
            }
        }
        let unmatched_book: Vec<&BookEntry> = book.iter().filter(|b| !b.matched).collect();

        // Balances.
        let closing_book_balance: Decimal = sqlx::query_scalar(
            "SELECT COALESCE(SUM(debit_amount - credit_amount),0) FROM ledgers WHERE company_id=$1 AND account_id=$2 AND posting_date <= $3",
        )
        .bind(req.company_id)
        .bind(req.account_id)
        .bind(req.statement_date)
        .fetch_one(&self.db_pool)
        .await?;
        let closing_statement_balance: Decimal = req.statement_lines.iter().map(|l| l.amount).sum();
        let difference = closing_book_balance - closing_statement_balance;
        let is_balanced = unmatched_book.is_empty() && unmatched_stmt.is_empty();

        // Persist (transaction).
        let mut tx = self.db_pool.begin().await?;
        let now = Utc::now();
        let reconciliation_id = Uuid::new_v4();
        let number = format!("REC-{}-{}", req.statement_date.format("%Y%m%d"), &Uuid::new_v4().to_string()[..8]);
        let status = if is_balanced { "completed" } else { "in_progress" };

        sqlx::query(
            r#"INSERT INTO reconciliations
                (id, company_id, reconciliation_number, account_id, account_number, account_name,
                 period_start, period_end, statement_date, opening_book_balance,
                 opening_statement_balance, closing_book_balance, closing_statement_balance,
                 matched_count, difference, is_balanced, status)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,0,0,$10,$11,$12,$13,$14,$15::reconciliation_status)"#,
        )
        .bind(reconciliation_id)
        .bind(req.company_id)
        .bind(&number)
        .bind(req.account_id)
        .bind(&account_number)
        .bind(&account_name)
        .bind(req.period_start)
        .bind(req.period_end)
        .bind(req.statement_date)
        .bind(closing_book_balance)
        .bind(closing_statement_balance)
        .bind(matched.len() as i32)
        .bind(difference)
        .bind(is_balanced)
        .bind(status)
        .execute(&mut *tx)
        .await?;

        let mut item_number = 0i32;
        for (ledger_id, line) in &matched {
            item_number += 1;
            sqlx::query(
                r#"INSERT INTO reconciliation_items
                    (id, reconciliation_id, company_id, item_number, source, ledger_id,
                     statement_reference, status, difference_amount)
                   VALUES ($1,$2,$3,$4,'matched',$5,$6,'matched'::reconciliation_item_status,0)"#,
            )
            .bind(Uuid::new_v4())
            .bind(reconciliation_id)
            .bind(req.company_id)
            .bind(item_number)
            .bind(ledger_id)
            .bind(&line.reference)
            .execute(&mut *tx)
            .await?;

            sqlx::query("UPDATE ledgers SET is_reconciled=TRUE, reconciliation_id=$1, reconciled_at=$2 WHERE id=$3")
                .bind(reconciliation_id)
                .bind(now)
                .bind(ledger_id)
                .execute(&mut *tx)
                .await?;
        }
        for b in &unmatched_book {
            item_number += 1;
            sqlx::query(
                r#"INSERT INTO reconciliation_items
                    (id, reconciliation_id, company_id, item_number, source, ledger_id, status,
                     difference_amount, is_outstanding)
                   VALUES ($1,$2,$3,$4,'book',$5,'unmatched'::reconciliation_item_status,$6,TRUE)"#,
            )
            .bind(Uuid::new_v4())
            .bind(reconciliation_id)
            .bind(req.company_id)
            .bind(item_number)
            .bind(b.ledger_id)
            .bind(b.amount)
            .execute(&mut *tx)
            .await?;
        }
        for line in &unmatched_stmt {
            item_number += 1;
            sqlx::query(
                r#"INSERT INTO reconciliation_items
                    (id, reconciliation_id, company_id, item_number, source, statement_reference,
                     status, difference_amount, is_outstanding)
                   VALUES ($1,$2,$3,$4,'statement',$5,'unmatched'::reconciliation_item_status,$6,TRUE)"#,
            )
            .bind(Uuid::new_v4())
            .bind(reconciliation_id)
            .bind(req.company_id)
            .bind(item_number)
            .bind(&line.reference)
            .bind(line.amount)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;

        Ok(ReconcileResult {
            reconciliation_id,
            matched_count: matched.len() as i32,
            unmatched_book: unmatched_book.len() as i32,
            unmatched_statement: unmatched_stmt.len() as i32,
            closing_book_balance,
            closing_statement_balance,
            difference,
            is_balanced,
        })
    }
}
