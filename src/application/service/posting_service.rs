//! GL-posting service — the inbound port of the GL-posting contract.
//!
//! Hand-authored behavior (NOT generated). Implements double-entry posting per
//! `docs/erp/gl-posting-contract.md` and the golden cases in
//! `docs/business-flows/golden-cases.md`:
//!   - validate: ≥2 lines, Σdebit = Σcredit, party required iff AR/AP, postable account,
//!     open fiscal period
//!   - within one transaction: write a Journal + JournalLines + immutable Ledger rows
//!     (running balance per account), update account balances, mark the AccountingPost posted
//!   - reversal: derive swapped lines from the original journal, link both ways
//!   - idempotency: a second post with the same (company, source_type, source_id, posting_type)
//!     returns the original instead of double-posting
//!
//! This file is user-owned (see `metaphor.codegen.yaml`) and survives regeneration.

use std::collections::HashMap;
use std::sync::Arc;

use chrono::{Datelike, DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use uuid::Uuid;

/// One debit/credit line of a posting request. Exactly one of `debit`/`credit` is > 0.
#[derive(Debug, Clone)]
pub struct PostingLine {
    pub account_id: Uuid,
    pub debit: Decimal,
    pub credit: Decimal,
    pub party_type: Option<String>, // "customer" | "supplier" | "employee"
    pub party_id: Option<Uuid>,
    pub cost_center_id: Option<Uuid>,
    pub project_id: Option<Uuid>,
    pub department_id: Option<Uuid>,
    pub description: Option<String>,
}

/// A request to record a balanced set of lines in the GL (the inbound contract shape).
#[derive(Debug, Clone)]
pub struct PostingRequest {
    pub company_id: Uuid,
    pub branch_id: Option<Uuid>,
    pub source_type: String, // posting_source_type: order|payment|settlement|refund|expense|inventory|manual
    pub source_id: Uuid,
    pub source_reference: Option<String>,
    pub posting_date: NaiveDate,
    pub currency: String,
    pub posting_type: String, // "original" | "reversal"
    pub reverses_post_id: Option<Uuid>,
    pub description: Option<String>,
    pub lines: Vec<PostingLine>,
    /// The REAL dedup key when set: two posts with the same `(company_id, idempotency_key)` collapse to one,
    /// and the producer may reuse `source_id` across its several posts. When `None`, dedup falls back to the
    /// tuple `(company_id, source_type, source_id, posting_type)` (backward compatible). A multi-post
    /// producer should EITHER set a distinct key per post OR namespace `source_id` — see the GL-posting
    /// contract §3.5.
    pub idempotency_key: Option<String>,
}

impl PostingRequest {
    /// Convenience constructor for an original posting.
    pub fn original(company_id: Uuid, source_type: &str, source_id: Uuid, posting_date: NaiveDate) -> Self {
        Self {
            company_id,
            branch_id: None,
            source_type: source_type.to_string(),
            source_id,
            source_reference: None,
            posting_date,
            currency: "IDR".to_string(),
            posting_type: "original".to_string(),
            reverses_post_id: None,
            description: None,
            lines: Vec::new(),
            idempotency_key: None,
        }
    }

    /// Set the idempotency key (the real per-post dedup identity). Prefer this over hand-namespacing
    /// `source_id` when a producer emits more than one post per source document.
    pub fn with_idempotency_key(mut self, key: impl Into<String>) -> Self {
        self.idempotency_key = Some(key.into());
        self
    }
}

/// Outcome of a successful post.
#[derive(Debug, Clone)]
pub struct PostingResult {
    pub post_id: Uuid,
    pub journal_id: Uuid,
    pub posting_status: String,
    /// True when an existing posted entry was returned instead of writing a new one.
    pub idempotent_reuse: bool,
}

/// Typed posting failure. `code()` is the stable error string asserted by the golden cases.
#[derive(Debug)]
pub enum PostingError {
    TooFewLines,
    Unbalanced,
    NonPostableAccount(String),
    AccountNotFound(Uuid),
    PartyRequired(String),
    PartyNotAllowed(String),
    PeriodClosed,
    Conflict(String),
    Db(sqlx::Error),
}

impl PostingError {
    pub fn code(&self) -> &'static str {
        match self {
            PostingError::TooFewLines => "too_few_lines",
            PostingError::Unbalanced => "unbalanced",
            PostingError::NonPostableAccount(_) => "non_postable_account",
            PostingError::AccountNotFound(_) => "account_not_found",
            PostingError::PartyRequired(_) => "party_required",
            PostingError::PartyNotAllowed(_) => "party_not_allowed",
            PostingError::PeriodClosed => "period_closed",
            PostingError::Conflict(_) => "conflict",
            PostingError::Db(_) => "internal_error",
        }
    }

    /// HTTP status: validation → 422, missing account → 404, db → 500.
    pub fn http_status(&self) -> u16 {
        match self {
            PostingError::AccountNotFound(_) => 404,
            PostingError::Db(_) => 500,
            _ => 422,
        }
    }
}

