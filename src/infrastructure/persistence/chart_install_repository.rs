//! SQLx adapter for the chart install engine's persistence port.
//!
//! Hand-authored (user-owned; see `metaphor.codegen.yaml`). Enum parameters bind as
//! text with an explicit `::enum_type` cast — the sqlx runtime-tokio-rustls build has
//! no compile-time knowledge of the database enum OIDs, so an uncast string bind fails.

use crate::domain::chart_dataset::ChartDataset;
use crate::domain::repositories::chart_install_repository::{
    ChartInstallRepository, ChartAccountRow, OverlappingAccount, UpsertOutcome,
};
use sqlx::Row;
use uuid::Uuid;

#[derive(Default)]
pub struct SqlxChartInstallRepository;

impl SqlxChartInstallRepository {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl ChartInstallRepository for SqlxChartInstallRepository {
    async fn company_has_postings(
        &self,
        tx: &mut sqlx::PgConnection,
        company_id: Uuid,
    ) -> anyhow::Result<bool> {
        let has: bool = sqlx::query_scalar(
            "SELECT EXISTS (
                SELECT 1 FROM accounting.journal_lines WHERE company_id = $1
            )",
        )
        .bind(company_id)
        .fetch_one(&mut *tx)
        .await?;
        Ok(has)
    }

    async fn overlapping_accounts(
        &self,
        tx: &mut sqlx::PgConnection,
        company_id: Uuid,
        dataset: &ChartDataset,
    ) -> anyhow::Result<Vec<OverlappingAccount>> {
        let numbers: Vec<String> = dataset.accounts.iter().map(|a| a.number.clone()).collect();
        let codes: Vec<String> = dataset.accounts.iter().map(|a| a.code.clone()).collect();

        let rows = sqlx::query(
            r#"SELECT id, account_number, account_code, chart_code
                 FROM accounting.accounts
                WHERE company_id = $1
                  AND (metadata->>'deleted_at') IS NULL
                  AND (account_number = ANY($2) OR account_code = ANY($3))"#,
        )
        .bind(company_id)
        .bind(&numbers)
        .bind(&codes)
        .fetch_all(&mut *tx)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| OverlappingAccount {
                id: r.get("id"),
                account_number: r.get("account_number"),
                account_code: r.get("account_code"),
                chart_code: r.get("chart_code"),
            })
            .collect())
    }

    async fn upsert_account(
        &self,
        tx: &mut sqlx::PgConnection,
        row: &ChartAccountRow,
    ) -> anyhow::Result<UpsertOutcome> {
        // `pre` reads the row's pre-statement soft-delete state; `up` reports whether
        // the statement took the INSERT arm (xmax = 0) or the ON CONFLICT UPDATE arm.
        // Engine-owned fields only — name, status, balances, budget settings
        // keep whatever the company last set them to.
        let result = sqlx::query(
            r#"WITH pre AS (
                   SELECT (metadata->>'deleted_at') IS NOT NULL AS was_deleted
                     FROM accounting.accounts WHERE id = $1
               ), up AS (
                   INSERT INTO accounting.accounts
                       (id, company_id, account_number, account_code, name,
                        account_type, account_subtype, normal_balance,
                        parent_id, level, path, is_header, is_detail,
                        currency, is_reconcilable, sort_order,
                        chart_code, chart_version)
                   VALUES ($1, $2, $3, $4, $5,
                           $6::account_type, $7::account_subtype, $8::normal_balance,
                           $9, $10, $11, $12, $13,
                           $14, $15, $16,
                           $17, $18)
                   ON CONFLICT (id) DO UPDATE SET
                       account_number   = EXCLUDED.account_number,
                       account_code     = EXCLUDED.account_code,
                       account_type     = EXCLUDED.account_type,
                       account_subtype  = EXCLUDED.account_subtype,
                       normal_balance   = EXCLUDED.normal_balance,
                       parent_id        = EXCLUDED.parent_id,
                       level            = EXCLUDED.level,
                       path             = EXCLUDED.path,
                       is_header        = EXCLUDED.is_header,
                       is_detail        = EXCLUDED.is_detail,
                       currency         = EXCLUDED.currency,
                       is_reconcilable  = EXCLUDED.is_reconcilable,
                       sort_order       = EXCLUDED.sort_order,
                       chart_code       = EXCLUDED.chart_code,
                       chart_version    = EXCLUDED.chart_version,
                       metadata = jsonb_set(accounting.accounts.metadata, '{deleted_at}', 'null'::jsonb, true)
                   RETURNING (xmax = 0) AS inserted
               )
               SELECT up.inserted AS inserted, COALESCE(pre.was_deleted, false) AS was_deleted
                 FROM up LEFT JOIN pre ON true"#,
        )
        .bind(row.id)
        .bind(row.company_id)
        .bind(&row.def.number)
        .bind(&row.def.code)
        .bind(&row.def.name)
        .bind(row.def.account_type.to_string())
        .bind(row.def.account_subtype.to_string())
        .bind(row.def.normal_balance.to_string())
        .bind(row.parent_id)
        .bind(row.level)
        .bind(&row.path)
        .bind(row.is_header)
        .bind(row.is_detail)
        .bind(&row.def.currency)
        .bind(row.def.is_reconcilable)
        .bind(row.def.sort_order)
        .bind(&row.chart_code)
        .bind(&row.chart_version)
        .fetch_one(&mut *tx)
        .await?;

        let inserted: bool = result.get("inserted");
        let was_deleted: bool = result.get("was_deleted");
        Ok(match (inserted, was_deleted) {
            (true, _) => UpsertOutcome::Inserted,
            (false, true) => UpsertOutcome::Resurrected,
            (false, false) => UpsertOutcome::Updated,
        })
    }
}
