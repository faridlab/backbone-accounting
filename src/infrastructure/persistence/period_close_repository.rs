//! SqlxPeriodCloseRepository — SQLx adapter for the period-close port.
//!
//! Tenancy (ADR-0029): the module carries no tenancy of its own — the composing service's
//! tenancy decorator owns org scoping. The port's `company_id` lanes are the documented legacy
//! twin: they keep their shapes for unstripped callers, but no statement here keys on a tenant
//! column. Every statement rides the request-dedicated connection when the composing service
//! bound one (carrying the decorator's fence variables), plainly on the pool otherwise. An
//! undecorated deployment gets an unfenced module.

use chrono::NaiveDate;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use backbone_orm::company_scope::fetch_all_rows_scoped;
// The optional-row read twin rides the org-scope module; its connection discipline is the
// request-dedicated connection when the composing service bound one, plain pool otherwise.
// The legacy task-local branch is never taken: this module sets no legacy scope of its own
// (ADR-0029).
use backbone_orm::org_scope;

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
        _company_id: Uuid,
    ) -> anyhow::Result<Option<PeriodRow>> {
        let row = org_scope::fetch_optional_row_scoped(
            &self.pool,
            sqlx::query(
                "SELECT start_date, end_date, status::text AS status FROM accounting.fiscal_periods WHERE id=$1",
            )
            .bind(period_id),
        )
        .await?;
        Ok(row.map(|r| PeriodRow {
            start_date: r.get("start_date"),
            end_date: r.get("end_date"),
            status: r.get("status"),
        }))
    }

    async fn sum_pl_balances(
        &self,
        _company_id: Uuid,
        start: NaiveDate,
        end: NaiveDate,
    ) -> anyhow::Result<Vec<PlBalanceRow>> {
        // Multi-row read twin: see the module docs — request-dedicated connection when the
        // composing service bound one, plain pool otherwise (ADR-0029).
        let rows = fetch_all_rows_scoped(
            &self.pool,
            sqlx::query(
                r#"SELECT l.account_id AS id, a.account_type::text AS at,
                          COALESCE(SUM(l.debit_amount),0) AS dr, COALESCE(SUM(l.credit_amount),0) AS cr
                   FROM accounting.ledgers l
                   JOIN accounting.accounts a ON a.id = l.account_id
                   WHERE l.posting_date BETWEEN $1 AND $2
                     AND a.account_type::text IN ('revenue','other_income','expense','cogs','other_expense')
                   GROUP BY l.account_id, a.account_type"#,
            )
            .bind(start)
            .bind(end),
        )
        .await?;
        Ok(rows
            .iter()
            .map(|r| PlBalanceRow {
                account_id: r.get("id"),
                account_type: r.get("at"),
                debit: r.get("dr"),
                credit: r.get("cr"),
            })
            .collect())
    }

    async fn mark_closed(&self, period_id: Uuid) -> anyhow::Result<()> {
        org_scope::execute_scoped(
            &self.pool,
            sqlx::query(
                "UPDATE accounting.fiscal_periods SET status='closed'::period_status WHERE id=$1",
            )
            .bind(period_id),
        )
        .await?;
        Ok(())
    }
}