impl std::fmt::Display for PostingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.code())
    }
}
impl std::error::Error for PostingError {}

impl From<sqlx::Error> for PostingError {
    fn from(e: sqlx::Error) -> Self {
        PostingError::Db(e)
    }
}

/// Per-account snapshot loaded for validation + denormalization.
struct AccountInfo {
    number: String,
    name: String,
    account_type: String,
    subtype: String,
    normal_balance: String, // "debit" | "credit"
    is_detail: bool,
    is_header: bool,
    status: String,
    current_balance: Decimal,
}

/// Context shared by every ledger-write path (`post()` and `post_journal()`). Carries everything
/// the per-line ledger insert needs that is constant across all lines of one journal.
struct LedgerCtx {
    company_id: Uuid,
    branch_id: Option<Uuid>,
    journal_id: Uuid,
    journal_number: String,
    posting_date: NaiveDate,
    fiscal_period_id: Option<Uuid>,
    fiscal_year: i32,
    fiscal_month: i32,
    currency: String,
    source_type: String,
    source_id: Uuid,
    description: Option<String>,
    is_reversing: bool,
    now: DateTime<Utc>,
}

/// One line to write to the ledger. `journal_line_id` is the already-persisted `journal_lines`
/// row this ledger entry back-references — freshly created by `post()`, or loaded from an
/// existing draft journal by `post_journal()`.
struct LedgerEntryInput {
    journal_line_id: Uuid,
    account_id: Uuid,
    debit: Decimal,
    credit: Decimal,
    party_type: Option<String>,
    party_id: Option<Uuid>,
    cost_center_id: Option<Uuid>,
    project_id: Option<Uuid>,
    department_id: Option<Uuid>,
    description: Option<String>,
}

/// Sink for GL-posting domain events (the event-bus seam). Fire-and-forget. A real adapter
/// (e.g. backbone-messaging) implements this; tests use a recording sink; default logs.
pub trait PostingEventSink: Send + Sync {
    fn publish(&self, event: PostingEvent);
}

/// Default sink — emits structured tracing events.
pub struct LoggingSink;

impl PostingEventSink for LoggingSink {
    fn publish(&self, event: PostingEvent) {
        match &event {
            PostingEvent::AccountingPostPosted(e) => tracing::info!(
                target: "accounting.events", post_id = %e.post_id, journal_id = %e.journal_id,
                source_type = %e.source_type, "AccountingPostPosted"
            ),
            PostingEvent::AccountingPostFailed(e) => tracing::warn!(
                target: "accounting.events", source_type = %e.source_type, code = %e.error_code,
                "AccountingPostFailed"
            ),
        }
    }
}

/// The GL-posting service. Owns a pool; every post runs in one transaction.
#[derive(Clone)]
pub struct PostingService {
    db_pool: PgPool,
    sink: Arc<dyn PostingEventSink>,
}

impl PostingService {
    pub fn new(db_pool: PgPool) -> Self {
        Self { db_pool, sink: Arc::new(LoggingSink) }
    }

    /// Construct with a custom event sink (real bus adapter or a test recorder).
    pub fn with_sink(db_pool: PgPool, sink: Arc<dyn PostingEventSink>) -> Self {
        Self { db_pool, sink }
    }

