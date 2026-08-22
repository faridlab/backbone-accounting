//! Golden cases for the budget-control consult on the GL-posting chokepoint.
//!
//! Proves the port contract end-to-end with a stub implementation:
//! - unwired port (None) is fail-open: over-budget posts succeed (no budget module composed);
//! - a block breach refuses the post with `budget_exceeded` (422), records a failed
//!   AccountingPost row, and publishes AccountingPostFailed — zero extra wiring;
//! - a warn breach logs and commits;
//! - a block breach dominates warn breaches on the same posting;
//! - a wired port that errors fails CLOSED (internal error, no journal row);
//! - the idempotent-reuse early return skips the consult (an already-posted entry
//!   is not re-judged).
//!
//! The pure mapping (empty/warned/blocked, block-wins) is unit-tested inside
//! `domain/services/posting_rules.rs`. The real budget-side implementation is
//! proved in the backbone-budget module's own suite.
//!
//! Requires DATABASE_URL (defaults to local dev Postgres on :5433). Each test
//! seeds its own company_id + chart, so tests are isolated and parallel-safe.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use chrono::NaiveDate;
use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

use backbone_accounting::application::service::journal_workflow_service::JournalWorkflowService;
use backbone_accounting::application::service::posting_service::{
    PostingEvent, PostingEventSink, PostingLine, PostingRequest, PostingService,
};
use backbone_accounting::domain::repositories::budget_control::{
    BudgetBreach, BudgetControlPort, BudgetEnforcement,
};
use backbone_accounting::infrastructure::persistence::{
    SqlxJournalWorkflowRepository, SqlxPostingRepository,
};

// ── fixtures ──────────────────────────────────────────────────────────────────

fn dec(s: &str) -> Decimal {
    Decimal::from_str_exact(s).unwrap()
}

async fn pool() -> PgPool {
    let url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgresql://postgres:postgres@localhost:5433/backbone_accounting".to_string()
    });
    PgPool::connect(&url).await.expect("connect DB")
}

/// Seed a minimal chart under a fresh company: bank (debit), revenue (credit),
/// operating expense (debit). Returns (company_id, bank, revenue, expense).
async fn seed_coa(pool: &PgPool) -> (Uuid, Uuid, Uuid, Uuid) {
    let company = Uuid::new_v4();
    let bank = Uuid::new_v4();
    let revenue = Uuid::new_v4();
    let expense = Uuid::new_v4();
    for (id, code, name, at, st, nb) in [
        (bank, "1100", "Bank", "asset", "bank", "debit"),
        (revenue, "4000", "Revenue", "revenue", "operating_revenue", "credit"),
        (expense, "5000", "Ops Expense", "expense", "operating_expense", "debit"),
    ] {
        sqlx::query(
            r#"INSERT INTO accounting.accounts
                (id, company_id, account_number, account_code, name, account_type, account_subtype,
                 normal_balance, is_detail, is_header, status)
               VALUES ($1,$2,$3,$3,$4,$5::account_type,$6::account_subtype,$7::normal_balance,
                       TRUE, FALSE, 'active'::account_status)"#,
        )
        .bind(id)
        .bind(company)
        .bind(code)
        .bind(name)
        .bind(at)
        .bind(st)
        .bind(nb)
        .execute(pool)
        .await
        .unwrap();
    }
    (company, bank, revenue, expense)
}

fn line(account_id: Uuid, debit: &str, credit: &str) -> PostingLine {
    PostingLine {
        account_id,
        debit: dec(debit),
        credit: dec(credit),
        party_type: None,
        party_id: None,
        cost_center_id: None,
        project_id: None,
        department_id: None,
        description: None,
    }
}

fn req(company: Uuid, source_id: Uuid, lines: Vec<PostingLine>) -> PostingRequest {
    let mut r = PostingRequest::original(
        company,
        "order",
        source_id,
        NaiveDate::from_ymd_opt(2026, 6, 15).unwrap(),
    );
    r.lines = lines;
    r
}

/// Test sink that records every published event.
#[derive(Default, Clone)]
struct RecordingSink {
    events: Arc<Mutex<Vec<PostingEvent>>>,
}
impl PostingEventSink for RecordingSink {
    fn publish(&self, event: PostingEvent) {
        self.events.lock().unwrap().push(event);
    }
}

