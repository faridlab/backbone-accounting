//! SqlxReconcileGraphRepository — the persistence adapter for the reconciliation-graph port.
//!
//! Owns ALL SQL for the graph contract (the application `ReconcileWriteService` has none).
//! Every method rides the caller's `&mut sqlx::PgConnection` — the edge (or its
//! side-effecting unlink) commits atomically with the caller's unit of work. The caller
//! must have bound `app.company_id` on the connection (see `company_scope::bind_company_on`);
//! every statement additionally carries an explicit `company_id` predicate.
//!
//! Residual is COMPUTED here (`base face − Σ partial amounts on either side`), never stored.
//! The connected-component reads use a bounded recursive CTE (UNION dedup ⇒ termination on
//! the component's own size).

use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use sqlx::{PgConnection, Row};
use uuid::Uuid;

use crate::domain::gl_posting::PostingLine;
use crate::domain::reconcile_graph::{
    AccountReconcileFlags, LineLocator, LocatorResolution, MatchingGroup, NewPartial,
    PartyResidual, PartialRow, ReconcileLineSnapshot,
};
use crate::domain::repositories::reconcile_graph_repository::{
    JournalReversalMeta, ReconcileGraphRepository,
};

/// The snapshot columns shared by every line read.
const LINE_SELECT: &str = r#"
    SELECT l.id, l.journal_id, l.company_id, l.account_id,
           COALESCE(a.account_subtype::text, '') AS account_subtype,
           l.party_type::text AS party_type, l.party_id,
           l.debit_amount, l.credit_amount, l.currency, l.exchange_rate,
           j.transaction_date, l.is_posted,
           j.status::text AS journal_status, j.is_reversing,
           l.source_type, l.source_id, l.is_reconciled, l.full_reconcile_id,
           (l.base_debit_amount + l.base_credit_amount) AS base_amount
    FROM accounting.journal_lines l
    JOIN accounting.journals j ON j.id = l.journal_id
    LEFT JOIN accounting.accounts a ON a.id = l.account_id AND a.company_id = l.company_id
"#;

fn map_line(row: &sqlx::postgres::PgRow) -> ReconcileLineSnapshot {
    ReconcileLineSnapshot {
        id: row.get("id"),
        journal_id: row.get("journal_id"),
        company_id: row.get("company_id"),
        account_id: row.get("account_id"),
        account_subtype: row.get("account_subtype"),
        party_type: row.get("party_type"),
        party_id: row.get("party_id"),
        debit_amount: row.get("debit_amount"),
        credit_amount: row.get("credit_amount"),
        currency: row.get("currency"),
        exchange_rate: row.get("exchange_rate"),
        transaction_date: row.get("transaction_date"),
        is_posted: row.get("is_posted"),
        journal_status: row.get("journal_status"),
        journal_is_reversing: row.get("is_reversing"),
        source_type: row.get("source_type"),
        source_id: row.get("source_id"),
        is_reconciled: row.get("is_reconciled"),
        full_reconcile_id: row.get("full_reconcile_id"),
        base_amount: row.get("base_amount"),
    }
}

fn map_partial(row: &sqlx::postgres::PgRow) -> PartialRow {
    PartialRow {
        id: row.get("id"),
        company_id: row.get("company_id"),
        debit_move_id: row.get("debit_move_id"),
        credit_move_id: row.get("credit_move_id"),
        amount: row.get("amount"),
        max_date: row.get("max_date"),
        origin: row.get::<String, _>("origin"),
        full_reconcile_id: row.get("full_reconcile_id"),
        exchange_move_id: row.get("exchange_move_id"),
        source_type: row.get("source_type"),
        source_id: row.get("source_id"),
    }
}

const PARTIAL_SELECT: &str = r#"
    SELECT id, company_id, debit_move_id, credit_move_id, amount, max_date,
           origin::text AS origin, full_reconcile_id, exchange_move_id, source_type, source_id
    FROM accounting.partial_reconciles
"#;

/// The computed residual expression, reusable in any journal_lines read.
const RESIDUAL_EXPR: &str = r#"
    (l.base_debit_amount + l.base_credit_amount)
      - COALESCE((SELECT SUM(pr.amount) FROM accounting.partial_reconciles pr
                  WHERE pr.company_id = l.company_id
                    AND (pr.debit_move_id = l.id OR pr.credit_move_id = l.id)), 0)
