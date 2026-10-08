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

// The multi-row and optional-row read twins ride the org-scope module; their connection
// discipline is the request-dedicated connection when the composing service bound one,
// plain pool otherwise, no scope invented. The legacy task-local branch is never taken:
// this module sets no legacy scope of its own (ADR-0029).
use backbone_orm::org_scope;
use backbone_orm::org_scope::fetch_all_rows_scoped;

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
    ) -> anyhow::Result<Option<PeriodRow>> {
        let row = org_scope::fetch_optional_row_scoped(
            &self.pool,
            sqlx::query(
                "SELECT start_date, end_date, status::text AS status, reopened_at FROM accounting.fiscal_periods WHERE id=$1",
            )
            .bind(period_id),
        )
        .await?;
        Ok(row.map(|r| PeriodRow {
            start_date: r.get("start_date"),
            end_date: r.get("end_date"),
            status: r.get("status"),
            reopened_at: r.get("reopened_at"),
        }))
    }

    async fn sum_pl_balances(
        &self,
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

    // Every status move below is one conditional UPDATE: the `WHERE status`
    // clause is the check, so a move that lost a race matches no row instead
    // of overwriting the winner. The transition trigger
    // (migrations/20261008100000_fiscal_period_status_verbs) holds any other
    // writer to the same moves.

    async fn begin_close(&self, period_id: Uuid, actor: Option<Uuid>) -> anyhow::Result<Option<String>> {
        let row = org_scope::fetch_optional_row_scoped(
            &self.pool,
            sqlx::query(
                r#"WITH prior AS (
                       SELECT status::text AS status FROM accounting.fiscal_periods WHERE id = $1
                   )
                   UPDATE accounting.fiscal_periods
                      SET status = 'closing'::period_status,
                          closing_started_at = now(),
                          closing_started_by = $2
                    WHERE id = $1 AND status IN ('open', 'adjusting')
                RETURNING (SELECT status FROM prior) AS prior_status"#,
            )
            .bind(period_id)
            .bind(actor),
        )
        .await?;
        Ok(row.map(|r| r.get("prior_status")))
    }

    async fn finish_close(&self, period_id: Uuid, actor: Option<Uuid>) -> anyhow::Result<bool> {
        let done = org_scope::execute_scoped(
            &self.pool,
            sqlx::query(
                r#"UPDATE accounting.fiscal_periods
                      SET status = 'closed'::period_status, closed_at = now(), closed_by = $2
                    WHERE id = $1 AND status = 'closing'"#,
            )
            .bind(period_id)
            .bind(actor),
        )
        .await?;
        Ok(done.rows_affected() == 1)
    }

    async fn abort_close(&self, period_id: Uuid, back_to: &str) -> anyhow::Result<()> {
        org_scope::execute_scoped(
            &self.pool,
            sqlx::query(
                r#"UPDATE accounting.fiscal_periods
                      SET status = $2::period_status, closing_started_at = NULL, closing_started_by = NULL
                    WHERE id = $1 AND status = 'closing'"#,
            )
            .bind(period_id)
            .bind(back_to),
        )
        .await?;
        Ok(())
    }

    async fn lock(&self, period_id: Uuid, actor: Option<Uuid>, reason: &str) -> anyhow::Result<bool> {
        let done = org_scope::execute_scoped(
            &self.pool,
            sqlx::query(
                r#"UPDATE accounting.fiscal_periods
                      SET status = 'locked'::period_status, locked_at = now(), locked_by = $2, lock_reason = $3
                    WHERE id = $1 AND status = 'closed'"#,
            )
            .bind(period_id)
            .bind(actor)
            .bind(reason),
        )
        .await?;
        Ok(done.rows_affected() == 1)
    }

    async fn reopen(&self, period_id: Uuid, actor: Option<Uuid>, reason: &str) -> anyhow::Result<bool> {
        let done = org_scope::execute_scoped(
            &self.pool,
            sqlx::query(
                r#"UPDATE accounting.fiscal_periods
                      SET status = 'open'::period_status, reopened_at = now(), reopened_by = $2, reopen_reason = $3
                    WHERE id = $1 AND status = 'closed'"#,
            )
            .bind(period_id)
            .bind(actor)
            .bind(reason),
        )
        .await?;
        Ok(done.rows_affected() == 1)
    }
}