fn failed_events(sink: &RecordingSink) -> usize {
    sink.events
        .lock()
        .unwrap()
        .iter()
        .filter(|e| matches!(e, PostingEvent::AccountingPostFailed(_)))
        .count()
}

/// What the stub port answers per consult.
enum StubAnswer {
    NoBreach,
    Breaches(Vec<BudgetBreach>),
    Fail,
}

/// Stub budget-control port: scripted answers + a consult counter.
struct StubBudgetControl {
    answer: StubAnswer,
    calls: AtomicUsize,
}

impl StubBudgetControl {
    fn new(answer: StubAnswer) -> Arc<Self> {
        Arc::new(Self {
            answer,
            calls: AtomicUsize::new(0),
        })
    }
    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl BudgetControlPort for StubBudgetControl {
    async fn evaluate_posting(
        &self,
        _company_id: Uuid,
        _posting_date: NaiveDate,
        _lines: &[PostingLine],
    ) -> anyhow::Result<Vec<BudgetBreach>> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        match &self.answer {
            StubAnswer::NoBreach => Ok(vec![]),
            StubAnswer::Breaches(b) => Ok(b.clone()),
            StubAnswer::Fail => Err(anyhow::anyhow!("stub budget store unreachable")),
        }
    }
}

fn breach(account_id: Uuid, enforcement: BudgetEnforcement) -> BudgetBreach {
    BudgetBreach {
        budget_id: Uuid::new_v4(),
        budget_line_id: Uuid::new_v4(),
        account_id,
        cost_center_id: None,
        fiscal_period_id: Uuid::new_v4(),
        planned_amount: dec("100"),
        achieved_amount: dec("40"),
        pending_amount: dec("70"),
        enforcement,
    }
}

async fn journal_rows(pool: &PgPool, company: Uuid) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM accounting.journals WHERE company_id=$1")
        .bind(company)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn failed_posts_with_code(pool: &PgPool, company: Uuid, code: &str) -> i64 {
    sqlx::query_scalar(
        "SELECT COUNT(*) FROM accounting.accounting_posts \
         WHERE company_id=$1 AND posting_status='failed' AND error_code=$2",
    )
    .bind(company)
    .bind(code)
    .fetch_one(pool)
    .await
    .unwrap()
}

fn posting_repo(
    pool: &PgPool,
) -> Arc<dyn backbone_accounting::domain::repositories::PostingRepository> {
    Arc::new(SqlxPostingRepository::new(pool.clone()))
}

// ── A1: unwired port is fail-open ────────────────────────────────────────────

#[tokio::test]
async fn unwired_port_posts_over_budget_amounts() {
    let pool = pool().await;
    let (company, bank, revenue, _expense) = seed_coa(&pool).await;
    let sink = RecordingSink::default();
    // No budget_control wired — the default build for hosts without a budget module.
    let svc = PostingService::with_sink(posting_repo(&pool), Arc::new(sink.clone()));

    let r = svc
        .post(
            req(
                company,
                Uuid::new_v4(),
                vec![
                    line(bank, "999999", "0"),
                    line(revenue, "0", "999999"),
                ],
            ),
            None,
        )
        .await;

    assert!(r.is_ok(), "unwired port must not restrict any posting");
    assert_eq!(journal_rows(&pool, company).await, 1);
    assert_eq!(failed_events(&sink), 0);
}

// ── A2: block breach refuses post() and audits the refusal ───────────────────