    /// Record a balanced posting. Idempotent on (company, source_type, source_id, posting_type).
    pub async fn post(
        &self,
        mut req: PostingRequest,
        posted_by: Option<Uuid>,
    ) -> Result<PostingResult, PostingError> {
        // Idempotency: return the existing posted entry for the same source identity.
        if let Some((post_id, journal_id)) = self.find_posted(&req).await? {
            return Ok(PostingResult {
                post_id,
                journal_id,
                posting_status: "posted".to_string(),
                idempotent_reuse: true,
            });
        }

        // Reversal derives its (swapped) lines from the original journal.
        let reverses_journal_id = if req.posting_type == "reversal" {
            Some(self.build_reversal_lines(&mut req).await?)
        } else {
            None
        };

        // Validate; on failure record a failed AccountingPost (audit) and return the code.
        if let Err(e) = self.validate(&req).await {
            let _ = self.record_failed(&req, &e).await; // best-effort audit
            return Err(e);
        }

        // ---- write everything in one transaction ----
        let now = Utc::now();
        let mut tx = self.db_pool.begin().await?;

        let total_debit: Decimal = req.lines.iter().map(|l| l.debit).sum();
        let total_credit: Decimal = req.lines.iter().map(|l| l.credit).sum();
        let fiscal_year = req.posting_date.year();
        let fiscal_month = req.posting_date.month() as i32;
        let fiscal_period_id = self.find_period_id(&req).await?;
        // Lock the accounts we are about to mutate (FOR UPDATE, in deterministic id order) BEFORE
        // reading current_balance / MAX(sequence_number). Without this, two concurrent posts from
        // distinct sources to the SAME account both read the same balance/sequence and corrupt the
        // running-balance chain (duplicate sequence_number, last-writer-wins current_balance).
        // The lock serializes only same-account writers; cross-account posts stay fully parallel.
        let accounts = self.load_accounts_locked(&mut *tx, &req).await?;

        let journal_id = Uuid::new_v4();
        let journal_number = format!(
            "JV-{}-{}",
            req.posting_date.format("%Y%m%d"),
            &Uuid::new_v4().to_string()[..8]
        );
        let (journal_type, journal_source) = map_source(&req.source_type, &req.posting_type);
        let is_reversing = req.posting_type == "reversal";

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
        .bind(req.company_id)
        .bind(req.branch_id)
        .bind(&journal_number)
        .bind(journal_type)
        .bind(req.posting_date)
        .bind(req.posting_date)
        .bind(fiscal_period_id)
        .bind(fiscal_year)
        .bind(fiscal_month)
        .bind(req.description.clone().unwrap_or_else(|| format!("{} posting", req.source_type)))
        .bind(&req.currency)
        .bind(total_debit)
        .bind(total_credit)
        .bind(req.lines.len() as i32)
        .bind(journal_source)
        .bind(&req.source_type)
        .bind(req.source_id)
        .bind(&req.source_reference)
        .bind(is_reversing)
        .bind(reverses_journal_id)
        .bind(now)
        .bind(posted_by)
        .execute(&mut *tx)
        .await?;

        // Create the journal_lines rows (is_posted=TRUE on creation here), collecting the inputs
        // the shared ledger-write core needs. The immutable ledger rows + running balances are
        // written by `append_ledger_entries`, shared with `post_journal()`.
        let mut line_inputs: Vec<LedgerEntryInput> = Vec::with_capacity(req.lines.len());
        for (i, line) in req.lines.iter().enumerate() {
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
            .bind(req.company_id)
            .bind(req.branch_id)
            .bind(&line.party_type)
            .bind(line.party_id)
            .bind(line_number)
            .bind(line.account_id)
            .bind(&acct.number)
            .bind(&acct.name)
            .bind(line.debit)
            .bind(line.credit)
            .bind(&req.currency)
            // base_debit_amount / base_credit_amount — base = IDR today; the GL is single-currency
            // and producers must convert to base currency before posting (ADR-003). The columns are
            // retained so a future FX layer can populate them without a schema change.
            .bind(line.debit)
            .bind(line.credit)
            .bind(&line.description)
            .bind(line.cost_center_id)
            .bind(line.project_id)
            .bind(line.department_id)
            .bind(now)
            .bind(&req.source_type)
            .bind(req.source_id)
            .execute(&mut *tx)
            .await?;

            line_inputs.push(LedgerEntryInput {
                journal_line_id,
                account_id: line.account_id,
                debit: line.debit,
                credit: line.credit,
                party_type: line.party_type.clone(),
                party_id: line.party_id,
                cost_center_id: line.cost_center_id,
                project_id: line.project_id,
                department_id: line.department_id,
                description: line.description.clone(),
            });
        }

        let ledger_ctx = LedgerCtx {
            company_id: req.company_id,
            branch_id: req.branch_id,
            journal_id,
            journal_number: journal_number.clone(),
            posting_date: req.posting_date,
            fiscal_period_id,
            fiscal_year,
            fiscal_month,
            currency: req.currency.clone(),
            source_type: req.source_type.clone(),
            source_id: req.source_id,
            description: req.description.clone(),
            is_reversing,
            now,
        };
        Self::append_ledger_entries(&mut *tx, &ledger_ctx, &line_inputs, &accounts).await?;

        // The AccountingPost row (the contract record).
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
        .bind(req.company_id)
        .bind(req.branch_id)
        .bind(&req.source_type)
        .bind(req.source_id)
        .bind(&req.source_reference)
        .bind(journal_id)
        .bind(&req.posting_type)
        .bind(&req.currency)
        .bind(total_debit)
        .bind(total_credit)
        .bind(now)
        .bind(posted_by)
        .bind(req.reverses_post_id)
        .bind(&req.idempotency_key)
        .execute(&mut *tx)
        .await;

        // Concurrency guard: the partial unique index (company, source_type, source_id,
        // posting_type) WHERE posting_status='posted' is the real arbiter. If a concurrent post
        // for the same source won the race, our insert violates it — roll everything back (no
        // partial write) and return the winner.
        if let Err(ref e) = post_result {
            if e.as_database_error().map(|d| d.is_unique_violation()).unwrap_or(false) {
                drop(tx); // rollback — no partial write
                return match self.find_posted(&req).await? {
                    Some((existing_post, existing_journal)) => Ok(PostingResult {
                        post_id: existing_post,
                        journal_id: existing_journal,
                        posting_status: "posted".to_string(),
                        idempotent_reuse: true,
                    }),
                    None => Err(PostingError::Conflict("concurrent posting conflict".into())),
                };
            }
        }
        post_result?; // propagate any non-uniqueness error

        // Reversal links: original post + original journal point back to the reversing pair.
        if is_reversing {
            if let Some(orig_post) = req.reverses_post_id {
                sqlx::query("UPDATE accounting.accounting_posts SET reversed_by_post_id=$1 WHERE id=$2")
                    .bind(post_id)
                    .bind(orig_post)
                    .execute(&mut *tx)
                    .await?;
            }
            if let Some(orig_journal) = reverses_journal_id {
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

        // Contract event (fire-and-forget) — the public extension seam.
        self.sink.publish(PostingEvent::AccountingPostPosted(AccountingPostPosted {
            post_id,
            journal_id,
            company_id: req.company_id,
            source_type: req.source_type.clone(),
            source_id: req.source_id,
            total_debit,
            total_credit,
            occurred_at: now,
        }));

        Ok(PostingResult {
            post_id,
            journal_id,
            posting_status: "posted".to_string(),
            idempotent_reuse: false,
        })
    }

    /// Post an existing **approved** manual journal to the ledger. Idempotent: a journal already
    /// `posted` returns its existing post. This is the second caller of the shared
    /// `append_ledger_entries` core — used by the journal approval workflow (approve → post). The
    /// journal's lines were persisted at draft time (is_posted=false); this writes their immutable
    /// ledger rows, flips the journal to `posted`, and records the `AccountingPost`.
    pub async fn post_journal(
        &self,
        journal_id: Uuid,
        company_id: Uuid,
        posted_by: Option<Uuid>,
    ) -> Result<PostingResult, PostingError> {
        // Load the journal header (must exist + match tenant).
        let journal = sqlx::query(
            r#"SELECT journal_number, branch_id, posting_date, fiscal_period_id, fiscal_year,
                      fiscal_month, currency, description, source_type::text AS source_type,
                      source_id, status::text AS status
               FROM accounting.journals
               WHERE id=$1 AND company_id=$2 AND (metadata->>'deleted_at') IS NULL"#,
        )
        .bind(journal_id)
        .bind(company_id)
        .fetch_optional(&self.db_pool)
        .await?
        .ok_or_else(|| PostingError::Conflict(format!("journal {journal_id} not found")))?;

        let status: String = journal.get("status");
        if status == "posted" {
            // Idempotent: return the existing post for this journal, if any.
            let existing: Option<Uuid> = sqlx::query_scalar(
                "SELECT id FROM accounting.accounting_posts WHERE journal_id=$1 AND company_id=$2 AND posting_status='posted'::posting_status LIMIT 1",
            )
            .bind(journal_id)
            .bind(company_id)
            .fetch_optional(&self.db_pool)
            .await?;
            return Ok(PostingResult {
                post_id: existing.unwrap_or_default(),
                journal_id,
                posting_status: "posted".to_string(),
                idempotent_reuse: true,
            });
        }
        if status != "approved" {
            return Err(PostingError::Conflict(format!(
                "journal {journal_id} is '{status}', must be 'approved' to post"
            )));
        }

        let journal_number: String = journal.get("journal_number");
        let branch_id: Option<Uuid> = journal.get("branch_id");
        let posting_date: NaiveDate = journal.get("posting_date");
        let fiscal_period_id: Option<Uuid> = journal.get("fiscal_period_id");
        let fiscal_year: i32 = journal.get("fiscal_year");
        let fiscal_month: i32 = journal.get("fiscal_month");
        let currency: String = journal.get("currency");
        let description: Option<String> = journal.get("description");
        let source_type: String = journal
            .get::<Option<String>, _>("source_type")
            .unwrap_or_else(|| "manual".to_string());
        let source_id: Uuid = journal.get::<Option<Uuid>, _>("source_id").unwrap_or(journal_id);

        // Load lines (ordered) → validation request + ledger inputs.
        let rows = sqlx::query(
            r#"SELECT id, account_id, debit_amount, credit_amount, party_type::text AS pt, party_id,
                      cost_center_id, project_id, department_id, description
               FROM accounting.journal_lines
               WHERE journal_id=$1 AND company_id=$2 ORDER BY line_number"#,
        )
        .bind(journal_id)
        .bind(company_id)
        .fetch_all(&self.db_pool)
        .await?;

        let mut req = PostingRequest {
            company_id,
            branch_id,
            source_type: source_type.clone(),
            source_id,
            source_reference: None,
            posting_date,
            currency: currency.clone(),
            posting_type: "original".to_string(),
            reverses_post_id: None,
            description: description.clone(),
            lines: Vec::new(),
            idempotency_key: None,
        };
        let mut line_inputs: Vec<LedgerEntryInput> = Vec::with_capacity(rows.len());
        for r in rows {
            let debit: Decimal = r.get("debit_amount");
            let credit: Decimal = r.get("credit_amount");
            let account_id: Uuid = r.get("account_id");
            let party_type: Option<String> = r.get("pt");
            let party_id: Option<Uuid> = r.get("party_id");
            let cost_center_id: Option<Uuid> = r.get("cost_center_id");
            let project_id: Option<Uuid> = r.get("project_id");
            let department_id: Option<Uuid> = r.get("department_id");
            let line_desc: Option<String> = r.get("description");
            req.lines.push(PostingLine {
                account_id,
                debit,
                credit,
                party_type: party_type.clone(),
                party_id,
                cost_center_id,
                project_id,
                department_id,
                description: line_desc.clone(),
            });
            line_inputs.push(LedgerEntryInput {
                journal_line_id: r.get("id"),
                account_id,
                debit,
                credit,
                party_type,
                party_id,
                cost_center_id,
                project_id,
                department_id,
                description: line_desc,
            });
        }

        // Validate (balance, ≥2 lines, party-for-AR/AP, postable, open period) on the stored lines.
        if let Err(e) = self.validate(&req).await {
            let _ = self.record_failed(&req, &e).await;
            return Err(e);
        }

        let now = Utc::now();
        let mut tx = self.db_pool.begin().await?;
        let accounts = self.load_accounts_locked(&mut *tx, &req).await?;

        let ctx = LedgerCtx {
            company_id,
            branch_id,
            journal_id,
            journal_number,
            posting_date,
            fiscal_period_id,
            fiscal_year,
            fiscal_month,
            currency,
            source_type: source_type.clone(),
            source_id,
            description,
            is_reversing: false,
            now,
        };
        let (total_debit, total_credit) =
            Self::append_ledger_entries(&mut *tx, &ctx, &line_inputs, &accounts).await?;

        // Flip the journal to posted; mark its lines posted.
        sqlx::query(
            "UPDATE accounting.journals SET status='posted'::journal_status, posted_at=$1, posted_by=$2 WHERE id=$3",
        )
        .bind(now)
        .bind(posted_by)
        .bind(journal_id)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "UPDATE accounting.journal_lines SET is_posted=TRUE, posted_at=$1 WHERE journal_id=$2",
        )
        .bind(now)
        .bind(journal_id)
        .execute(&mut *tx)
        .await?;

        // Record the AccountingPost (source_type=manual, source_id=journal_id) with a real per-post
        // idempotency key so a replay collapses to this post.
        let post_id = Uuid::new_v4();
        let idem = format!("journal:{journal_id}");
        sqlx::query(
            r#"INSERT INTO accounting.accounting_posts
                (id, company_id, branch_id, source_type, source_id, source_reference, journal_id,
                 posting_type, posting_status, currency, total_debit, total_credit, posted_at,
                 posted_by, idempotency_key)
               VALUES ($1,$2,$3,$4::posting_source_type,$5,$6,$7,$8::posting_type,
                       'posted'::posting_status,$9,$10,$11,$12,$13,$14)"#,
        )
        .bind(post_id)
        .bind(company_id)
        .bind(branch_id)
        .bind(&source_type)
        .bind(source_id)
        .bind(Option::<String>::None)
        .bind(journal_id)
        .bind("original")
        .bind(&req.currency)
        .bind(total_debit)
        .bind(total_credit)
        .bind(now)
        .bind(posted_by)
        .bind(&idem)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        self.sink.publish(PostingEvent::AccountingPostPosted(AccountingPostPosted {
            post_id,
            journal_id,
            company_id,
            source_type,
            source_id,
            total_debit,
            total_credit,
            occurred_at: now,
        }));

        Ok(PostingResult {
            post_id,
            journal_id,
            posting_status: "posted".to_string(),
            idempotent_reuse: false,
        })
    }

    // ---- validation ----------------------------------------------------------

    async fn validate(&self, req: &PostingRequest) -> Result<(), PostingError> {
        if req.lines.len() < 2 {
            return Err(PostingError::TooFewLines);
        }
        let total_debit: Decimal = req.lines.iter().map(|l| l.debit).sum();
        let total_credit: Decimal = req.lines.iter().map(|l| l.credit).sum();
        if total_debit != total_credit {
            return Err(PostingError::Unbalanced);
        }

        let accounts = self.load_accounts(req).await?;
        for line in &req.lines {
            let acct = accounts
                .get(&line.account_id)
                .ok_or(PostingError::AccountNotFound(line.account_id))?;
            if acct.is_header || !acct.is_detail || acct.status != "active" {
                return Err(PostingError::NonPostableAccount(acct.number.clone()));
            }
            let is_party_account =
                acct.subtype == "accounts_receivable" || acct.subtype == "accounts_payable";
            let has_party = line.party_type.is_some() && line.party_id.is_some();
            if is_party_account && !has_party {
                return Err(PostingError::PartyRequired(acct.number.clone()));
            }
            if !is_party_account && (line.party_type.is_some() || line.party_id.is_some()) {
                return Err(PostingError::PartyNotAllowed(acct.number.clone()));
            }
        }

        // Closed/locked fiscal period blocks posting; absent or open period is fine.
        let blocked: Option<bool> = sqlx::query_scalar(
            r#"SELECT bool_or(status IN ('closed','locked'))
               FROM accounting.fiscal_periods
               WHERE company_id=$1 AND start_date<=$2 AND end_date>=$2
                 AND (metadata->>'deleted_at') IS NULL"#,
        )
        .bind(req.company_id)
        .bind(req.posting_date)
        .fetch_one(&self.db_pool)
        .await?;
        if blocked == Some(true) {
            return Err(PostingError::PeriodClosed);
        }

        Ok(())
    }

    // ---- helpers -------------------------------------------------------------

    async fn load_accounts(
        &self,
        req: &PostingRequest,
    ) -> Result<HashMap<Uuid, AccountInfo>, PostingError> {
        let ids: Vec<Uuid> = req.lines.iter().map(|l| l.account_id).collect();
        let rows = sqlx::query(
            r#"SELECT id, account_number, name, account_type::text AS at,
                      account_subtype::text AS st, normal_balance::text AS nb,
                      is_detail, is_header, status::text AS status, current_balance
               FROM accounting.accounts
               WHERE company_id=$1 AND id = ANY($2) AND (metadata->>'deleted_at') IS NULL"#,
        )
        .bind(req.company_id)
        .bind(&ids)
        .fetch_all(&self.db_pool)
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
                    subtype: row.get("st"),
                    normal_balance: row.get("nb"),
                    is_detail: row.get("is_detail"),
                    is_header: row.get("is_header"),
                    status: row.get("status"),
                    current_balance: row.get("current_balance"),
                },
            );
        }
        Ok(map)
    }

    /// Same projection as `load_accounts`, but runs INSIDE the posting transaction and takes
    /// `SELECT ... FOR UPDATE` on the affected account rows (visited in ascending id order so
    /// concurrent multi-account posts can't deadlock). This is the concurrency fence: it makes the
    /// `current_balance` + `MAX(sequence_number)` reads that follow authoritative and serialized
    /// per account, so distinct-source posts to one account can't interleave.
    async fn load_accounts_locked(
        &self,
        conn: &mut sqlx::PgConnection,
        req: &PostingRequest,
    ) -> Result<HashMap<Uuid, AccountInfo>, PostingError> {
        let mut ids: Vec<Uuid> = req.lines.iter().map(|l| l.account_id).collect();
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
        .bind(req.company_id)
        .bind(&ids)
        .fetch_all(conn)
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
                    subtype: row.get("st"),
                    normal_balance: row.get("nb"),
                    is_detail: row.get("is_detail"),
                    is_header: row.get("is_header"),
                    status: row.get("status"),
                    current_balance: row.get("current_balance"),
                },
            );
        }
        Ok(map)
    }

    /// Shared ledger-write core: for each line, append an immutable `accounting.ledgers` row with
    /// running balance + monotonic `sequence_number`, back-link the journal_line, and persist the
    /// updated account balances. The accounts MUST already be locked (`FOR UPDATE`) by the caller
    /// so the `MAX(sequence_number)` / `current_balance` reads are authoritative and serialized per
    /// account. Used by both `post()` (fresh journal_lines) and `post_journal()` (existing draft
    /// journal_lines). Returns `(total_debit, total_credit)`.
    async fn append_ledger_entries(
        tx: &mut sqlx::PgConnection,
        ctx: &LedgerCtx,
        lines: &[LedgerEntryInput],
        accounts: &HashMap<Uuid, AccountInfo>,
    ) -> Result<(Decimal, Decimal), PostingError> {
        // Running balance + sequence per account (seeded from the locked accounts + existing ledger).
        let mut running: HashMap<Uuid, Decimal> =
            accounts.iter().map(|(id, a)| (*id, a.current_balance)).collect();
        let mut seq: HashMap<Uuid, i32> = HashMap::new();
        for id in accounts.keys() {
            let max: i32 = sqlx::query_scalar(
                "SELECT COALESCE(MAX(sequence_number),0) FROM accounting.ledgers WHERE company_id=$1 AND account_id=$2",
            )
            .bind(ctx.company_id)
            .bind(id)
            .fetch_one(&mut *tx)
            .await?;
            seq.insert(*id, max);
        }

        let mut total_debit = Decimal::ZERO;
        let mut total_credit = Decimal::ZERO;

        for line in lines {
            total_debit += line.debit;
            total_credit += line.credit;
            let acct = &accounts[&line.account_id];

            // balance_change per normal-balance side; ledger stores raw non-negative sides.
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
            .bind(ctx.company_id)
            .bind(line.account_id)
            .bind(&acct.number)
            .bind(&acct.name)
            .bind(&acct.account_type)
            .bind(&acct.normal_balance)
            .bind(ctx.journal_id)
            .bind(&ctx.journal_number)
            .bind(line.journal_line_id)
            .bind(ctx.posting_date)
            .bind(ctx.posting_date)
            .bind(ctx.fiscal_period_id)
            .bind(ctx.fiscal_year)
            .bind(ctx.fiscal_month)
            .bind(ctx.description.clone().unwrap_or_else(|| acct.name.clone()))
            .bind(&ctx.currency)
            .bind(line.debit)
            .bind(line.credit)
            .bind(before)
            .bind(after)
            .bind(change)
            .bind(sequence_number)
            .bind(ctx.branch_id)
            .bind(&line.party_type)
            .bind(line.party_id)
            .bind(line.cost_center_id)
            .bind(line.project_id)
            .bind(line.department_id)
            .bind(ctx.is_reversing)
            .execute(&mut *tx)
            .await?;

            sqlx::query("UPDATE accounting.journal_lines SET ledger_id=$1 WHERE id=$2")
                .bind(ledger_id)
                .bind(line.journal_line_id)
                .execute(&mut *tx)
                .await?;
        }

        // Persist updated running balances.
        for (account_id, balance) in &running {
            sqlx::query("UPDATE accounting.accounts SET current_balance=$1 WHERE id=$2")
                .bind(balance)
                .bind(account_id)
                .execute(&mut *tx)
                .await?;
        }

        Ok((total_debit, total_credit))
    }

    async fn find_period_id(&self, req: &PostingRequest) -> Result<Option<Uuid>, PostingError> {
        let id: Option<Uuid> = sqlx::query_scalar(
            r#"SELECT id FROM accounting.fiscal_periods
               WHERE company_id=$1 AND start_date<=$2 AND end_date>=$2
                 AND (metadata->>'deleted_at') IS NULL
               ORDER BY (end_date - start_date) ASC LIMIT 1"#,
        )
        .bind(req.company_id)
        .bind(req.posting_date)
        .fetch_optional(&self.db_pool)
        .await?;
        Ok(id)
    }

    async fn find_posted(
        &self,
        req: &PostingRequest,
    ) -> Result<Option<(Uuid, Uuid)>, PostingError> {
        // When the producer set an idempotency_key, THAT is the dedup identity (a post may reuse source_id
        // across its several posts); otherwise fall back to the legacy tuple.
        let row = if let Some(key) = &req.idempotency_key {
            sqlx::query(
                r#"SELECT id, journal_id FROM accounting.accounting_posts
                   WHERE company_id=$1 AND idempotency_key=$2 AND posting_status='posted'::posting_status
                   LIMIT 1"#,
            )
            .bind(req.company_id)
            .bind(key)
            .fetch_optional(&self.db_pool)
            .await?
        } else {
            sqlx::query(
                r#"SELECT id, journal_id FROM accounting.accounting_posts
                   WHERE company_id=$1 AND source_type=$2::posting_source_type AND source_id=$3
                     AND posting_type=$4::posting_type AND posting_status='posted'::posting_status
                   LIMIT 1"#,
            )
            .bind(req.company_id)
            .bind(&req.source_type)
            .bind(req.source_id)
            .bind(&req.posting_type)
            .fetch_optional(&self.db_pool)
            .await?
        };
        Ok(row.and_then(|r| {
            let id: Uuid = r.get("id");
            let jid: Option<Uuid> = r.get("journal_id");
            jid.map(|j| (id, j))
        }))
    }

    /// Load the original journal's lines, swap debit/credit, and set req.lines. Returns the
    /// original journal id (for the reversal links).
    async fn build_reversal_lines(
        &self,
        req: &mut PostingRequest,
    ) -> Result<Uuid, PostingError> {
        let orig_post_id = req
            .reverses_post_id
            .ok_or_else(|| PostingError::Conflict("reversal requires reverses_post_id".into()))?;
        let orig_journal_id: Uuid = sqlx::query_scalar::<_, Option<Uuid>>(
            "SELECT journal_id FROM accounting.accounting_posts WHERE id=$1 AND company_id=$2 AND posting_status='posted'::posting_status",
        )
        .bind(orig_post_id)
        .bind(req.company_id)
        .fetch_optional(&self.db_pool)
        .await?
        .flatten()
        .ok_or_else(|| PostingError::Conflict("original posting not found or not posted".into()))?;

        let rows = sqlx::query(
            r#"SELECT account_id, debit_amount, credit_amount, party_type::text AS pt, party_id,
                      cost_center_id, project_id, department_id
               FROM accounting.journal_lines WHERE journal_id=$1 AND company_id=$2 ORDER BY line_number"#,
        )
        .bind(orig_journal_id)
        .bind(req.company_id)
        .fetch_all(&self.db_pool)
        .await?;

        req.lines = rows
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
        Ok(orig_journal_id)
    }

    async fn record_failed(&self, req: &PostingRequest, err: &PostingError) -> Result<(), sqlx::Error> {
        let total_debit: Decimal = req.lines.iter().map(|l| l.debit).sum();
        let total_credit: Decimal = req.lines.iter().map(|l| l.credit).sum();
        sqlx::query(
            r#"INSERT INTO accounting.accounting_posts
                (id, company_id, branch_id, source_type, source_id, source_reference, posting_type,
                 posting_status, currency, total_debit, total_credit, failed_at, error_code, error_message)
               VALUES ($1,$2,$3,$4::posting_source_type,$5,$6,$7::posting_type,
                       'failed'::posting_status,$8,$9,$10,$11,$12,$13)"#,
        )
        .bind(Uuid::new_v4())
        .bind(req.company_id)
        .bind(req.branch_id)
        .bind(&req.source_type)
        .bind(req.source_id)
        .bind(&req.source_reference)
        .bind(&req.posting_type)
        .bind(&req.currency)
        .bind(total_debit)
        .bind(total_credit)
        .bind(Utc::now())
        .bind(err.code())
        .bind(err.to_string())
        .execute(&self.db_pool)
        .await?;
        self.sink.publish(PostingEvent::AccountingPostFailed(AccountingPostFailed {
            company_id: req.company_id,
            source_type: req.source_type.clone(),
            source_id: req.source_id,
            error_code: err.code().to_string(),
            error_message: err.to_string(),
            occurred_at: Utc::now(),
        }));
        Ok(())
    }
}

