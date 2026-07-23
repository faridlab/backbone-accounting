//! SqlxReportingRepository — SQLx adapter for the reporting port.

use chrono::NaiveDate;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::domain::repositories::reporting_repository::{AccountSumRow, ReportingRepository};

pub struct SqlxReportingRepository {
    pool: PgPool,
}

impl SqlxReportingRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl ReportingRepository for SqlxReportingRepository {
    async fn account_sums(
        &self,
        company_id: Uuid,
        lo: Option<NaiveDate>,
        hi: NaiveDate,
    ) -> anyhow::Result<Vec<AccountSumRow>> {
        let rows = sqlx::query(
            r#"SELECT a.account_type::text AS at, a.account_number AS num, a.name AS name,
                      COALESCE(SUM(l.debit_amount),0) AS dr,
                      COALESCE(SUM(l.credit_amount),0) AS cr
               FROM accounting.accounts a
               LEFT JOIN accounting.ledgers l
                 ON l.account_id = a.id
                AND l.posting_date <= $2
                AND ($3::date IS NULL OR l.posting_date >= $3)
               WHERE a.company_id = $1
                 AND a.is_detail = TRUE
                 AND (a.metadata->>'deleted_at') IS NULL
               GROUP BY a.id, a.account_type, a.account_number, a.name
               ORDER BY a.account_number"#,
        )
        .bind(company_id)
        .bind(hi)
        .bind(lo)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| AccountSumRow {
                account_type: r.get("at"),
                account_number: r.get("num"),
                name: r.get("name"),
                debit: r.get("dr"),
                credit: r.get("cr"),
            })
            .collect())
    }
}
