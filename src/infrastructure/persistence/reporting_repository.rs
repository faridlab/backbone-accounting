//! SqlxReportingRepository — SQLx adapter for the reporting port.
//!
//! Fence discipline: every read rides the company-scoped helpers (request connection →
//! task-local bind → plain pool), and every statement carries an explicit `company_id`
//! predicate — the bare-pool shape this replaces silently emptied under the app role because
//! `set_config(is_local)` evaporates off a pooled connection.

use async_trait::async_trait;
use chrono::NaiveDate;
use rust_decimal::Decimal;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use backbone_orm::company_scope::fetch_all_rows_scoped;

use crate::domain::repositories::reporting_repository::{
    AccountNodeRow, AccountSumRow, AgedItemRow, GlLineRow, PartyLedgerRow, ReportingRepository,
};

/// Residual as of a date: face minus the partial reconciliations already applied by that
/// date (a partial dated after `as_of` has not settled yet, so it does not reduce the
/// historical residual). Mirrors the graph's computed-residual contract — never stored.
/// `pos` is the positional parameter holding `as_of` in the surrounding query — the
/// expression may be instantiated more than once per statement, so the number is per-query.
fn as_of_residual_expr(pos: usize) -> String {
    format!(
        r#"(l.base_debit_amount + l.base_credit_amount)
             - COALESCE((SELECT SUM(pr.amount) FROM accounting.partial_reconciles pr
                         WHERE pr.company_id = l.company_id
                           AND pr.max_date <= ${pos}
                           AND (pr.debit_move_id = l.id OR pr.credit_move_id = l.id)), 0)"#
    )
}

pub struct SqlxReportingRepository {
    pool: PgPool,
}