#[tokio::test]
async fn block_breach_refuses_post_and_records_failure() {
    let pool = pool().await;
    let (company, bank, revenue, expense) = seed_coa(&pool).await;
    let sink = RecordingSink::default();
    let stub = StubBudgetControl::new(StubAnswer::Breaches(vec![breach(
        expense,
        BudgetEnforcement::Block,
    )]));
    let svc = PostingService::with_sink(posting_repo(&pool), Arc::new(sink.clone()))
        .with_budget_control_if_set(Some(stub.clone()));

    let err = svc
        .post(
            req(
                company,
                Uuid::new_v4(),
                vec![
                    line(expense, "110", "0"),
                    line(bank, "0", "110"),
                ],
            ),
            None,
        )
        .await
        .expect_err("block breach must refuse");

    use backbone_accounting::domain::gl_posting::PostingError;
    match &err {
        PostingError::BudgetExceeded(b) => {
            assert_eq!(b.len(), 1);
            assert_eq!(b[0].account_id, expense);
            assert_eq!(b[0].enforcement, BudgetEnforcement::Block);
        }
        other => panic!("expected BudgetExceeded, got {other:?}"),
    }
    assert_eq!(err.code(), "budget_exceeded");
    assert_eq!(err.http_status(), 422);

    // The refusal rides the existing audit path: failed post row + event, no journal.
    assert_eq!(journal_rows(&pool, company).await, 0);
    assert_eq!(failed_posts_with_code(&pool, company, "budget_exceeded").await, 1);
    assert_eq!(failed_events(&sink), 1);
    assert_eq!(stub.calls(), 1);
}

// ── A2b: block breach refuses post_journal() (the second entry point) ────────

async fn insert_draft_journal(
    pool: &PgPool,
    company: Uuid,
    bank: Uuid,
    expense: Uuid,
    debit: &str,
) -> Uuid {
    let j = Uuid::new_v4();
    let total = dec(debit);
    sqlx::query(
        r#"INSERT INTO accounting.journals
            (id, company_id, journal_number, journal_type, transaction_date, posting_date,
             fiscal_year, fiscal_month, description, currency, total_debit, total_credit,
             line_count, source, source_type, status)
           VALUES ($1,$2,$3,'general'::journal_type,$4,$4,2026,6,'manual draft','IDR',$5,$5,2,
                   'manual'::journal_source,'manual','draft'::journal_status)"#,
    )
    .bind(j)
    .bind(company)
    .bind(format!("MJD-{j}"))
    .bind(NaiveDate::from_ymd_opt(2026, 6, 15).unwrap())
    .bind(total)
    .execute(pool)
    .await
    .unwrap();
    for (i, (id, code, name, d, c)) in
        [(expense, "5000", "Ops Expense", debit, "0"), (bank, "1100", "Bank", "0", debit)]
            .iter()
            .enumerate()
    {
        sqlx::query(
            r#"INSERT INTO accounting.journal_lines
                (id, journal_id, company_id, line_number, account_id, account_number, account_name,
                 debit_amount, credit_amount, currency, is_posted)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,'IDR',FALSE)"#,
        )
        .bind(Uuid::new_v4())
        .bind(j)
        .bind(company)
        .bind((i + 1) as i32)
        .bind(id)
        .bind(code)
        .bind(name)
        .bind(dec(d))
        .bind(dec(c))
        .execute(pool)
        .await
        .unwrap();
    }
    j
}

#[tokio::test]
async fn block_breach_refuses_manual_journal_post() {
    let pool = pool().await;
    let (company, bank, _revenue, expense) = seed_coa(&pool).await;
    let j = insert_draft_journal(&pool, company, bank, expense, "110").await;
    let stub = StubBudgetControl::new(StubAnswer::Breaches(vec![breach(
        expense,
        BudgetEnforcement::Block,
    )]));
    let svc = JournalWorkflowService::new(
        posting_repo(&pool),
        Arc::new(SqlxJournalWorkflowRepository::new(pool.clone())),
    )
    .with_budget_control_if_set(Some(stub));

    svc.submit(j, company).await.unwrap();
    let err = svc
        .approve(j, company, None)
        .await
        .expect_err("block breach must refuse the journal post");

    assert_eq!(err.code(), "posting_error");
    assert_eq!(
        failed_posts_with_code(&pool, company, "budget_exceeded").await,
        1,
        "the journal refusal rides the same failed-post audit path"
    );
    let ledgers: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM accounting.ledgers WHERE journal_id=$1")
            .bind(j)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(ledgers, 0, "no ledger rows may be written for a refusal");
}

// ── A3: warn breach commits ──────────────────────────────────────────────────

