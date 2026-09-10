//! Golden-case oracle for the GL-posting service (docs/business-flows/golden-cases.md).
//!
//! Runs the exact GC-1..GC-11 numeric cases against a real Postgres and asserts DB state.
//! Requires DATABASE_URL (defaults to the local dev Postgres on :5433).
//!
//! Tenancy: the module ships NONE (ADR-0029) — the request shapes keep the legacy company
//! twin but no table carries a tenant column, and an undecorated database has no fence.
//! Isolation therefore rides identifiers and keys:
//!   - every test seeds its own chart of accounts (fresh UUID rows) and posts with fresh
//!     source ids, so row-level reads are disjoint by construction;
//!   - the posting date is derived per test from its company UUID (whole months, 240
//!     slots) because the fiscal-period guard reads periods by DATE OVERLAP globally on
//!     an undecorated database — a fixed date would cross-fire with any suite that seeds
//!     a closed/locked period over it;
//!   - the probes run under one lock and GC-10 sheds its closed period afterwards, so
//!     the shared global period table never carries a blocking row outside the one probe
//!     that means to have one;
//!   - GC-4a's cross-tenant refusal was pre-strip a module-side company comparison; the
//!     strip moves that block to the composing service's fence. What the module still
//!     pins undecorated is the same refusal for an absent/unposted reversal target —
//!     the exact path a fence-hidden row takes (see `gc4a_reversal_target_conflicts`).

use std::collections::HashMap;

use chrono::{Datelike, Months};
use rust_decimal::Decimal;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use std::sync::{Arc, Mutex};

use backbone_accounting::application::service::posting_service::{
    PostingError, PostingEvent, PostingEventSink, PostingLine, PostingRequest, PostingService,
};

/// Serializes the database-touching probes in this file (see the header note): the
/// fiscal-period table is a global singleton surface on an undecorated database.
static DB_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

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

fn dec(s: &str) -> Decimal {
    Decimal::from_str_exact(s).unwrap()
}

async fn pool() -> PgPool {
    let url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgresql://postgres:postgres@localhost:5433/backbone_accounting".to_string()
    });
    PgPool::connect(&url).await.expect("connect DB")
}

/// Per-test posting date: the 15th of a month chosen by the company UUID. Keeping every
/// test (and every suite that follows this pattern) in its own month is what makes the
/// global date-overlap period guard safe on an undecorated database.
fn posting_date(company: Uuid) -> chrono::NaiveDate {
    let offset = (company.as_u128() % 240) as u32;
    chrono::NaiveDate::from_ymd_opt(2026, 1, 15)
        .unwrap()
        .checked_add_months(Months::new(offset))
        .unwrap()
}

