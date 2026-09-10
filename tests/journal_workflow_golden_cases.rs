//! Golden-case oracle for the manual-journal approval workflow
//! (docs/business-flows/gl-posting.md → manual-journal flow; BRD §3).
//!
//!   draft → submit → pending_approval → approve → posted (ledger written)
//!                                              → reject   (no ledger)
//!   posted → void (reversal posted, net zero, original retained)
//!
//! Requires DATABASE_URL (defaults to local dev Postgres on :5433).
//!
//! Tenancy: the module ships NONE (ADR-0029) — the request shapes keep the legacy company
//! twin but no table carries a tenant column, and an undecorated database has no fence.
//! Each test seeds its own chart (fresh UUID rows) and its own journal, the posting date
//! is derived per test (the fiscal-period guard reads periods by date overlap globally
//! undecorated), and the probes run under one lock because of that global period surface.

use chrono::Datelike;
use rust_decimal::Decimal;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use backbone_accounting::application::service::journal_workflow_service::JournalWorkflowService;

/// Serializes the database-touching probes in this file (see the header note): the
/// fiscal-period table is a global singleton surface on an undecorated database.
static DB_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

fn dec(s: &str) -> Decimal {
    Decimal::from_str_exact(s).unwrap()
}

async fn pool() -> PgPool {
    let url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgresql://postgres:postgres@localhost:5433/backbone_accounting".to_string()
    });
    PgPool::connect(&url).await.expect("connect DB")
}

/// Seed a minimal chart under a fresh company. Returns (company_id, code→account_id).
async fn seed_coa(pool: &PgPool) -> (Uuid, Uuid, Uuid) {
    let company = Uuid::new_v4();
    let bank = Uuid::new_v4();
    let rev = Uuid::new_v4();
    for (id, code, name, at, st, nb) in [
        (bank, "1100", "Bank", "asset", "bank", "debit"),
        (
            rev,
            "4000",
            "Revenue",
            "revenue",
            "operating_revenue",
            "credit",
        ),
    ] {
        sqlx::query(
            r#"INSERT INTO accounting.accounts
                (id, account_number, account_code, name, account_type, account_subtype,
                 normal_balance, is_detail, is_header, status)
               VALUES ($1,$2,$2,$3,$4::account_type,$5::account_subtype,$6::normal_balance,
                       TRUE, FALSE, 'active'::account_status)"#,
        )
        .bind(id)
        .bind(code)
        .bind(name)
        .bind(at)
        .bind(st)
        .bind(nb)
        .execute(pool)
        .await
        .unwrap();
    }
    (company, bank, rev)
}