// =============================================================================
// Exported domain events (the public extension surface for the GL-posting contract).
// Live here (a user-owned file) so they survive regeneration; also re-exported from
// `application::service` and mirrored in schema/hooks/journal.hook.yaml.
// =============================================================================

/// Published when the GL-posting service records a balanced entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountingPostPosted {
    pub post_id: Uuid,
    pub journal_id: Uuid,
    pub company_id: Uuid,
    pub source_type: String,
    pub source_id: Uuid,
    pub total_debit: Decimal,
    pub total_credit: Decimal,
    pub occurred_at: DateTime<Utc>,
}

/// Published when a posting is rejected (validation failure). Carries the stable error code.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountingPostFailed {
    pub company_id: Uuid,
    pub source_type: String,
    pub source_id: Uuid,
    pub error_code: String,
    pub error_message: String,
    pub occurred_at: DateTime<Utc>,
}

/// GL-posting domain events (discriminated union) for the module event bus.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum PostingEvent {
    AccountingPostPosted(AccountingPostPosted),
    AccountingPostFailed(AccountingPostFailed),
}

/// Map a posting source_type + posting_type to (journal_type, journal_source) enum labels.
fn map_source(source_type: &str, posting_type: &str) -> (&'static str, &'static str) {
    if posting_type == "reversal" {
        return ("reversing", "adjustment");
    }
    match source_type {
        "order" => ("sales", "order"),
        "payment" => ("cash_receipt", "payment"),
        "settlement" => ("general", "settlement"),
        "refund" => ("general", "adjustment"),
        "expense" => ("purchase", "adjustment"),
        "inventory" => ("general", "system"),
        _ => ("general", "manual"),
    }
}
