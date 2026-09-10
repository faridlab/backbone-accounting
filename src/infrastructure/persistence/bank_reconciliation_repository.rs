//! SqlxBankReconciliationRepository — SQLx adapter for the bank-reconciliation port.
//!
//! Tenancy (ADR-0029): the module carries no tenancy of its own — the composing service's
//! tenancy decorator owns org scoping. The port's `company_id` lanes are the documented legacy
//! twin: they keep their shapes for unstripped callers, but no statement here keys on a tenant
//! column. Reads and standalone writes ride the request-dedicated connection when the
//! composing service bound one (carrying the decorator's fence variables), plainly on the pool
//! otherwise; the atomic commit relays the AMBIENT org scope onto its transaction
//! (`org_scope::bind_org_scope_on`) before any statement. An undecorated deployment gets an
//! unfenced module.

use chrono::NaiveDate;
use rust_decimal::Decimal;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use backbone_orm::org_scope;
// The multi-row and scalar read twins live only in the legacy `company_scope` module. Their
// connection discipline is what this adapter needs — request-dedicated connection when the
// composing service bound one, plain pool otherwise. The helper's legacy task-local branch is
// never taken: this module sets no legacy scope of its own (ADR-0029).
use backbone_orm::company_scope::{fetch_all_rows_scoped, fetch_one_scalar_scoped};

use crate::domain::repositories::bank_reconciliation_repository::{
    BankReconciliationRepository, BookEntryRow, ReconciliationCommit,
};

pub struct SqlxBankReconciliationRepository {
    pool: PgPool,
}

impl SqlxBankReconciliationRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl BankReconciliationRepository for SqlxBankReconciliationRepository {
    async fn find_bank_account(
        &self,
        account_id: Uuid,
        _company_id: Uuid,
    ) -> anyhow::Result<Option<(String, String)>> {
        let row = org_scope::fetch_optional_row_scoped(
            &self.pool,
            sqlx::query("SELECT account_number, name FROM accounting.accounts WHERE id=$1")
                .bind(account_id),
        )
        .await?;
        Ok(row.map(|r| (r.get("account_number"), r.get("name"))))
    }

    async fn find_unreconciled_book(
        &self,
        _company_id: Uuid,
        account_id: Uuid,
        period_start: NaiveDate,
        statement_date: NaiveDate,
    ) -> anyhow::Result<Vec<BookEntryRow>> {
        let rows = fetch_all_rows_scoped(
            &self.pool,
            sqlx::query(
                r#"SELECT id, debit_amount, credit_amount, reference
                   FROM accounting.ledgers
                   WHERE account_id=$1 AND is_reconciled=FALSE
                     AND posting_date BETWEEN $2 AND $3
                   ORDER BY posting_date, sequence_number"#,
            )
            .bind(account_id)
            .bind(period_start)
            .bind(statement_date),
        )
        .await?;
        Ok(rows
            .iter()
            .map(|r| {
                let d: Decimal = r.get("debit_amount");
                let c: Decimal = r.get("credit_amount");
                BookEntryRow {
                    ledger_id: r.get("id"),
                    amount: d - c,
                    reference: r.get("reference"),
                }
            })
            .collect())
    }

    async fn closing_book_balance(
        &self,
        _company_id: Uuid,
        account_id: Uuid,
        as_of: NaiveDate,
    ) -> anyhow::Result<Decimal> {
        let bal: Decimal = fetch_one_scalar_scoped(
            &self.pool,
            sqlx::query_scalar(
                "SELECT COALESCE(SUM(debit_amount - credit_amount),0) FROM accounting.ledgers \
                 WHERE account_id=$1 AND posting_date <= $2",
            )
            .bind(account_id)
            .bind(as_of),
        )
        .await?;
        Ok(bal)
    }

    async fn commit_reconciliation(&self, c: ReconciliationCommit) -> anyhow::Result<Uuid> {
        let mut tx = self.pool.begin().await?;
        // Tenancy posture (ADR-0029): the module owns no scoping column — the composing
        // service's tenancy decorator does. Relay the AMBIENT request scope onto this
        // transaction when the caller bound one; an undecorated deployment has no ambient
        // scope and skips this entirely.
        if let Some(scope) = org_scope::current_org_scope() {
            org_scope::bind_org_scope_on(&mut tx, &scope).await?;
        }
        let reconciliation_id = Uuid::new_v4();

        sqlx::query(
            r#"INSERT INTO accounting.reconciliations
                (id, reconciliation_number, account_id, account_number, account_name,
                 period_start, period_end, statement_date, opening_book_balance,
                 opening_statement_balance, closing_book_balance, closing_statement_balance,
                 matched_count, difference, is_balanced, status)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,0,0,$9,$10,$11,$12,$13,$14::reconciliation_status)"#,
        )
        .bind(reconciliation_id)
        .bind(&c.reconciliation_number)
        .bind(c.account_id)
        .bind(&c.account_number)
        .bind(&c.account_name)
        .bind(c.period_start)
        .bind(c.period_end)
        .bind(c.statement_date)
        .bind(c.closing_book_balance)
        .bind(c.closing_statement_balance)
        .bind(c.matched_count)
        .bind(c.difference)
        .bind(c.is_balanced)
        .bind(&c.status)
        .execute(&mut *tx)
        .await?;

        let mut item_number = 0i32;
        for m in &c.matched {
            item_number += 1;
            sqlx::query(
                r#"INSERT INTO accounting.reconciliation_items
                    (id, reconciliation_id, item_number, source, ledger_id,
                     statement_reference, status, difference_amount)
                   VALUES ($1,$2,$3,'matched',$4,$5,'matched'::reconciliation_item_status,0)"#,
            )
            .bind(Uuid::new_v4())
            .bind(reconciliation_id)
            .bind(item_number)
            .bind(m.ledger_id)
            .bind(&m.statement_reference)
            .execute(&mut *tx)
            .await?;

            sqlx::query(
                "UPDATE accounting.ledgers SET is_reconciled=TRUE, reconciliation_id=$1, reconciled_at=$2 WHERE id=$3",
            )
            .bind(reconciliation_id)
            .bind(c.now)
            .bind(m.ledger_id)
            .execute(&mut *tx)
            .await?;
        }
        for b in &c.unmatched_book {
            item_number += 1;
            sqlx::query(
                r#"INSERT INTO accounting.reconciliation_items
                    (id, reconciliation_id, item_number, source, ledger_id, status,
                     difference_amount, is_outstanding)
                   VALUES ($1,$2,$3,'book',$4,'unmatched'::reconciliation_item_status,$5,TRUE)"#,
            )
            .bind(Uuid::new_v4())
            .bind(reconciliation_id)
            .bind(item_number)
            .bind(b.ledger_id)
            .bind(b.amount)
            .execute(&mut *tx)
            .await?;
        }
        for s in &c.unmatched_statement {
            item_number += 1;
            sqlx::query(
                r#"INSERT INTO accounting.reconciliation_items
                    (id, reconciliation_id, item_number, source, statement_reference,
                     status, difference_amount, is_outstanding)
                   VALUES ($1,$2,$3,'statement',$4,'unmatched'::reconciliation_item_status,$5,TRUE)"#,
            )
            .bind(Uuid::new_v4())
            .bind(reconciliation_id)
            .bind(item_number)
            .bind(&s.reference)
            .bind(s.amount)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(reconciliation_id)
    }
}