/// Seed the golden-case chart of accounts. Returns (company_id, code→id).
async fn seed_coa(pool: &PgPool) -> (Uuid, HashMap<&'static str, Uuid>) {
    let company_id = Uuid::new_v4();
    // (code, name, type, subtype, normal_balance, is_header, is_detail)
    let coa: &[(&str, &str, &str, &str, &str, bool, bool)] = &[
        (
            "1000",
            "Header Aset",
            "asset",
            "current_asset",
            "debit",
            true,
            false,
        ),
        ("1100", "Bank BCA", "asset", "bank", "debit", false, true),
        (
            "1200",
            "Piutang Usaha",
            "asset",
            "accounts_receivable",
            "debit",
            false,
            true,
        ),
        ("1210", "PPN Masukan", "asset", "tax", "debit", false, true),
        (
            "2100",
            "Utang Usaha",
            "liability",
            "accounts_payable",
            "credit",
            false,
            true,
        ),
        (
            "2200",
            "PPN Keluaran",
            "liability",
            "tax",
            "credit",
            false,
            true,
        ),
        (
            "2300",
            "Utang PPh 23",
            "liability",
            "tax",
            "credit",
            false,
            true,
        ),
        (
            "4000",
            "Pendapatan",
            "revenue",
            "operating_revenue",
            "credit",
            false,
            true,
        ),
        (
            "5000",
            "Beban Operasional",
            "expense",
            "operating_expense",
            "debit",
            false,
            true,
        ),
    ];
    let mut map = HashMap::new();
    for (code, name, at, st, nb, is_header, is_detail) in coa {
        let id = Uuid::new_v4();
        sqlx::query(
            r#"INSERT INTO accounting.accounts
                (id, account_number, account_code, name, account_type, account_subtype,
                 normal_balance, is_header, is_detail, status)
               VALUES ($1,$2,$2,$3,$4::account_type,$5::account_subtype,$6::normal_balance,
                       $7,$8,'active'::account_status)"#,
        )
        .bind(id)
        .bind(code)
        .bind(name)
        .bind(at)
        .bind(st)
        .bind(nb)
        .bind(is_header)
        .bind(is_detail)
        .execute(pool)
        .await
        .expect("seed account");
        map.insert(*code, id);
    }
    (company_id, map)
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

fn party_line(mut l: PostingLine, kind: &str, id: Uuid) -> PostingLine {
    l.party_type = Some(kind.to_string());
    l.party_id = Some(id);
    l
}

fn req(
    company: Uuid,
    source_type: &str,
    source_id: Uuid,
    lines: Vec<PostingLine>,
) -> PostingRequest {
    let mut r = PostingRequest::original(company, source_type, source_id, posting_date(company));
    r.lines = lines;
    r
}

/// Count ledger rows over the seeded chart (every row this test creates posts on one of
/// its own accounts — the per-test equivalent of the old per-company count).
async fn ledger_count(pool: &PgPool, a: &HashMap<&'static str, Uuid>) -> i64 {
    let ids: Vec<Uuid> = a.values().copied().collect();
    sqlx::query_scalar("SELECT COUNT(*) FROM accounting.ledgers WHERE account_id = ANY($1)")
        .bind(&ids)
        .fetch_one(pool)
        .await
        .unwrap()
}

/// Count journals with at least one line on the seeded chart.
async fn journal_count(pool: &PgPool, a: &HashMap<&'static str, Uuid>) -> i64 {
    let ids: Vec<Uuid> = a.values().copied().collect();
    sqlx::query_scalar(
        "SELECT COUNT(DISTINCT journal_id) FROM accounting.journal_lines WHERE account_id = ANY($1)",
    )
    .bind(&ids)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn acct_balance(pool: &PgPool, id: Uuid) -> Decimal {
    sqlx::query_scalar("SELECT current_balance FROM accounting.accounts WHERE id=$1")
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap()
}

/// A/R (or A/P) subledger balance for one party on one control account = Σ balance_change.
async fn party_balance(pool: &PgPool, account_id: Uuid, party_id: Uuid) -> Decimal {
    sqlx::query_scalar(
        "SELECT COALESCE(SUM(balance_change),0) FROM accounting.ledgers WHERE account_id=$1 AND party_id=$2",
    )
    .bind(account_id)
    .bind(party_id)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn ledger_change(pool: &PgPool, account_id: Uuid) -> Decimal {
    sqlx::query_scalar(
        "SELECT COALESCE(SUM(balance_change),0) FROM accounting.ledgers WHERE account_id=$1",
    )
    .bind(account_id)
    .fetch_one(pool)
    .await
    .unwrap()
}

/// Double-entry invariant across all ledger rows on the seeded chart.
async fn assert_globally_balanced(pool: &PgPool, a: &HashMap<&'static str, Uuid>) {
    let ids: Vec<Uuid> = a.values().copied().collect();
    let row = sqlx::query(
        "SELECT COALESCE(SUM(debit_amount),0) AS d, COALESCE(SUM(credit_amount),0) AS c \
         FROM accounting.ledgers WHERE account_id = ANY($1)",
    )
    .bind(&ids)
    .fetch_one(pool)
    .await
    .unwrap();
    let d: Decimal = row.get("d");
    let c: Decimal = row.get("c");
    assert_eq!(d, c, "global ledger not balanced");
}

// ── GC-1: sales invoice IDR 1,000,000 + PPN Output 11% ───────────────────────
#[tokio::test]
async fn gc1_sales_invoice() {
    let _guard = DB_LOCK.lock().await;
    let pool = pool().await;
    let (company, a) = seed_coa(&pool).await;
    let svc = PostingService::new(std::sync::Arc::new(
        backbone_accounting::infrastructure::persistence::SqlxPostingRepository::new(pool.clone()),
    ));
    let cust = Uuid::new_v4();

    let r = req(
        company,
        "order",
        Uuid::new_v4(),
        vec![
            party_line(line(a["1200"], "1110000.00", "0"), "customer", cust),
            line(a["4000"], "0", "1000000.00"),
            line(a["2200"], "0", "110000.00"),
        ],
    );
    let res = svc.post(r, None).await.expect("GC-1 should post");
    assert_eq!(res.posting_status, "posted");
    assert!(!res.idempotent_reuse);

    assert_eq!(journal_count(&pool, &a).await, 1);
    assert_eq!(ledger_count(&pool, &a).await, 3);
    assert_eq!(acct_balance(&pool, a["1200"]).await, dec("1110000.00"));
    assert_eq!(acct_balance(&pool, a["4000"]).await, dec("1000000.00"));
    assert_eq!(acct_balance(&pool, a["2200"]).await, dec("110000.00"));
    assert_eq!(
        party_balance(&pool, a["1200"], cust).await,
        dec("1110000.00")
    );
    assert_globally_balanced(&pool, &a).await;
}

// ── GC-2: payment settles the A/R back to zero ───────────────────────────────
#[tokio::test]
async fn gc2_payment_settles_ar() {
    let _guard = DB_LOCK.lock().await;
    let pool = pool().await;
    let (company, a) = seed_coa(&pool).await;
    let svc = PostingService::new(std::sync::Arc::new(
        backbone_accounting::infrastructure::persistence::SqlxPostingRepository::new(pool.clone()),
    ));
    let cust = Uuid::new_v4();

    svc.post(
        req(
            company,
            "order",
            Uuid::new_v4(),
            vec![
                party_line(line(a["1200"], "1110000.00", "0"), "customer", cust),
                line(a["4000"], "0", "1000000.00"),
                line(a["2200"], "0", "110000.00"),
            ],
        ),
        None,
    )
    .await
    .unwrap();

    svc.post(
        req(
            company,
            "payment",
            Uuid::new_v4(),
            vec![
                line(a["1100"], "1110000.00", "0"),
                party_line(line(a["1200"], "0", "1110000.00"), "customer", cust),
            ],
        ),
        None,
    )
    .await
    .expect("GC-2 should post");

    assert_eq!(party_balance(&pool, a["1200"], cust).await, dec("0.00"));
    assert_eq!(acct_balance(&pool, a["1100"]).await, dec("1110000.00"));
    assert_globally_balanced(&pool, &a).await;
}

// ── GC-3: purchase invoice + PPN Input + PPh 23 withholding ───────────────────
#[tokio::test]
async fn gc3_purchase_invoice() {
    let _guard = DB_LOCK.lock().await;
    let pool = pool().await;
    let (company, a) = seed_coa(&pool).await;
    let svc = PostingService::new(std::sync::Arc::new(
        backbone_accounting::infrastructure::persistence::SqlxPostingRepository::new(pool.clone()),
    ));
    let supp = Uuid::new_v4();

    svc.post(
        req(
            company,
            "expense",
            Uuid::new_v4(),
            vec![
                line(a["5000"], "500000.00", "0"),
                line(a["1210"], "55000.00", "0"),
                party_line(line(a["2100"], "0", "545000.00"), "supplier", supp),
                line(a["2300"], "0", "10000.00"),
            ],
        ),
        None,
    )
    .await
    .expect("GC-3 should post");

    assert_eq!(ledger_count(&pool, &a).await, 4);
    assert_eq!(acct_balance(&pool, a["5000"]).await, dec("500000.00"));
    assert_eq!(acct_balance(&pool, a["1210"]).await, dec("55000.00"));
    assert_eq!(acct_balance(&pool, a["2100"]).await, dec("545000.00"));
    assert_eq!(acct_balance(&pool, a["2300"]).await, dec("10000.00"));
    assert_eq!(
        party_balance(&pool, a["2100"], supp).await,
        dec("545000.00")
    );
    assert_globally_balanced(&pool, &a).await;
}

// ── GC-4: reversal of the sales invoice → net GL zero ─────────────────────────
#[tokio::test]
async fn gc4_reversal() {
    let _guard = DB_LOCK.lock().await;
    let pool = pool().await;
    let (company, a) = seed_coa(&pool).await;
    let svc = PostingService::new(std::sync::Arc::new(
        backbone_accounting::infrastructure::persistence::SqlxPostingRepository::new(pool.clone()),
    ));
    let cust = Uuid::new_v4();
    let source = Uuid::new_v4();

    let p1 = svc
        .post(
            req(
                company,
                "order",
                source,
                vec![
                    party_line(line(a["1200"], "1110000.00", "0"), "customer", cust),
                    line(a["4000"], "0", "1000000.00"),
                    line(a["2200"], "0", "110000.00"),
                ],
            ),
            None,
        )
        .await
        .unwrap();

    // Reversal: same source, posting_type=reversal, derives swapped lines from the original.
    let mut rev = PostingRequest::original(company, "order", source, posting_date(company));
    rev.posting_type = "reversal".to_string();
    rev.reverses_post_id = Some(p1.post_id);
    let p2 = svc
        .post(rev, None)
        .await
        .expect("GC-4 reversal should post");
    assert_ne!(p2.journal_id, p1.journal_id);

    // Net GL effect across {original, reversal} is zero for every account.
    assert_eq!(ledger_change(&pool, a["1200"]).await, dec("0.00"));
    assert_eq!(ledger_change(&pool, a["4000"]).await, dec("0.00"));
    assert_eq!(ledger_change(&pool, a["2200"]).await, dec("0.00"));
    assert_eq!(party_balance(&pool, a["1200"], cust).await, dec("0.00"));
    assert_globally_balanced(&pool, &a).await;

    // Reversal links.
    let is_reversed: bool =
        sqlx::query_scalar("SELECT is_reversed FROM accounting.journals WHERE id=$1")
            .bind(p1.journal_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(is_reversed, "original journal must be flagged reversed");
    let reversed_by: Option<Uuid> = sqlx::query_scalar(
        "SELECT reversed_by_post_id FROM accounting.accounting_posts WHERE id=$1",
    )
    .bind(p1.post_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(reversed_by, Some(p2.post_id));
}

// ── GC-4a: a hidden/absent reversal target refuses ──────────────────────────────
// Pre-strip this case blocked a SECOND company from reversing the first one's post via a
// module-side company comparison. The strip moved the cross-unit block to the composing
// service's fence (org-fenced reads hide the other unit's post). What the module itself
// still guarantees — decorated or not — is that a reversal whose target cannot be seen
// refuses typed with `conflict`, which is the exact path a fence-hidden row takes.
#[tokio::test]
async fn gc4a_reversal_target_conflicts() {
    let _guard = DB_LOCK.lock().await;
    let pool = pool().await;
    let (company, a) = seed_coa(&pool).await;
    let svc = PostingService::new(std::sync::Arc::new(
        backbone_accounting::infrastructure::persistence::SqlxPostingRepository::new(pool.clone()),
    ));

    let mut rev = PostingRequest::original(company, "order", Uuid::new_v4(), posting_date(company));
    rev.posting_type = "reversal".to_string();
    rev.reverses_post_id = Some(Uuid::new_v4()); // no such posted post (fence-hidden or absent)
    let res = svc.post(rev, None).await;

    match res {
        Ok(_) => panic!("reversal of an unseen target should be rejected"),
        Err(e) => assert_eq!(e.code(), "conflict", "expected conflict error"),
    }

    // No ledger rows written for the seeded chart.
    assert_eq!(
        ledger_count(&pool, &a).await,
        0,
        "no ledger rows should be written"
    );
}

// ── GC-8: idempotent retry → original returned, no double write ───────────────
#[tokio::test]
async fn gc8_idempotent_retry() {
    let _guard = DB_LOCK.lock().await;
    let pool = pool().await;
    let (company, a) = seed_coa(&pool).await;
    let svc = PostingService::new(std::sync::Arc::new(
        backbone_accounting::infrastructure::persistence::SqlxPostingRepository::new(pool.clone()),
    ));
    let cust = Uuid::new_v4();
    let source = Uuid::new_v4();

    let build = || {
        req(
            company,
            "order",
            source,
            vec![
                party_line(line(a["1200"], "1110000.00", "0"), "customer", cust),
                line(a["4000"], "0", "1000000.00"),
                line(a["2200"], "0", "110000.00"),
            ],
        )
    };

    let p1 = svc.post(build(), None).await.unwrap();
    let p2 = svc
        .post(build(), None)
        .await
        .expect("retry returns original");

    assert!(p2.idempotent_reuse);
    assert_eq!(p1.journal_id, p2.journal_id);
    assert_eq!(journal_count(&pool, &a).await, 1);
    assert_eq!(ledger_count(&pool, &a).await, 3);
    assert_eq!(acct_balance(&pool, a["1200"]).await, dec("1110000.00")); // charged once
}

// ── Rejections (GC-5,6,7,9,10,11): typed error + zero rows written ────────────
async fn assert_rejected_no_write(
    name: &str,
    res: Result<impl std::fmt::Debug, PostingError>,
    code: &str,
    pool: &PgPool,
    a: &HashMap<&'static str, Uuid>,
) {
    match res {
        Ok(ok) => panic!("{name}: expected rejection, got Ok({ok:?})"),
        Err(e) => assert_eq!(e.code(), code, "{name}: wrong error code"),
    }
    assert_eq!(
        journal_count(pool, a).await,
        0,
        "{name}: journal rows written"
    );
    assert_eq!(
        ledger_count(pool, a).await,
        0,
        "{name}: ledger rows written"
    );
}

#[tokio::test]
async fn gc5_unbalanced() {
    let _guard = DB_LOCK.lock().await;
    let pool = pool().await;
    let (company, a) = seed_coa(&pool).await;
    let svc = PostingService::new(std::sync::Arc::new(
        backbone_accounting::infrastructure::persistence::SqlxPostingRepository::new(pool.clone()),
    ));
    let res = svc
        .post(
            req(
                company,
                "manual",
                Uuid::new_v4(),
                vec![
                    line(a["5000"], "100.00", "0"),
                    line(a["1100"], "0", "90.00"),
                ],
            ),
            None,
        )
        .await;
    assert_rejected_no_write("GC-5", res, "unbalanced", &pool, &a).await;
}

#[tokio::test]
async fn gc6_missing_party() {
    let _guard = DB_LOCK.lock().await;
    let pool = pool().await;
    let (company, a) = seed_coa(&pool).await;
    let svc = PostingService::new(std::sync::Arc::new(
        backbone_accounting::infrastructure::persistence::SqlxPostingRepository::new(pool.clone()),
    ));
    let res = svc
        .post(
            req(
                company,
                "order",
                Uuid::new_v4(),
                vec![
                    line(a["1200"], "1110000.00", "0"), // A/R but NO party
                    line(a["4000"], "0", "1000000.00"),
                    line(a["2200"], "0", "110000.00"),
                ],
            ),
            None,
        )
        .await;
    assert_rejected_no_write("GC-6", res, "party_required", &pool, &a).await;
}

#[tokio::test]
async fn gc7_party_not_allowed() {
    let _guard = DB_LOCK.lock().await;
    let pool = pool().await;
    let (company, a) = seed_coa(&pool).await;
    let svc = PostingService::new(std::sync::Arc::new(
        backbone_accounting::infrastructure::persistence::SqlxPostingRepository::new(pool.clone()),
    ));
    let cust = Uuid::new_v4();
    let res = svc
        .post(
            req(
                company,
                "order",
                Uuid::new_v4(),
                vec![
                    party_line(line(a["1200"], "1110000.00", "0"), "customer", cust),
                    party_line(line(a["4000"], "0", "1000000.00"), "customer", cust), // party on revenue → not allowed
                    line(a["2200"], "0", "110000.00"),
                ],
            ),
            None,
        )
        .await;
    assert_rejected_no_write("GC-7", res, "party_not_allowed", &pool, &a).await;
}

#[tokio::test]
async fn gc9_single_line() {
    let _guard = DB_LOCK.lock().await;
    let pool = pool().await;
    let (company, a) = seed_coa(&pool).await;
    let svc = PostingService::new(std::sync::Arc::new(
        backbone_accounting::infrastructure::persistence::SqlxPostingRepository::new(pool.clone()),
    ));
    let res = svc
        .post(
            req(
                company,
                "manual",
                Uuid::new_v4(),
                vec![line(a["1100"], "100.00", "0")],
            ),
            None,
        )
        .await;
    assert_rejected_no_write("GC-9", res, "too_few_lines", &pool, &a).await;
}

#[tokio::test]
async fn gc10_closed_period() {
    let _guard = DB_LOCK.lock().await;
    let pool = pool().await;
    let (company, a) = seed_coa(&pool).await;
    let svc = PostingService::new(std::sync::Arc::new(
        backbone_accounting::infrastructure::persistence::SqlxPostingRepository::new(pool.clone()),
    ));

    // A closed fiscal period covering this test's posting date (whole month).
    let date = posting_date(company);
    let (y, m) = (date.year(), date.month());
    let start = chrono::NaiveDate::from_ymd_opt(y, m, 1).unwrap();
    let end = chrono::NaiveDate::from_ymd_opt(if m == 12 { y + 1 } else { y }, m % 12 + 1, 1)
        .unwrap()
        - chrono::Duration::days(1);
    sqlx::query(
        r#"INSERT INTO accounting.fiscal_periods
            (period_code, name, period_type, fiscal_year, start_date, end_date, status)
           VALUES ($1,$2,'monthly'::period_type,$3,$4,$5,'closed'::period_status)"#,
    )
    .bind(format!("{y:04}-{m:02}"))
    .bind(format!("{} closed-period probe", format!("{y:04}-{m:02}")))
    .bind(y)
    .bind(start)
    .bind(end)
    .execute(&pool)
    .await
    .expect("seed closed period");

    let cust = Uuid::new_v4();
    let res = svc
        .post(
            req(
                company,
                "order",
                Uuid::new_v4(),
                vec![
                    party_line(line(a["1200"], "1110000.00", "0"), "customer", cust),
                    line(a["4000"], "0", "1000000.00"),
                    line(a["2200"], "0", "110000.00"),
                ],
            ),
            None,
        )
        .await;
    assert_rejected_no_write("GC-10", res, "period_closed", &pool, &a).await;

    // Shed the closed period: the period guard reads the table globally on an
    // undecorated database, so a leftover row would refuse every later probe
    // posting inside this month.
    sqlx::query("DELETE FROM accounting.fiscal_periods WHERE period_code = $1")
        .bind(format!("{y:04}-{m:02}"))
        .execute(&pool)
        .await
        .unwrap();
}

// ── Event bus: AccountingPostPosted on success, AccountingPostFailed on reject ───
#[tokio::test]
async fn events_emitted() {
    let _guard = DB_LOCK.lock().await;
    let pool = pool().await;
    let (company, a) = seed_coa(&pool).await;
    let sink = RecordingSink::default();
    let svc = PostingService::with_sink(
        std::sync::Arc::new(
            backbone_accounting::infrastructure::persistence::SqlxPostingRepository::new(
                pool.clone(),
            ),
        ),
        Arc::new(sink.clone()),
    );
    let cust = Uuid::new_v4();

    // success → exactly one AccountingPostPosted
    svc.post(
        req(
            company,
            "order",
            Uuid::new_v4(),
            vec![
                party_line(line(a["1200"], "1110000.00", "0"), "customer", cust),
                line(a["4000"], "0", "1000000.00"),
                line(a["2200"], "0", "110000.00"),
            ],
        ),
        None,
    )
    .await
    .unwrap();

    // reject → one AccountingPostFailed
    let _ = svc
        .post(
            req(
                company,
                "manual",
                Uuid::new_v4(),
                vec![
                    line(a["5000"], "100.00", "0"),
                    line(a["1100"], "0", "90.00"),
                ],
            ),
            None,
        )
        .await;

    let events = sink.events.lock().unwrap();
    let posted = events
        .iter()
        .filter(|e| matches!(e, PostingEvent::AccountingPostPosted(_)))
        .count();
    let failed = events
        .iter()
        .filter(|e| matches!(e, PostingEvent::AccountingPostFailed(_)))
        .count();
    assert_eq!(posted, 1, "expected one AccountingPostPosted");
    assert_eq!(failed, 1, "expected one AccountingPostFailed");
}

#[tokio::test]
async fn gc11_header_account() {
    let _guard = DB_LOCK.lock().await;
    let pool = pool().await;
    let (company, a) = seed_coa(&pool).await;
    let svc = PostingService::new(std::sync::Arc::new(
        backbone_accounting::infrastructure::persistence::SqlxPostingRepository::new(pool.clone()),
    ));
    let res = svc
        .post(
            req(
                company,
                "manual",
                Uuid::new_v4(),
                vec![
                    line(a["1000"], "100.00", "0"), // header account → non-postable
                    line(a["4000"], "0", "100.00"),
                ],
            ),
            None,
        )
        .await;
    assert_rejected_no_write("GC-11", res, "non_postable_account", &pool, &a).await;
}
