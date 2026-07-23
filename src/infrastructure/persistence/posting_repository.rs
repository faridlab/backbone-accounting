//! SqlxPostingRepository — the persistence adapter for the GL-posting port.
//!
//! Owns ALL SQL for the posting contract (the application `PostingService` has none). The atomic
//! `commit_posting` / `commit_manual_journal` methods open their own transaction and take the
//! per-account `FOR UPDATE` lock internally, so the cross-table write stays atomic and
//! concurrency-safe. SQL is moved verbatim from the former service implementation.

use std::collections::HashMap;

use chrono::NaiveDate;
use rust_decimal::Decimal;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::domain::gl_posting::{map_source, PostingLine};
use crate::domain::repositories::posting_repository::{
    FailedPost, LedgerEntryInput, ManualJournalCommit, ManualJournalForPost, PostableAccount,
    PostingCommit, PostingRepository, PostingWrite, ReversalSource,
};

/// Per-account snapshot used internally during the atomic write (mirrors `PostableAccount`).
struct AccountInfo {
    number: String,
    name: String,
    account_type: String,
    normal_balance: String,
    current_balance: Decimal,
}

/// SQLx implementation of the `PostingRepository` port.
pub struct SqlxPostingRepository {
    pool: PgPool,
}

impl SqlxPostingRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn row_to_account(row: &sqlx::postgres::PgRow) -> (Uuid, PostableAccount) {
        let id: Uuid = row.get("id");
        (
            id,
            PostableAccount {
                id,
                number: row.get("account_number"),
                name: row.get("name"),
                account_type: row.get("at"),
                subtype: row.get("st"),
                normal_balance: row.get("nb"),
                is_detail: row.get("is_detail"),
                is_header: row.get("is_header"),
                status: row.get("status"),
                current_balance: row.get("current_balance"),
            },
        )
    }
}

#[async_trait::async_trait]
impl PostingRepository for SqlxPostingRepository {
    async fn find_existing_post(
        &self,
        company_id: Uuid,
        source_type: &str,
        source_id: Uuid,
        posting_type: &str,
        idempotency_key: Option<&str>,
    ) -> anyhow::Result<Option<(Uuid, Uuid)>> {
        let row = if let Some(key) = idempotency_key {
            sqlx::query(
                r#"SELECT id, journal_id FROM accounting.accounting_posts
                   WHERE company_id=$1 AND idempotency_key=$2 AND posting_status='posted'::posting_status
                   LIMIT 1"#,
            )
            .bind(company_id)
            .bind(key)
            .fetch_optional(&self.pool)
            .await?
        } else {
            sqlx::query(
                r#"SELECT id, journal_id FROM accounting.accounting_posts
                   WHERE company_id=$1 AND source_type=$2::posting_source_type AND source_id=$3
                     AND posting_type=$4::posting_type AND posting_status='posted'::posting_status
                   LIMIT 1"#,
            )
            .bind(company_id)
            .bind(source_type)
            .bind(source_id)
            .bind(posting_type)
            .fetch_optional(&self.pool)
            .await?
        };
        Ok(row.and_then(|r| {
            let id: Uuid = r.get("id");
            let jid: Option<Uuid> = r.get("journal_id");
            jid.map(|j| (id, j))
        }))
    }

