//! SqlxJournalWorkflowRepository — SQLx adapter for the journal-workflow port.

use chrono::{DateTime, Utc};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::domain::repositories::journal_workflow_repository::{
    JournalStatusRow, JournalWorkflowRepository,
};

pub struct SqlxJournalWorkflowRepository {
    pool: PgPool,
}

impl SqlxJournalWorkflowRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl JournalWorkflowRepository for SqlxJournalWorkflowRepository {
    async fn find_status(
        &self,
        journal_id: Uuid,
        company_id: Uuid,
    ) -> anyhow::Result<Option<JournalStatusRow>> {
        let row = sqlx::query(
            r#"SELECT status::text AS status, currency FROM accounting.journals
               WHERE id=$1 AND company_id=$2 AND (metadata->>'deleted_at') IS NULL"#,
        )
        .bind(journal_id)
        .bind(company_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|r| JournalStatusRow {
            status: r.get("status"),
            currency: r.get("currency"),
        }))
    }

    async fn current_status(
        &self,
        journal_id: Uuid,
        company_id: Uuid,
    ) -> anyhow::Result<Option<String>> {
        let s: Option<String> = sqlx::query_scalar(
            "SELECT status::text FROM accounting.journals WHERE id=$1 AND company_id=$2 AND (metadata->>'deleted_at') IS NULL",
        )
        .bind(journal_id)
        .bind(company_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(s)
    }

    async fn submit(&self, journal_id: Uuid, company_id: Uuid) -> anyhow::Result<bool> {
        let res = sqlx::query(
            "UPDATE accounting.journals SET status='pending_approval'::journal_status \
             WHERE id=$1 AND company_id=$2 AND status='draft'::journal_status \
             AND (metadata->>'deleted_at') IS NULL",
        )
        .bind(journal_id)
        .bind(company_id)
        .execute(&self.pool)
        .await?;
        Ok(res.rows_affected() > 0)
    }

    async fn approve(
        &self,
        journal_id: Uuid,
        company_id: Uuid,
        approved_by: Option<Uuid>,
        at: DateTime<Utc>,
    ) -> anyhow::Result<bool> {
        let res = sqlx::query(
            "UPDATE accounting.journals SET status='approved'::journal_status, approved_at=$1, approved_by=$2 \
             WHERE id=$3 AND company_id=$4 AND status='pending_approval'::journal_status \
             AND (metadata->>'deleted_at') IS NULL",
        )
        .bind(at)
        .bind(approved_by)
        .bind(journal_id)
        .bind(company_id)
        .execute(&self.pool)
        .await?;
        Ok(res.rows_affected() > 0)
    }

    async fn reject(
        &self,
        journal_id: Uuid,
        company_id: Uuid,
        reason: &str,
        rejected_by: Option<Uuid>,
        at: DateTime<Utc>,
    ) -> anyhow::Result<bool> {
        let res = sqlx::query(
            "UPDATE accounting.journals SET status='rejected'::journal_status, rejected_at=$1, \
             rejected_by=$2, rejection_reason=$3 \
             WHERE id=$4 AND company_id=$5 AND status IN ('draft','pending_approval') \
             AND (metadata->>'deleted_at') IS NULL",
        )
        .bind(at)
        .bind(rejected_by)
        .bind(reason)
        .bind(journal_id)
        .bind(company_id)
        .execute(&self.pool)
        .await?;
        Ok(res.rows_affected() > 0)
    }

    async fn mark_voided(
        &self,
        journal_id: Uuid,
        company_id: Uuid,
        voided_by: Option<Uuid>,
        reason: &str,
        at: DateTime<Utc>,
    ) -> anyhow::Result<()> {
        sqlx::query(
            "UPDATE accounting.journals SET status='voided'::journal_status, is_voided=TRUE, \
             voided_at=$1, voided_by=$2, void_reason=$3 WHERE id=$4 AND company_id=$5",
        )
        .bind(at)
        .bind(voided_by)
        .bind(reason)
        .bind(journal_id)
        .bind(company_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn original_post(
        &self,
        journal_id: Uuid,
        company_id: Uuid,
    ) -> anyhow::Result<Option<Uuid>> {
        let id: Option<Uuid> = sqlx::query_scalar(
            r#"SELECT id FROM accounting.accounting_posts
               WHERE journal_id=$1 AND company_id=$2 AND posting_status='posted'::posting_status
                 AND posting_type='original' LIMIT 1"#,
        )
        .bind(journal_id)
        .bind(company_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(id)
    }
}
