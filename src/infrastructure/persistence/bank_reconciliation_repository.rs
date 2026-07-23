//! SqlxBankReconciliationRepository — SQLx adapter for the bank-reconciliation port.

use chrono::NaiveDate;
use rust_decimal::Decimal;
use sqlx::{PgPool, Row};
use uuid::Uuid;

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
        company_id: Uuid,
    ) -> anyhow::Result<Option<(String, String)>> {
        let row = sqlx::query(
            "SELECT account_number, name FROM accounting.accounts WHERE id=$1 AND company_id=$2",
        )
        .bind(account_id)
        .bind(company_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|r| (r.get("account_number"), r.get("name"))))
    }

    async fn find_unreconciled_book(
        &self,
        company_id: Uuid,
        account_id: Uuid,
        period_start: NaiveDate,
        statement_date: NaiveDate,
    ) -> anyhow::Result<Vec<BookEntryRow>> {
        let rows = sqlx::query(
            r#"SELECT id, debit_amount, credit_amount, reference
               FROM accounting.ledgers
               WHERE company_id=$1 AND account_id=$2 AND is_reconciled=FALSE
                 AND posting_date BETWEEN $3 AND $4
               ORDER BY posting_date, sequence_number"#,
        )
        .bind(company_id)
        .bind(account_id)
        .bind(period_start)
        .bind(statement_date)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
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
        company_id: Uuid,
        account_id: Uuid,
        as_of: NaiveDate,
    ) -> anyhow::Result<Decimal> {
        let bal: Decimal = sqlx::query_scalar(
            "SELECT COALESCE(SUM(debit_amount - credit_amount),0) FROM accounting.ledgers WHERE company_id=$1 AND account_id=$2 AND posting_date <= $3",
        )
        .bind(company_id)
        .bind(account_id)
        .bind(as_of)
        .fetch_one(&self.pool)
        .await?;
        Ok(bal)
    }

    async fn commit_reconciliation(&self, c: ReconciliationCommit) -> anyhow::Result<Uuid> {
        let mut tx = self.pool.begin().await?;
        let reconciliation_id = Uuid::new_v4();

        sqlx::query(
            r#"INSERT INTO accounting.reconciliations
                (id, company_id, reconciliation_number, account_id, account_number, account_name,
                 period_start, period_end, statement_date, opening_book_balance,
                 opening_statement_balance, closing_book_balance, closing_statement_balance,
                 matched_count, difference, is_balanced, status)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,0,0,$10,$11,$12,$13,$14,$15::reconciliation_status)"#,
        )
        .bind(reconciliation_id)
        .bind(c.company_id)
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
                    (id, reconciliation_id, company_id, item_number, source, ledger_id,
                     statement_reference, status, difference_amount)
                   VALUES ($1,$2,$3,$4,'matched',$5,$6,'matched'::reconciliation_item_status,0)"#,
            )
            .bind(Uuid::new_v4())
            .bind(reconciliation_id)
            .bind(c.company_id)
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
                    (id, reconciliation_id, company_id, item_number, source, ledger_id, status,
                     difference_amount, is_outstanding)
                   VALUES ($1,$2,$3,$4,'book',$5,'unmatched'::reconciliation_item_status,$6,TRUE)"#,
            )
            .bind(Uuid::new_v4())
            .bind(reconciliation_id)
            .bind(c.company_id)
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
                    (id, reconciliation_id, company_id, item_number, source, statement_reference,
                     status, difference_amount, is_outstanding)
                   VALUES ($1,$2,$3,$4,'statement',$5,'unmatched'::reconciliation_item_status,$6,TRUE)"#,
            )
            .bind(Uuid::new_v4())
            .bind(reconciliation_id)
            .bind(c.company_id)
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