/// Insert a draft manual journal + its lines (is_posted=false, no ledger rows).
async fn insert_draft_journal(
    pool: &PgPool,
    company: Uuid,
    bank: Uuid,
    rev: Uuid,
    debit: &str,
    credit: &str,
) -> Uuid {
    let j = Uuid::new_v4();
    let total = dec(debit);
    // Whole-month per-test window: see the header note on the globally-read
    // fiscal-period guard.
    let offset = (company.as_u128() % 240) as u32;
    let date = chrono::NaiveDate::from_ymd_opt(2026, 1, 15)
        .unwrap()
        .checked_add_months(chrono::Months::new(offset))
        .unwrap();
    sqlx::query(
        r#"INSERT INTO accounting.journals
            (id, journal_number, journal_type, transaction_date, posting_date,
             fiscal_year, fiscal_month, description, currency, total_debit, total_credit,
             line_count, source, source_type, status)
           VALUES ($1,$2,'general'::journal_type,$3,$3,$4,$5,'manual draft','IDR',$6,$6,2,
                   'manual'::journal_source,'manual','draft'::journal_status)"#,
    )
    .bind(j)
    .bind(format!("MJD-{j}"))
    .bind(date)
    .bind(date.year() as i32)
    .bind(date.month() as i32)
    .bind(total)
    .execute(pool)
    .await
    .unwrap();

    for (i, (id, code, name, d, c)) in [
        (bank, "1100", "Bank", debit, "0"),
        (rev, "4000", "Revenue", "0", credit),
    ]
    .iter()
    .enumerate()
    {
        sqlx::query(
            r#"INSERT INTO accounting.journal_lines
                (id, journal_id, line_number, account_id, account_number, account_name,
                 debit_amount, credit_amount, currency, is_posted)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,'IDR',FALSE)"#,
        )
        .bind(Uuid::new_v4())
        .bind(j)
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

async fn journal_status(pool: &PgPool, j: Uuid) -> String {
    let row =
        sqlx::query("SELECT status::text AS s, is_voided FROM accounting.journals WHERE id=$1")
            .bind(j)
            .fetch_one(pool)
            .await
            .unwrap();
    row.get::<String, _>("s")
}

async fn current_balance(pool: &PgPool, acct: Uuid) -> Decimal {
    sqlx::query_scalar("SELECT current_balance FROM accounting.accounts WHERE id=$1")
        .bind(acct)
        .fetch_one(pool)
        .await
        .unwrap()
}

// ── approve posts a draft journal to the ledger ──
#[tokio::test]
async fn approve_posts_draft_journal() {
    let _guard = DB_LOCK.lock().await;
    let pool = pool().await;
    let (company, bank, rev) = seed_coa(&pool).await;
    let j = insert_draft_journal(&pool, company, bank, rev, "100000", "100000").await;
    let svc = JournalWorkflowService::new(
        std::sync::Arc::new(
            backbone_accounting::infrastructure::persistence::SqlxPostingRepository::new(
                pool.clone(),
            ),
        ),
        std::sync::Arc::new(
            backbone_accounting::infrastructure::persistence::SqlxJournalWorkflowRepository::new(
                pool.clone(),
            ),
        ),
    );

    svc.submit(j, company).await.unwrap();
    let result = svc.approve(j, company, None).await.unwrap();

    // No idempotent reuse on first post; a real post_id was minted.
    assert_ne!(result.post_id, Uuid::nil());
    assert_eq!(result.posting_status, "posted");

    // Journal flipped to posted; ledger written; balances updated.
    assert_eq!(journal_status(&pool, j).await, "posted");
    let ledgers: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM accounting.ledgers WHERE journal_id=$1")
            .bind(j)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(ledgers, 2, "approve must write 2 ledger rows");
    assert_eq!(current_balance(&pool, bank).await, dec("100000"));
    assert_eq!(current_balance(&pool, rev).await, dec("100000"));

    // An AccountingPost was recorded (source_type=manual, source_id=journal).
    let (post_count, src): (i64, String) = sqlx::query_as(
        "SELECT COUNT(*), (SELECT source_type::text FROM accounting.accounting_posts WHERE journal_id=$1 LIMIT 1)",
    )
    .bind(j)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(post_count, 1);
    assert_eq!(src, "manual");
}

// ── reject writes no ledger rows ──
#[tokio::test]
async fn reject_keeps_ledger_empty() {
    let _guard = DB_LOCK.lock().await;
    let pool = pool().await;
    let (company, bank, rev) = seed_coa(&pool).await;
    let j = insert_draft_journal(&pool, company, bank, rev, "100000", "100000").await;
    let svc = JournalWorkflowService::new(
        std::sync::Arc::new(
            backbone_accounting::infrastructure::persistence::SqlxPostingRepository::new(
                pool.clone(),
            ),
        ),
        std::sync::Arc::new(
            backbone_accounting::infrastructure::persistence::SqlxJournalWorkflowRepository::new(
                pool.clone(),
            ),
        ),
    );

    svc.submit(j, company).await.unwrap();
    svc.reject(j, company, "does not look right".into(), None)
        .await
        .unwrap();

    assert_eq!(journal_status(&pool, j).await, "rejected");
    let ledgers: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM accounting.ledgers WHERE account_id = ANY($1)")
            .bind(&[bank, rev])
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(ledgers, 0, "reject must write zero ledger rows");
    assert_eq!(current_balance(&pool, bank).await, dec("0"));
}

// ── void posts a reversal; net effect zero, original retained ──
#[tokio::test]
async fn void_reverses_to_zero() {
    let _guard = DB_LOCK.lock().await;
    let pool = pool().await;
    let (company, bank, rev) = seed_coa(&pool).await;
    let j = insert_draft_journal(&pool, company, bank, rev, "100000", "100000").await;
    let svc = JournalWorkflowService::new(
        std::sync::Arc::new(
            backbone_accounting::infrastructure::persistence::SqlxPostingRepository::new(
                pool.clone(),
            ),
        ),
        std::sync::Arc::new(
            backbone_accounting::infrastructure::persistence::SqlxJournalWorkflowRepository::new(
                pool.clone(),
            ),
        ),
    );

    svc.submit(j, company).await.unwrap();
    svc.approve(j, company, None).await.unwrap();
    assert_eq!(current_balance(&pool, bank).await, dec("100000"));

    svc.void(j, company, None, "posted in error".into())
        .await
        .unwrap();

    // Balances net back to zero.
    assert_eq!(current_balance(&pool, bank).await, dec("0"));
    assert_eq!(current_balance(&pool, rev).await, dec("0"));

    // Original ledger retained (2 rows) + 2 reversal rows = 4.
    let ledgers: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM accounting.ledgers WHERE account_id = ANY($1)")
            .bind(&[bank, rev])
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        ledgers, 4,
        "void must add 2 reversal ledger rows, not edit the originals"
    );

    // Journal stamped voided.
    let row = sqlx::query(
        "SELECT status::text AS s, is_voided, void_reason FROM accounting.journals WHERE id=$1",
    )
    .bind(j)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row.get::<String, _>("s"), "voided");
    assert_eq!(row.get::<bool, _>("is_voided"), true);
    assert_eq!(
        row.get::<Option<String>, _>("void_reason").unwrap(),
        "posted in error"
    );
}

// ── submit is a state machine: rejects a non-draft journal ──
#[tokio::test]
async fn submit_rejects_non_draft() {
    let _guard = DB_LOCK.lock().await;
    let pool = pool().await;
    let (company, bank, rev) = seed_coa(&pool).await;
    let j = insert_draft_journal(&pool, company, bank, rev, "100000", "100000").await;
    let svc = JournalWorkflowService::new(
        std::sync::Arc::new(
            backbone_accounting::infrastructure::persistence::SqlxPostingRepository::new(
                pool.clone(),
            ),
        ),
        std::sync::Arc::new(
            backbone_accounting::infrastructure::persistence::SqlxJournalWorkflowRepository::new(
                pool.clone(),
            ),
        ),
    );

    svc.submit(j, company).await.unwrap(); // draft → pending_approval
                                           // A second submit must fail (now pending_approval, not draft).
    let err = svc.submit(j, company).await.unwrap_err();
    assert_eq!(err.code(), "invalid_journal_state");
}