    async fn find_postable_accounts(
        &self,
        company_id: Uuid,
        ids: &[Uuid],
    ) -> anyhow::Result<Vec<PostableAccount>> {
        let rows = sqlx::query(
            r#"SELECT id, account_number, name, account_type::text AS at,
                      account_subtype::text AS st, normal_balance::text AS nb,
                      is_detail, is_header, status::text AS status, current_balance
               FROM accounting.accounts
               WHERE company_id=$1 AND id = ANY($2) AND (metadata->>'deleted_at') IS NULL"#,
        )
        .bind(company_id)
        .bind(ids)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.iter().map(Self::row_to_account).map(|(_, a)| a).collect())
    }

    async fn is_period_closed(&self, company_id: Uuid, date: NaiveDate) -> anyhow::Result<bool> {
        let blocked: Option<bool> = sqlx::query_scalar(
            r#"SELECT bool_or(status IN ('closed','locked'))
               FROM accounting.fiscal_periods
               WHERE company_id=$1 AND start_date<=$2 AND end_date>=$2
                 AND (metadata->>'deleted_at') IS NULL"#,
        )
        .bind(company_id)
        .bind(date)
        .fetch_one(&self.pool)
        .await?;
        Ok(blocked == Some(true))
    }

    async fn find_period_id(
        &self,
        company_id: Uuid,
        date: NaiveDate,
    ) -> anyhow::Result<Option<Uuid>> {
        let id: Option<Uuid> = sqlx::query_scalar(
            r#"SELECT id FROM accounting.fiscal_periods
               WHERE company_id=$1 AND start_date<=$2 AND end_date>=$2
                 AND (metadata->>'deleted_at') IS NULL
               ORDER BY (end_date - start_date) ASC LIMIT 1"#,
        )
        .bind(company_id)
        .bind(date)
        .fetch_optional(&self.pool)
        .await?;
        Ok(id)
    }

    async fn find_reversal_source(
        &self,
        orig_post_id: Uuid,
        company_id: Uuid,
    ) -> anyhow::Result<Option<ReversalSource>> {
        let orig_journal_id: Option<Uuid> = sqlx::query_scalar(
            "SELECT journal_id FROM accounting.accounting_posts WHERE id=$1 AND company_id=$2 AND posting_status='posted'::posting_status",
        )
        .bind(orig_post_id)
        .bind(company_id)
        .fetch_optional(&self.pool)
        .await?;
        let Some(orig_journal_id) = orig_journal_id else {
            return Ok(None);
        };

        let rows = sqlx::query(
            r#"SELECT account_id, debit_amount, credit_amount, party_type::text AS pt, party_id,
                      cost_center_id, project_id, department_id
               FROM accounting.journal_lines WHERE journal_id=$1 AND company_id=$2 ORDER BY line_number"#,
        )
        .bind(orig_journal_id)
        .bind(company_id)
        .fetch_all(&self.pool)
        .await?;

        let lines = rows
            .into_iter()
            .map(|r| PostingLine {
                account_id: r.get("account_id"),
                debit: r.get("credit_amount"), // swapped
                credit: r.get("debit_amount"),
                party_type: r.get("pt"),
                party_id: r.get("party_id"),
                cost_center_id: r.get("cost_center_id"),
                project_id: r.get("project_id"),
                department_id: r.get("department_id"),
                description: Some("Reversal".to_string()),
            })
            .collect();
        Ok(Some(ReversalSource { journal_id: orig_journal_id, lines }))
    }

    async fn commit_posting(&self, write: PostingWrite) -> anyhow::Result<PostingCommit> {
        let now = write.now;
        let mut tx = self.pool.begin().await?;

        let total_debit: Decimal = write.lines.iter().map(|l| l.debit).sum();
        let total_credit: Decimal = write.lines.iter().map(|l| l.credit).sum();
        let accounts = load_accounts_locked(&mut tx, write.company_id, &write.lines).await?;

        let journal_id = Uuid::new_v4();
        let journal_number = format!(
            "JV-{}-{}",
            write.posting_date.format("%Y%m%d"),
            &Uuid::new_v4().to_string()[..8]
        );
        let (journal_type, journal_source) = map_source(&write.source_type, &write.posting_type);
        let is_reversing = write.posting_type == "reversal";

        sqlx::query(
            r#"INSERT INTO accounting.journals
                (id, company_id, branch_id, journal_number, journal_type, transaction_date,
                 posting_date, fiscal_period_id, fiscal_year, fiscal_month, description, currency,
                 total_debit, total_credit, line_count, source, source_type, source_id,
                 source_reference, is_reversing, reverses_id, status, posted_at, posted_by)
               VALUES ($1,$2,$3,$4,$5::journal_type,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,
                       $16::journal_source,$17,$18,$19,$20,$21,'posted'::journal_status,$22,$23)"#,
        )
        .bind(journal_id)
        .bind(write.company_id)
        .bind(write.branch_id)
        .bind(&journal_number)
        .bind(journal_type)
        .bind(write.posting_date)
        .bind(write.posting_date)
        .bind(write.fiscal_period_id)
        .bind(write.fiscal_year)
        .bind(write.fiscal_month)
        .bind(write.description.clone().unwrap_or_else(|| format!("{} posting", write.source_type)))
        .bind(&write.currency)
        .bind(total_debit)
        .bind(total_credit)
        .bind(write.lines.len() as i32)
        .bind(journal_source)
        .bind(&write.source_type)
        .bind(write.source_id)
        .bind(&write.source_reference)
        .bind(is_reversing)
        .bind(write.reverses_journal_id)
        .bind(now)
        .bind(write.posted_by)
        .execute(&mut *tx)
        .await?;

        let mut line_inputs: Vec<LedgerEntryInput> = Vec::with_capacity(write.lines.len());
        for (i, line) in write.lines.iter().enumerate() {
            let acct = &accounts[&line.account_id];
            let line_number = (i + 1) as i32;
            let journal_line_id = Uuid::new_v4();

            sqlx::query(
                r#"INSERT INTO accounting.journal_lines
                    (id, journal_id, company_id, branch_id, party_type, party_id, line_number,
                     account_id, account_number, account_name, debit_amount, credit_amount, currency,
                     base_debit_amount, base_credit_amount, description, cost_center_id, project_id,
                     department_id, is_posted, posted_at, source_type, source_id)
                   VALUES ($1,$2,$3,$4,$5::party_type,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,
                           $18,$19,TRUE,$20,$21,$22)"#,
            )
            .bind(journal_line_id)
            .bind(journal_id)
            .bind(write.company_id)
            .bind(write.branch_id)
            .bind(&line.party_type)
            .bind(line.party_id)
            .bind(line_number)
            .bind(line.account_id)
            .bind(&acct.number)
            .bind(&acct.name)
            .bind(line.debit)
            .bind(line.credit)
            .bind(&write.currency)
            // base_debit_amount / base_credit_amount — base = IDR today (ADR-003); FX is producer-owned.
            .bind(line.debit)
            .bind(line.credit)
            .bind(&line.description)
            .bind(line.cost_center_id)
            .bind(line.project_id)
            .bind(line.department_id)
            .bind(now)
            .bind(&write.source_type)
            .bind(write.source_id)
            .execute(&mut *tx)
            .await?;

            line_inputs.push(LedgerEntryInput {
                journal_line_id,
                line: line.clone(),
            });
        }

        append_ledger_entries(
            &mut tx,
            write.company_id,
            write.branch_id,
            journal_id,
            &journal_number,
            write.posting_date,
            write.fiscal_period_id,
            write.fiscal_year,
            write.fiscal_month,
            &write.currency,
            write.description.as_deref(),
            is_reversing,
            &line_inputs,
            &accounts,
        )
        .await?;

        let post_id = Uuid::new_v4();
        let post_result = sqlx::query(
            r#"INSERT INTO accounting.accounting_posts
                (id, company_id, branch_id, source_type, source_id, source_reference, journal_id,
                 posting_type, posting_status, currency, total_debit, total_credit, posted_at,
                 posted_by, reverses_post_id, idempotency_key)
               VALUES ($1,$2,$3,$4::posting_source_type,$5,$6,$7,$8::posting_type,
                       'posted'::posting_status,$9,$10,$11,$12,$13,$14,$15)"#,
        )
        .bind(post_id)
        .bind(write.company_id)
        .bind(write.branch_id)
        .bind(&write.source_type)
        .bind(write.source_id)
        .bind(&write.source_reference)
        .bind(journal_id)
        .bind(&write.posting_type)
        .bind(&write.currency)
        .bind(total_debit)
        .bind(total_credit)
        .bind(now)
        .bind(write.posted_by)
        .bind(write.reverses_post_id)
        .bind(&write.idempotency_key)
        .execute(&mut *tx)
        .await;

        // Concurrency guard: the partial unique index is the real arbiter. On a race loss, roll
        // everything back (no partial write) and return the winner.
        if let Err(ref e) = post_result {
            if e.as_database_error().map(|d| d.is_unique_violation()).unwrap_or(false) {
                drop(tx);
                if let Some((existing_post, existing_journal)) = self
                    .find_existing_post(
                        write.company_id,
                        &write.source_type,
                        write.source_id,
                        &write.posting_type,
                        write.idempotency_key.as_deref(),
                    )
                    .await?
                {
                    return Ok(PostingCommit {
                        post_id: existing_post,
                        journal_id: existing_journal,
                        total_debit,
                        total_credit,
                        reused: true,
                    });
                }
                anyhow::bail!("concurrent posting conflict with no existing winner");
            }
        }
        post_result?;

        if is_reversing {
            if let Some(orig_post) = write.reverses_post_id {
                sqlx::query("UPDATE accounting.accounting_posts SET reversed_by_post_id=$1 WHERE id=$2")
                    .bind(post_id)
                    .bind(orig_post)
                    .execute(&mut *tx)
                    .await?;
            }
            if let Some(orig_journal) = write.reverses_journal_id {
                sqlx::query(
                    "UPDATE accounting.journals SET is_reversed=TRUE, reversed_by_id=$1, reversed_at=$2 WHERE id=$3",
                )
                .bind(journal_id)
                .bind(now)
                .bind(orig_journal)
                .execute(&mut *tx)
                .await?;
            }
        }

        tx.commit().await?;

        Ok(PostingCommit { post_id, journal_id, total_debit, total_credit, reused: false })
    }

    async fn find_manual_journal_for_post(
        &self,
        journal_id: Uuid,
        company_id: Uuid,
    ) -> anyhow::Result<Option<ManualJournalForPost>> {
        let journal = sqlx::query(
            r#"SELECT journal_number, branch_id, posting_date, fiscal_period_id, fiscal_year,
                      fiscal_month, currency, description, source_type::text AS source_type,
                      source_id, status::text AS status
               FROM accounting.journals
               WHERE id=$1 AND company_id=$2 AND (metadata->>'deleted_at') IS NULL"#,
        )
        .bind(journal_id)
        .bind(company_id)
        .fetch_optional(&self.pool)
        .await?;
        let Some(journal) = journal else {
            return Ok(None);
        };

        let rows = sqlx::query(
            r#"SELECT id, account_id, debit_amount, credit_amount, party_type::text AS pt, party_id,
                      cost_center_id, project_id, department_id, description
               FROM accounting.journal_lines
               WHERE journal_id=$1 AND company_id=$2 ORDER BY line_number"#,
        )
        .bind(journal_id)
        .bind(company_id)
        .fetch_all(&self.pool)
        .await?;

        let lines = rows
            .into_iter()
            .map(|r| {
                let line = PostingLine {
                    account_id: r.get("account_id"),
                    debit: r.get("debit_amount"),
                    credit: r.get("credit_amount"),
                    party_type: r.get("pt"),
                    party_id: r.get("party_id"),
                    cost_center_id: r.get("cost_center_id"),
                    project_id: r.get("project_id"),
                    department_id: r.get("department_id"),
                    description: r.get("description"),
                };
                (r.get::<Uuid, _>("id"), line)
            })
            .collect();

        Ok(Some(ManualJournalForPost {
            status: journal.get("status"),
            journal_number: journal.get("journal_number"),
            branch_id: journal.get("branch_id"),
            posting_date: journal.get("posting_date"),
            fiscal_period_id: journal.get("fiscal_period_id"),
            fiscal_year: journal.get("fiscal_year"),
            fiscal_month: journal.get("fiscal_month"),
            currency: journal.get("currency"),
            description: journal.get("description"),
            source_type: journal
                .get::<Option<String>, _>("source_type")
                .unwrap_or_else(|| "manual".to_string()),
            source_id: journal
                .get::<Option<Uuid>, _>("source_id")
                .unwrap_or(journal_id),
            lines,
        }))
    }

    async fn existing_post_for_journal(
        &self,
        journal_id: Uuid,
        company_id: Uuid,
    ) -> anyhow::Result<Option<Uuid>> {
        let id: Option<Uuid> = sqlx::query_scalar(
            "SELECT id FROM accounting.accounting_posts WHERE journal_id=$1 AND company_id=$2 AND posting_status='posted'::posting_status LIMIT 1",
        )
        .bind(journal_id)
        .bind(company_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(id)
    }

    async fn commit_manual_journal(
        &self,
        c: ManualJournalCommit,
    ) -> anyhow::Result<PostingCommit> {
        let now = c.now;
        let mut tx = self.pool.begin().await?;
        let accounts = load_accounts_locked(&mut tx, c.company_id, &c.lines.iter().map(|l| &l.line).cloned().collect::<Vec<_>>()).await?;

        let (total_debit, total_credit) = append_ledger_entries(
            &mut tx,
            c.company_id,
            c.branch_id,
            c.journal_id,
            &c.journal_number,
            c.posting_date,
            c.fiscal_period_id,
            c.fiscal_year,
            c.fiscal_month,
            &c.currency,
            c.description.as_deref(),
            false,
            &c.lines,
            &accounts,
        )
        .await?;

        sqlx::query(
            "UPDATE accounting.journals SET status='posted'::journal_status, posted_at=$1, posted_by=$2 WHERE id=$3",
        )
        .bind(now)
        .bind(c.posted_by)
        .bind(c.journal_id)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "UPDATE accounting.journal_lines SET is_posted=TRUE, posted_at=$1 WHERE journal_id=$2",
        )
        .bind(now)
        .bind(c.journal_id)
        .execute(&mut *tx)
        .await?;

        let post_id = Uuid::new_v4();
        let idem = format!("journal:{}", c.journal_id);
        sqlx::query(
            r#"INSERT INTO accounting.accounting_posts
                (id, company_id, branch_id, source_type, source_id, source_reference, journal_id,
                 posting_type, posting_status, currency, total_debit, total_credit, posted_at,
                 posted_by, idempotency_key)
               VALUES ($1,$2,$3,$4::posting_source_type,$5,$6,$7,$8::posting_type,
                       'posted'::posting_status,$9,$10,$11,$12,$13,$14)"#,
        )
        .bind(post_id)
        .bind(c.company_id)
        .bind(c.branch_id)
        .bind(&c.source_type)
        .bind(c.source_id)
        .bind(Option::<String>::None)
        .bind(c.journal_id)
        .bind("original")
        .bind(&c.currency)
        .bind(total_debit)
        .bind(total_credit)
        .bind(now)
        .bind(c.posted_by)
        .bind(&idem)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok(PostingCommit { post_id, journal_id: c.journal_id, total_debit, total_credit, reused: false })
    }

    async fn record_failed(&self, failed: FailedPost) -> anyhow::Result<()> {
        sqlx::query(
            r#"INSERT INTO accounting.accounting_posts
                (id, company_id, branch_id, source_type, source_id, source_reference, posting_type,
                 posting_status, currency, total_debit, total_credit, failed_at, error_code, error_message)
               VALUES ($1,$2,$3,$4::posting_source_type,$5,$6,$7::posting_type,
                       'failed'::posting_status,$8,$9,$10,$11,$12,$13)"#,
        )
        .bind(Uuid::new_v4())
        .bind(failed.company_id)
        .bind(failed.branch_id)
        .bind(&failed.source_type)
        .bind(failed.source_id)
        .bind(&failed.source_reference)
        .bind(&failed.posting_type)
        .bind(&failed.currency)
        .bind(failed.total_debit)
        .bind(failed.total_credit)
        .bind(failed.failed_at)
        .bind(&failed.error_code)
        .bind(&failed.error_message)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

// ---- private helpers (SQL) --------------------------------------------------

/// Lock the affected accounts FOR UPDATE (ascending id order) and return their snapshots.
async fn load_accounts_locked(
    tx: &mut sqlx::PgConnection,
    company_id: Uuid,
    lines: &[PostingLine],
) -> anyhow::Result<HashMap<Uuid, AccountInfo>> {
    let mut ids: Vec<Uuid> = lines.iter().map(|l| l.account_id).collect();
    ids.sort_unstable();
    ids.dedup();
    let rows = sqlx::query(
        r#"SELECT id, account_number, name, account_type::text AS at,
                  account_subtype::text AS st, normal_balance::text AS nb,
                  is_detail, is_header, status::text AS status, current_balance
           FROM accounting.accounts
           WHERE company_id=$1 AND id = ANY($2) AND (metadata->>'deleted_at') IS NULL
           ORDER BY id
           FOR UPDATE"#,
    )
    .bind(company_id)
    .bind(&ids)
    .fetch_all(tx)
    .await?;

    let mut map = HashMap::new();
    for row in rows {
        let id: Uuid = row.get("id");
        map.insert(
            id,
            AccountInfo {
                number: row.get("account_number"),
                name: row.get("name"),
                account_type: row.get("at"),
                normal_balance: row.get("nb"),
                current_balance: row.get("current_balance"),
            },
        );
    }
    Ok(map)
}

/// Append immutable ledger rows (running balance + monotonic sequence) and persist updated
/// account balances. Accounts MUST already be locked by the caller. Returns (total_debit, total_credit).
#[allow(clippy::too_many_arguments)]
async fn append_ledger_entries(
    tx: &mut sqlx::PgConnection,
    company_id: Uuid,
    branch_id: Option<Uuid>,
    journal_id: Uuid,
    journal_number: &str,
    posting_date: NaiveDate,
    fiscal_period_id: Option<Uuid>,
    fiscal_year: i32,
    fiscal_month: i32,
    currency: &str,
    description: Option<&str>,
    is_reversing: bool,
    lines: &[LedgerEntryInput],
    accounts: &HashMap<Uuid, AccountInfo>,
) -> anyhow::Result<(Decimal, Decimal)> {
    let mut running: HashMap<Uuid, Decimal> =
        accounts.iter().map(|(id, a)| (*id, a.current_balance)).collect();
    let mut seq: HashMap<Uuid, i32> = HashMap::new();
    for id in accounts.keys() {
        let max: i32 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(sequence_number),0) FROM accounting.ledgers WHERE company_id=$1 AND account_id=$2",
        )
        .bind(company_id)
        .bind(id)
        .fetch_one(&mut *tx)
        .await?;
        seq.insert(*id, max);
    }

    let mut total_debit = Decimal::ZERO;
    let mut total_credit = Decimal::ZERO;

    for input in lines {
        let line = &input.line;
        total_debit += line.debit;
        total_credit += line.credit;
        let acct = &accounts[&line.account_id];

        let change = if acct.normal_balance == "debit" {
            line.debit - line.credit
        } else {
            line.credit - line.debit
        };
        let before = *running.get(&line.account_id).unwrap();
        let after = before + change;
        running.insert(line.account_id, after);
        let s = seq.get_mut(&line.account_id).unwrap();
        *s += 1;
        let sequence_number = *s;

        let ledger_id = Uuid::new_v4();
        sqlx::query(
            r#"INSERT INTO accounting.ledgers
                (id, company_id, account_id, account_number, account_name, account_type,
                 normal_balance, journal_id, journal_number, journal_line_id, transaction_date,
                 posting_date, fiscal_period_id, fiscal_year, fiscal_month, description, currency,
                 debit_amount, credit_amount, balance_before, balance_after, balance_change,
                 sequence_number, branch_id, party_type, party_id, cost_center_id, project_id,
                 department_id, is_reversed)
               VALUES ($1,$2,$3,$4,$5,$6::account_type,$7::normal_balance,$8,$9,$10,$11,$12,$13,
                       $14,$15,$16,$17,$18,$19,$20,$21,$22,$23,$24,$25::party_type,$26,$27,$28,$29,$30)"#,
        )
        .bind(ledger_id)
        .bind(company_id)
        .bind(line.account_id)
        .bind(&acct.number)
        .bind(&acct.name)
        .bind(&acct.account_type)
        .bind(&acct.normal_balance)
        .bind(journal_id)
        .bind(journal_number)
        .bind(input.journal_line_id)
        .bind(posting_date)
        .bind(posting_date)
        .bind(fiscal_period_id)
        .bind(fiscal_year)
        .bind(fiscal_month)
        .bind(description.unwrap_or(&acct.name))
        .bind(currency)
        .bind(line.debit)
        .bind(line.credit)
        .bind(before)
        .bind(after)
        .bind(change)
        .bind(sequence_number)
        .bind(branch_id)
        .bind(&line.party_type)
        .bind(line.party_id)
        .bind(line.cost_center_id)
        .bind(line.project_id)
        .bind(line.department_id)
        .bind(is_reversing)
        .execute(&mut *tx)
        .await?;

        sqlx::query("UPDATE accounting.journal_lines SET ledger_id=$1 WHERE id=$2")
            .bind(ledger_id)
            .bind(input.journal_line_id)
            .execute(&mut *tx)
            .await?;
    }

    for (account_id, balance) in &running {
        sqlx::query("UPDATE accounting.accounts SET current_balance=$1 WHERE id=$2")
            .bind(balance)
            .bind(account_id)
            .execute(&mut *tx)
            .await?;
    }

    Ok((total_debit, total_credit))
}