#[tokio::test]
async fn warn_breach_posts_and_commits() {
    let pool = pool().await;
    let (company, bank, _revenue, expense) = seed_coa(&pool).await;
    let sink = RecordingSink::default();
    let stub = StubBudgetControl::new(StubAnswer::Breaches(vec![breach(
        expense,
        BudgetEnforcement::Warn,
    )]));
    let svc = PostingService::with_sink(posting_repo(&pool), Arc::new(sink.clone()))
        .with_budget_control_if_set(Some(stub));

    let r = svc
        .post(
            req(
                company,
                Uuid::new_v4(),
                vec![line(expense, "110", "0"), line(bank, "0", "110")],
            ),
            None,
        )
        .await
        .expect("warn breach must not block the commit");

    assert!(!r.idempotent_reuse);
    assert_eq!(journal_rows(&pool, company).await, 1);
    assert_eq!(
        failed_posts_with_code(&pool, company, "budget_exceeded").await,
        0
    );
    assert_eq!(failed_events(&sink), 0, "a warned post is not a failure");
}

// ── A4: block dominates warn on the same posting ─────────────────────────────

#[tokio::test]
async fn mixed_breach_blocks_and_carries_only_blocking_positions() {
    let pool = pool().await;
    let (company, bank, revenue, expense) = seed_coa(&pool).await;
    let stub = StubBudgetControl::new(StubAnswer::Breaches(vec![
        breach(expense, BudgetEnforcement::Warn),
        breach(revenue, BudgetEnforcement::Block),
    ]));
    let svc = PostingService::with_sink(posting_repo(&pool), Arc::new(RecordingSink::default()))
        .with_budget_control_if_set(Some(stub));

    let err = svc
        .post(
            req(
                company,
                Uuid::new_v4(),
                vec![line(expense, "110", "0"), line(bank, "0", "110")],
            ),
            None,
        )
        .await
        .expect_err("a block breach anywhere refuses the posting");

    use backbone_accounting::domain::gl_posting::PostingError;
    match &err {
        PostingError::BudgetExceeded(b) => {
            assert_eq!(b.len(), 1, "only the blocking position rides the refusal");
            assert_eq!(b[0].account_id, revenue);
        }
        other => panic!("expected BudgetExceeded, got {other:?}"),
    }
}

// ── A5: a wired port that errors fails closed ────────────────────────────────

#[tokio::test]
async fn broken_port_fails_closed_with_internal_error() {
    let pool = pool().await;
    let (company, bank, _revenue, expense) = seed_coa(&pool).await;
    let stub = StubBudgetControl::new(StubAnswer::Fail);
    let svc = PostingService::with_sink(posting_repo(&pool), Arc::new(RecordingSink::default()))
        .with_budget_control_if_set(Some(stub));

    let err = svc
        .post(
            req(
                company,
                Uuid::new_v4(),
                vec![line(expense, "10", "0"), line(bank, "0", "10")],
            ),
            None,
        )
        .await
        .expect_err("a broken budget module must not disable enforcement");

    assert_eq!(err.code(), "internal_error");
    assert_eq!(err.http_status(), 500);
    assert_eq!(journal_rows(&pool, company).await, 0, "nothing commits");
}

// ── A7: idempotent reuse skips the consult ───────────────────────────────────

#[tokio::test]
async fn idempotent_reuse_skips_the_budget_consult() {
    let pool = pool().await;
    let (company, bank, revenue, _expense) = seed_coa(&pool).await;
    let stub = StubBudgetControl::new(StubAnswer::NoBreach);
    let svc = PostingService::with_sink(posting_repo(&pool), Arc::new(RecordingSink::default()))
        .with_budget_control_if_set(Some(stub.clone()));

    let source_id = Uuid::new_v4();
    let lines = vec![line(bank, "50", "0"), line(revenue, "0", "50")];
    let first = svc.post(req(company, source_id, lines.clone()), None).await.unwrap();
    assert!(!first.idempotent_reuse);

    let second = svc.post(req(company, source_id, lines), None).await.unwrap();
    assert!(
        second.idempotent_reuse,
        "the same source identity returns the existing post"
    );
    assert_eq!(
        stub.calls(),
        1,
        "an already-posted entry is not re-judged by the budget control"
    );
}