"#;

/// Sqlx adapter. Stateless — every method rides the caller's connection.
#[derive(Default)]
pub struct SqlxReconcileGraphRepository {
    _private: (),
}

impl SqlxReconcileGraphRepository {
    pub fn new() -> Self {
        Self { _private: () }
    }
}

#[async_trait]
impl ReconcileGraphRepository for SqlxReconcileGraphRepository {
    async fn lock_line_by_locator(
        &self,
        conn: &mut PgConnection,
        company_id: Uuid,
        locator: &LineLocator,
    ) -> anyhow::Result<LocatorResolution> {
        let rows = sqlx::query(&format!(
            "{LINE_SELECT} WHERE l.company_id=$1 AND l.source_type=$2 AND l.source_id=$3 \
             AND l.account_id=$4 AND j.is_reversing=$5 ORDER BY l.id FOR UPDATE OF l"
        ))
        .bind(company_id)
        .bind(&locator.source_type)
        .bind(locator.source_id)
        .bind(locator.account_id)
        .bind(locator.reversing)
        .fetch_all(&mut *conn)
        .await?;
        Ok(match rows.len() {
            0 => LocatorResolution::NotFound,
            1 => LocatorResolution::One(map_line(&rows[0])),
            n => LocatorResolution::Ambiguous(n),
        })
    }

    async fn lock_line(
        &self,
        conn: &mut PgConnection,
        company_id: Uuid,
        line_id: Uuid,
    ) -> anyhow::Result<Option<ReconcileLineSnapshot>> {
        let row = sqlx::query(&format!(
            "{LINE_SELECT} WHERE l.company_id=$1 AND l.id=$2 FOR UPDATE OF l"
        ))
        .bind(company_id)
        .bind(line_id)
        .fetch_optional(&mut *conn)
        .await?;
        Ok(row.as_ref().map(map_line))
    }

    async fn account_flags(
        &self,
        conn: &mut PgConnection,
        company_id: Uuid,
        account_id: Uuid,
    ) -> anyhow::Result<Option<AccountReconcileFlags>> {
        let row = sqlx::query(
            r#"SELECT is_reconcilable, account_subtype::text AS subtype
               FROM accounting.accounts
               WHERE company_id=$1 AND id=$2 AND (metadata->>'deleted_at') IS NULL"#,
        )
        .bind(company_id)
        .bind(account_id)
        .fetch_optional(&mut *conn)
        .await?;
        Ok(row.map(|r| AccountReconcileFlags {
            is_reconcilable: r.get("is_reconcilable"),
            subtype: r.get("subtype"),
        }))
    }

    async fn residuals_of(
        &self,
        conn: &mut PgConnection,
        company_id: Uuid,
        line_ids: &[Uuid],
    ) -> anyhow::Result<Vec<(Uuid, Decimal)>> {
        let rows = sqlx::query(&format!(
            "SELECT l.id, {RESIDUAL_EXPR} AS residual FROM accounting.journal_lines l \
             WHERE l.company_id=$1 AND l.id = ANY($2) ORDER BY l.id"
        ))
        .bind(company_id)
        .bind(line_ids)
        .fetch_all(&mut *conn)
        .await?;
        Ok(rows.iter().map(|r| (r.get("id"), r.get::<Decimal, _>("residual"))).collect())
    }

    async fn lock_lines(
        &self,
        conn: &mut PgConnection,
        company_id: Uuid,
        line_ids: &[Uuid],
    ) -> anyhow::Result<()> {
        sqlx::query(
            "SELECT l.id FROM accounting.journal_lines l \
             WHERE l.company_id=$1 AND l.id = ANY($2) ORDER BY l.id FOR UPDATE OF l",
        )
        .bind(company_id)
        .bind(line_ids)
        .execute(&mut *conn)
        .await?;
        Ok(())
    }

