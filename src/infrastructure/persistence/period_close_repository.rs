//! SqlxPeriodCloseRepository — SQLx adapter for the period-close port.

use chrono::NaiveDate;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::domain::repositories::period_close_repository::{
    PeriodCloseRepository, PeriodRow, PlBalanceRow,
};

pub struct SqlxPeriodCloseRepository {
    pool: PgPool,
}

impl SqlxPeriodCloseRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl PeriodCloseRepository for SqlxPeriodCloseRepository {
    async fn find_period(
        &self,
        period_id: Uuid,
        company_id: Uuid,
    ) -> anyhow::Result<Option<PeriodRow>> {
        let row = sqlx::query(
            "SELECT start_date, end_date, status::text AS status FROM accounting.fiscal_periods WHERE id=$1 AND company_id=$2",
        )
        .bind(period_id)
        .bind(company_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|r| PeriodRow {
            start_date: r.get("start_date"),
            end_date: r.get("end_date"),
            status: r.get("status"),
        }))
    }

    async fn sum_pl_balances(
        &self,
        company_id: Uuid,
        start: NaiveDate,
        end: NaiveDate,
    ) -> anyhow::Result<Vec<PlBalanceRow>> {
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
        .bind(start)
        .bind(end)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|r| PlBalanceRow {
                account_id: r.get("id"),
                account_type: r.get("at"),
                debit: r.get("dr"),
                credit: r.get("cr"),
            })
            .collect())
    }

    async fn mark_closed(&self, period_id: Uuid) -> anyhow::Result<()> {
        sqlx::query("UPDATE accounting.fiscal_periods SET status='closed'::period_status WHERE id=$1")
            .bind(period_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