impl SqlxReportingRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ReportingRepository for SqlxReportingRepository {
    async fn account_sums(
        &self,
        company_id: Uuid,
        lo: Option<NaiveDate>,
        hi: NaiveDate,
    ) -> anyhow::Result<Vec<AccountSumRow>> {
        let rows = fetch_all_rows_scoped(
            &self.pool,
            sqlx::query(
                r#"SELECT a.id AS aid, a.account_type::text AS at, a.account_number AS num, a.name AS name,
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
            .bind(lo),
        )
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| AccountSumRow {
                account_id: r.get("aid"),
                account_type: r.get("at"),
                account_number: r.get("num"),
                name: r.get("name"),
                debit: r.get("dr"),
                credit: r.get("cr"),
            })
            .collect())
    }

    async fn account_directory(&self, company_id: Uuid) -> anyhow::Result<Vec<AccountNodeRow>> {
        let rows = fetch_all_rows_scoped(
            &self.pool,
            sqlx::query(
                r#"SELECT id, parent_id, account_number, name, account_type::text AS at,
                          level, is_header, is_detail
                   FROM accounting.accounts
                   WHERE company_id = $1
                     AND (metadata->>'deleted_at') IS NULL
                   ORDER BY account_number"#,
            )
            .bind(company_id),
        )
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| AccountNodeRow {
                id: r.get("id"),
                parent_id: r.get("parent_id"),
                account_number: r.get("account_number"),
                name: r.get("name"),
                account_type: r.get("at"),
                level: r.get("level"),
                is_header: r.get("is_header"),
                is_detail: r.get("is_detail"),
            })
            .collect())
    }

    async fn gl_lines(
        &self,
        company_id: Uuid,
        account_id: Option<Uuid>,
        lo: Option<NaiveDate>,
        hi: NaiveDate,
        limit: i64,
        offset: i64,
    ) -> anyhow::Result<Vec<GlLineRow>> {
        // Running balances are COMPUTED in date order, not read from the materialized
        // balance_before/balance_after columns: those chain in insertion order, so a
        // backdated posting (earlier date, later sequence) makes the stored columns
        // inconsistent with any date-ordered read. The window runs over the account's
        // FULL history inside the subquery; the date window and pagination apply outside
        // so the opening balance stays absolute since inception. Signed on the account's
        // normal side, matching `accounts.current_balance` and the stored columns.
        // The `account_id` key after `account_number` keeps sections contiguous even if
        // two accounts share a number (grouping is by id, ordering was by number only).
        let rows = fetch_all_rows_scoped(
            &self.pool,
            sqlx::query(
                r#"SELECT account_id, account_number, account_name, journal_number,
                          transaction_date, posting_date, description, reference, currency,
                          debit_amount, credit_amount, balance_before, balance_after,
                          party_type::text AS pt, party_id,
                          source_type, source_reference, is_reconciled
                   FROM (
                     SELECT l.account_id, l.account_number, l.account_name, l.journal_number,
                            l.transaction_date, l.posting_date, l.description, l.reference,
                            l.currency, l.debit_amount, l.credit_amount, l.sequence_number,
                            l.id,
                            COALESCE(SUM(
                              CASE WHEN l.normal_balance = 'debit'
                                   THEN l.debit_amount - l.credit_amount
                                   ELSE l.credit_amount - l.debit_amount END
                            ) OVER (PARTITION BY l.account_id
                                    ORDER BY l.posting_date, l.sequence_number, l.id
                                    ROWS BETWEEN UNBOUNDED PRECEDING AND 1 PRECEDING), 0)
                              AS balance_before,
                            SUM(
                              CASE WHEN l.normal_balance = 'debit'
                                   THEN l.debit_amount - l.credit_amount
                                   ELSE l.credit_amount - l.debit_amount END
                            ) OVER (PARTITION BY l.account_id
                                    ORDER BY l.posting_date, l.sequence_number, l.id
                                    ROWS UNBOUNDED PRECEDING)
                              AS balance_after,
                            l.party_type::text AS party_type, l.party_id,
                            l.source_type, l.source_reference, l.is_reconciled
                     FROM accounting.ledgers l
                     WHERE l.company_id = $1
                       AND ($2::uuid IS NULL OR l.account_id = $2)
                   ) hist
                   WHERE posting_date <= $3
                     AND ($4::date IS NULL OR posting_date >= $4)
                   ORDER BY account_number, account_id, posting_date, sequence_number, id
                   LIMIT $5 OFFSET $6"#,
            )
            .bind(company_id)
            .bind(account_id)
            .bind(hi)
            .bind(lo)
            .bind(limit)
            .bind(offset),
        )
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| GlLineRow {
                account_id: r.get("account_id"),
                account_number: r.get("account_number"),
                account_name: r.get("account_name"),
                journal_number: r.get("journal_number"),
                transaction_date: r.get("transaction_date"),
                posting_date: r.get("posting_date"),
                description: r.get("description"),
                reference: r.get("reference"),
                currency: r.get("currency"),
                debit: r.get("debit_amount"),
                credit: r.get("credit_amount"),
                balance_before: r.get("balance_before"),
                balance_after: r.get("balance_after"),
                party_type: r.get("pt"),
                party_id: r.get("party_id"),
                source_type: r.get("source_type"),
                source_reference: r.get("source_reference"),
                is_reconciled: r.get("is_reconciled"),
            })
            .collect())
    }

    async fn party_ledger_lines(
        &self,
        company_id: Uuid,
        party_type: &str,
        party_id: Uuid,
        as_of: NaiveDate,
    ) -> anyhow::Result<Vec<PartyLedgerRow>> {
        let sql = format!(
            r#"SELECT l.id, l.account_id, l.account_number, l.account_name,
                      j.journal_number, j.transaction_date, l.description,
                      l.base_debit_amount, l.base_credit_amount, l.currency,
                      {residual} AS residual
               FROM accounting.journal_lines l
               JOIN accounting.journals j ON j.id = l.journal_id
               JOIN accounting.accounts a ON a.id = l.account_id
                                        AND a.company_id = l.company_id
               WHERE l.company_id = $1
                 AND l.party_type = $2::party_type
                 AND l.party_id = $3
                 AND j.transaction_date <= $4
                 AND a.account_subtype IN ('accounts_receivable'::account_subtype,
                                           'accounts_payable'::account_subtype)
               ORDER BY j.transaction_date, l.id"#,
            residual = as_of_residual_expr(4)
        );
        let rows = fetch_all_rows_scoped(
            &self.pool,
            sqlx::query(&sql)
                .bind(company_id)
                .bind(party_type)
                .bind(party_id)
                .bind(as_of),
        )
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| PartyLedgerRow {
                line_id: r.get("id"),
                account_id: r.get("account_id"),
                account_number: r.get("account_number"),
                account_name: r.get("account_name"),
                journal_number: r.get("journal_number"),
                transaction_date: r.get("transaction_date"),
                description: r.get("description"),
                debit: r.get("base_debit_amount"),
                credit: r.get("base_credit_amount"),
                currency: r.get("currency"),
                residual: r.get("residual"),
            })
            .collect())
    }

    async fn aged_open_items(
        &self,
        company_id: Uuid,
        account_subtype: &str,
        as_of: NaiveDate,
    ) -> anyhow::Result<Vec<AgedItemRow>> {
        let sql = format!(
            r#"SELECT l.party_type::text AS pt, l.party_id, l.account_id,
                      l.account_number, j.transaction_date, l.source_reference, l.currency,
                      {residual} AS residual
               FROM accounting.journal_lines l
               JOIN accounting.journals j ON j.id = l.journal_id
               JOIN accounting.accounts a ON a.id = l.account_id AND a.company_id = l.company_id
               WHERE l.company_id = $1
                 AND a.account_subtype = $2::account_subtype
                 AND j.transaction_date <= $3
                 AND l.party_id IS NOT NULL
                 AND {residual} > 0
               ORDER BY l.party_id, j.transaction_date, l.id"#,
            residual = as_of_residual_expr(3)
        );
        let rows = fetch_all_rows_scoped(
            &self.pool,
            sqlx::query(&sql)
                .bind(company_id)
                .bind(account_subtype)
                .bind(as_of),
        )
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| AgedItemRow {
                party_type: r.get("pt"),
                party_id: r.get("party_id"),
                account_id: r.get("account_id"),
                account_number: r.get("account_number"),
                transaction_date: r.get("transaction_date"),
                source_reference: r.get("source_reference"),
                currency: r.get("currency"),
                residual: r.get("residual"),
            })
            .collect())
    }
}