    async fn insert_partial(&self, conn: &mut PgConnection, p: &NewPartial) -> anyhow::Result<Uuid> {
        let id = Uuid::new_v4();
        sqlx::query(
            r#"INSERT INTO accounting.partial_reconciles
                 (id, company_id, debit_move_id, credit_move_id, amount, currency, max_date,
                  origin, source_type, source_id, created_at, updated_at, metadata)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8::reconcile_origin,$9,$10,$11,$11,$12::jsonb)"#,
        )
        .bind(id)
        .bind(p.company_id)
        .bind(p.debit_move_id)
        .bind(p.credit_move_id)
        .bind(p.amount)
        .bind(&p.currency)
        .bind(p.max_date)
        .bind(&p.origin)
        .bind(&p.source_type)
        .bind(p.source_id)
        .bind(Utc::now())
        .bind(p.metadata.to_string())
        .execute(&mut *conn)
        .await?;
        Ok(id)
    }

    async fn set_exchange_move(
        &self,
        conn: &mut PgConnection,
        company_id: Uuid,
        partial_id: Uuid,
        journal_id: Uuid,
    ) -> anyhow::Result<()> {
        sqlx::query(
            "UPDATE accounting.partial_reconciles SET exchange_move_id=$3, updated_at=NOW() \
             WHERE company_id=$1 AND id=$2",
        )
        .bind(company_id)
        .bind(partial_id)
        .bind(journal_id)
        .execute(&mut *conn)
        .await?;
        Ok(())
    }

    async fn component_line_ids(
        &self,
        conn: &mut PgConnection,
        company_id: Uuid,
        seeds: &[Uuid],
    ) -> anyhow::Result<Vec<Uuid>> {
        let rows: Vec<(Uuid,)> = sqlx::query_as(
            r#"WITH RECURSIVE comp AS (
                   SELECT id FROM accounting.journal_lines WHERE company_id=$1 AND id = ANY($2)
                 UNION
                   SELECT CASE WHEN pr.debit_move_id = c.id THEN pr.credit_move_id
                               ELSE pr.debit_move_id END
                   FROM accounting.partial_reconciles pr
                   JOIN comp c ON (pr.debit_move_id = c.id OR pr.credit_move_id = c.id)
                   WHERE pr.company_id = $1
               )
               SELECT id FROM comp ORDER BY id"#,
        )
        .bind(company_id)
        .bind(seeds)
        .fetch_all(&mut *conn)
        .await?;
        Ok(rows.into_iter().map(|(id,)| id).collect())
    }

    async fn component_partial_ids(
        &self,
        conn: &mut PgConnection,
        company_id: Uuid,
        line_ids: &[Uuid],
    ) -> anyhow::Result<Vec<Uuid>> {
        let rows: Vec<(Uuid,)> = sqlx::query_as(
            r#"SELECT id FROM accounting.partial_reconciles
               WHERE company_id=$1 AND (debit_move_id = ANY($2) OR credit_move_id = ANY($2))
               ORDER BY id"#,
        )
        .bind(company_id)
        .bind(line_ids)
        .fetch_all(&mut *conn)
        .await?;
        Ok(rows.into_iter().map(|(id,)| id).collect())
    }

    async fn create_full_group(
        &self,
        conn: &mut PgConnection,
        company_id: Uuid,
        exchange_total: Decimal,
        now: DateTime<Utc>,
    ) -> anyhow::Result<Uuid> {
        let id = Uuid::new_v4();
        sqlx::query(
            r#"INSERT INTO accounting.full_reconciles
                 (id, company_id, exchange_total, reconciled_at, created_at, updated_at, metadata)
               VALUES ($1,$2,$3,$4,$4,$4,'{}'::jsonb)"#,
        )
        .bind(id)
        .bind(company_id)
        .bind(exchange_total)
        .bind(now)
        .execute(&mut *conn)
        .await?;
        Ok(id)
    }

    async fn attach_group(
        &self,
        conn: &mut PgConnection,
        company_id: Uuid,
        group_id: Uuid,
        line_ids: &[Uuid],
        partial_ids: &[Uuid],
        now: DateTime<Utc>,
    ) -> anyhow::Result<()> {
        // journal_lines carries no updated_at column (no audit metadata on lines).
        sqlx::query(
            "UPDATE accounting.journal_lines \
             SET full_reconcile_id=$3, is_reconciled=TRUE, reconciled_at=$4 \
             WHERE company_id=$1 AND id = ANY($2)",
        )
        .bind(company_id)
        .bind(line_ids)
        .bind(group_id)
        .bind(now)
        .execute(&mut *conn)
        .await?;
        sqlx::query(
            "UPDATE accounting.partial_reconciles SET full_reconcile_id=$3, updated_at=NOW() \
             WHERE company_id=$1 AND id = ANY($2)",
        )
        .bind(company_id)
        .bind(partial_ids)
        .bind(group_id)
        .execute(&mut *conn)
        .await?;
        Ok(())
    }

    async fn clear_line_flags(
        &self,
        conn: &mut PgConnection,
        company_id: Uuid,
        line_ids: &[Uuid],
    ) -> anyhow::Result<()> {
        sqlx::query(
            "UPDATE accounting.journal_lines \
             SET full_reconcile_id=NULL, is_reconciled=FALSE, reconciled_at=NULL \
             WHERE company_id=$1 AND id = ANY($2)",
        )
        .bind(company_id)
        .bind(line_ids)
        .execute(&mut *conn)
        .await?;
        Ok(())
    }

    async fn load_partial(
        &self,
        conn: &mut PgConnection,
        company_id: Uuid,
        partial_id: Uuid,
    ) -> anyhow::Result<Option<PartialRow>> {
        let row = sqlx::query(&format!(
            "{PARTIAL_SELECT} WHERE company_id=$1 AND id=$2"
        ))
        .bind(company_id)
        .bind(partial_id)
        .fetch_optional(&mut *conn)
        .await?;
        Ok(row.as_ref().map(map_partial))
    }

    async fn partials_between(
        &self,
        conn: &mut PgConnection,
        company_id: Uuid,
        a_id: Uuid,
        b_id: Uuid,
    ) -> anyhow::Result<Vec<PartialRow>> {
        let rows = sqlx::query(&format!(
            "{PARTIAL_SELECT} WHERE company_id=$1 \
             AND ((debit_move_id=$2 AND credit_move_id=$3) OR (debit_move_id=$3 AND credit_move_id=$2)) \
             ORDER BY created_at, id"
        ))
        .bind(company_id)
        .bind(a_id)
        .bind(b_id)
        .fetch_all(&mut *conn)
        .await?;
        Ok(rows.iter().map(map_partial).collect())
    }

    async fn derived_partials(
        &self,
        conn: &mut PgConnection,
        company_id: Uuid,
        parent_partial_id: Uuid,
    ) -> anyhow::Result<Vec<PartialRow>> {
        let rows = sqlx::query(&format!(
            "{PARTIAL_SELECT} WHERE company_id=$1 AND source_id=$2 ORDER BY created_at, id"
        ))
        .bind(company_id)
        .bind(parent_partial_id)
        .fetch_all(&mut *conn)
        .await?;
        Ok(rows.iter().map(map_partial).collect())
    }

    async fn generated_journal_ids(
        &self,
        conn: &mut PgConnection,
        company_id: Uuid,
        partial_ids: &[Uuid],
    ) -> anyhow::Result<Vec<Uuid>> {
        let rows: Vec<(Uuid,)> = sqlx::query_as(
            r#"SELECT id FROM accounting.journals
               WHERE company_id=$1 AND source_type='reconciliation' AND source_id = ANY($2)
               ORDER BY id"#,
        )
        .bind(company_id)
        .bind(partial_ids)
        .fetch_all(&mut *conn)
        .await?;
        Ok(rows.into_iter().map(|(id,)| id).collect())
    }

    async fn journal_lines_with_ids(
        &self,
        conn: &mut PgConnection,
        company_id: Uuid,
        journal_id: Uuid,
    ) -> anyhow::Result<Vec<(Uuid, PostingLine)>> {
        let rows = sqlx::query(
            r#"SELECT id, account_id, debit_amount, credit_amount, party_type::text AS party_type,
                      party_id, cost_center_id, project_id, department_id, description
               FROM accounting.journal_lines
               WHERE company_id=$1 AND journal_id=$2 ORDER BY line_number"#,
        )
        .bind(company_id)
        .bind(journal_id)
        .fetch_all(&mut *conn)
        .await?;
        Ok(rows
            .iter()
            .map(|r| {
                (
                    r.get("id"),
                    PostingLine {
                        account_id: r.get("account_id"),
                        debit: r.get("debit_amount"),
                        credit: r.get("credit_amount"),
                        party_type: r.get::<Option<String>, _>("party_type"),
                        party_id: r.get("party_id"),
                        cost_center_id: r.get("cost_center_id"),
                        project_id: r.get("project_id"),
                        department_id: r.get("department_id"),
                        description: r.get("description"),
                    },
                )
            })
            .collect())
    }

    async fn journal_reversal_meta(
        &self,
        conn: &mut PgConnection,
        company_id: Uuid,
        journal_id: Uuid,
    ) -> anyhow::Result<Option<JournalReversalMeta>> {
        let row = sqlx::query(
            r#"SELECT j.id, j.company_id, j.branch_id, j.posting_date, j.currency, j.source_id,
                      j.fiscal_period_id, j.fiscal_year, j.fiscal_month,
                      (SELECT ap.id FROM accounting.accounting_posts ap
                       WHERE ap.journal_id = j.id AND ap.company_id = j.company_id
                       ORDER BY ap.posted_at NULLS LAST, ap.id LIMIT 1) AS reverses_post_id
               FROM accounting.journals j
               WHERE j.company_id=$1 AND j.id=$2"#,
        )
        .bind(company_id)
        .bind(journal_id)
        .fetch_optional(&mut *conn)
        .await?;
        Ok(row.map(|r| JournalReversalMeta {
            journal_id: r.get("id"),
            company_id: r.get("company_id"),
            branch_id: r.get("branch_id"),
            posting_date: r.get("posting_date"),
            currency: r.get("currency"),
            source_id: r.get("source_id"),
            reverses_post_id: r.get("reverses_post_id"),
            fiscal_period_id: r.get("fiscal_period_id"),
            fiscal_year: r.get("fiscal_year"),
            fiscal_month: r.get("fiscal_month"),
        }))
    }

    async fn delete_partials(
        &self,
        conn: &mut PgConnection,
        company_id: Uuid,
        partial_ids: &[Uuid],
    ) -> anyhow::Result<()> {
        sqlx::query("DELETE FROM accounting.partial_reconciles WHERE company_id=$1 AND id = ANY($2)")
            .bind(company_id)
            .bind(partial_ids)
            .execute(&mut *conn)
            .await?;
        Ok(())
    }

    async fn group_partial_ids(
        &self,
        conn: &mut PgConnection,
        company_id: Uuid,
        group_id: Uuid,
    ) -> anyhow::Result<Vec<Uuid>> {
        let rows: Vec<(Uuid,)> = sqlx::query_as(
            "SELECT id FROM accounting.partial_reconciles \
             WHERE company_id=$1 AND full_reconcile_id=$2",
        )
        .bind(company_id)
        .bind(group_id)
        .fetch_all(&mut *conn)
        .await?;
        Ok(rows.into_iter().map(|r| r.0).collect())
    }

    async fn dissolve_group(
        &self,
        conn: &mut PgConnection,
        company_id: Uuid,
        group_id: Uuid,
    ) -> anyhow::Result<()> {
        // Null any straggler references first (FK from partials; lines are cleared by the
        // write service before this runs).
        sqlx::query(
            "UPDATE accounting.partial_reconciles SET full_reconcile_id=NULL, updated_at=NOW() \
             WHERE company_id=$1 AND full_reconcile_id=$2",
        )
        .bind(company_id)
        .bind(group_id)
        .execute(&mut *conn)
        .await?;
        sqlx::query("DELETE FROM accounting.full_reconciles WHERE company_id=$1 AND id=$2")
            .bind(company_id)
            .bind(group_id)
            .execute(&mut *conn)
            .await?;
        Ok(())
    }

    async fn reversal_counterpart(
        &self,
        conn: &mut PgConnection,
        company_id: Uuid,
        line: &ReconcileLineSnapshot,
    ) -> anyhow::Result<Option<ReconcileLineSnapshot>> {
        let Some(src_type) = line.source_type.as_deref() else {
            return Ok(None);
        };
        let Some(src_id) = line.source_id else {
            return Ok(None);
        };
        let row = sqlx::query(&format!(
            "{LINE_SELECT} WHERE l.company_id=$1 AND l.account_id=$2 AND l.source_type=$3 \
             AND l.source_id=$4 AND j.is_reversing=$5 AND l.is_posted AND j.status='posted' \
             AND l.id <> $6 ORDER BY l.id LIMIT 1 FOR UPDATE OF l"
        ))
        .bind(company_id)
        .bind(line.account_id)
        .bind(src_type)
        .bind(src_id)
        .bind(!line.journal_is_reversing)
        .bind(line.id)
        .fetch_optional(&mut *conn)
        .await?;
        Ok(row.as_ref().map(map_line))
    }

    async fn residuals_for_party(
        &self,
        conn: &mut PgConnection,
        company_id: Uuid,
        account_id: Uuid,
        party_type: &str,
        party_id: Uuid,
    ) -> anyhow::Result<Vec<PartyResidual>> {
        let rows = sqlx::query(&format!(
            r#"SELECT l.id, l.journal_id, j.journal_number, j.transaction_date,
                      l.source_reference, l.currency, l.is_reconciled, {RESIDUAL_EXPR} AS residual
               FROM accounting.journal_lines l
               JOIN accounting.journals j ON j.id = l.journal_id
               WHERE l.company_id=$1 AND l.account_id=$2 AND l.party_type=$3::party_type
                 AND l.party_id=$4 AND {RESIDUAL_EXPR} > 0"#,
        ))
        .bind(company_id)
        .bind(account_id)
        .bind(party_type)
        .bind(party_id)
        .fetch_all(&mut *conn)
        .await?;
        let mut out: Vec<PartyResidual> = rows
            .iter()
            .map(|r| PartyResidual {
                line_id: r.get("id"),
                journal_id: r.get("journal_id"),
                journal_number: r.get("journal_number"),
                transaction_date: r.get("transaction_date"),
                source_reference: r.get("source_reference"),
                residual: r.get("residual"),
                currency: r.get("currency"),
                is_reconciled: r.get("is_reconciled"),
            })
            .collect();
        // Oldest first — the aging shape.
        out.sort_by(|a, b| a.transaction_date.cmp(&b.transaction_date).then(a.line_id.cmp(&b.line_id)));
        Ok(out)
    }

    async fn matching_group(
        &self,
        conn: &mut PgConnection,
        company_id: Uuid,
        line_id: Uuid,
    ) -> anyhow::Result<MatchingGroup> {
        let line_ids = self.component_line_ids(conn, company_id, &[line_id]).await?;
        let partial_ids = self.component_partial_ids(conn, company_id, &line_ids).await?;
        let residuals = self.residuals_of(conn, company_id, &line_ids).await?;

        // Label: a stored full-reconcile wins; otherwise derive from the minimum partial id.
        let seed = sqlx::query(
            "SELECT full_reconcile_id FROM accounting.journal_lines WHERE company_id=$1 AND id=$2",
        )
        .bind(company_id)
        .bind(line_id)
        .fetch_optional(&mut *conn)
        .await?;
        let full_id: Option<Uuid> = seed.as_ref().and_then(|r| r.get("full_reconcile_id"));

        let label = if let Some(fid) = full_id {
            format!("F-{}", fid.to_string()[..8].to_lowercase())
        } else if let Some(min) = partial_ids.iter().min() {
            format!("P-{}", min.to_string()[..8].to_lowercase())
        } else {
            "-".to_string()
        };
        Ok(MatchingGroup { label, full_reconcile_id: full_id, line_ids, partial_ids, residuals })
    }

    async fn period_closed(
        &self,
        conn: &mut PgConnection,
        company_id: Uuid,
        date: NaiveDate,
    ) -> anyhow::Result<bool> {
        // bool_or over an empty match still yields one row with NULL — decode nullable.
        let blocked: Option<Option<bool>> = sqlx::query_scalar(
            r#"SELECT bool_or(status IN ('closed','locked'))
               FROM accounting.fiscal_periods
               WHERE company_id=$1 AND start_date<=$2 AND end_date>=$2
                 AND (metadata->>'deleted_at') IS NULL"#,
        )
        .bind(company_id)
        .bind(date)
        .fetch_optional(&mut *conn)
        .await?;
        Ok(blocked.flatten() == Some(true))
    }
}
