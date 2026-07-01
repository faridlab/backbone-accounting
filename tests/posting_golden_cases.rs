//! Golden-case oracle for the GL-posting service (docs/business-flows/golden-cases.md).
//!
//! Runs the exact GC-1..GC-11 numeric cases against a real Postgres and asserts DB state.
//! Requires DATABASE_URL (defaults to the local dev Postgres on :5433). Each test seeds its
//! own company_id + chart of accounts, so tests are isolated and parallel-safe.

use std::collections::HashMap;

use rust_decimal::Decimal;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use std::sync::{Arc, Mutex};

use backbone_accounting::application::service::posting_service::{
    PostingEvent, PostingEventSink, PostingLine, PostingRequest, PostingService,
};

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

/// Seed the golden-case chart of accounts under a fresh company. Returns (company_id, code→id).
async fn seed_coa(pool: &PgPool) -> (Uuid, HashMap<&'static str, Uuid>) {
    let company_id = Uuid::new_v4();
    // (code, name, type, subtype, normal_balance, is_header, is_detail)
    let coa: &[(&str, &str, &str, &str, &str, bool, bool)] = &[
        ("1000", "Header Aset", "asset", "current_asset", "debit", true, false),
        ("1100", "Bank BCA", "asset", "bank", "debit", false, true),
        ("1200", "Piutang Usaha", "asset", "accounts_receivable", "debit", false, true),
        ("1210", "PPN Masukan", "asset", "tax", "debit", false, true),
        ("2100", "Utang Usaha", "liability", "accounts_payable", "credit", false, true),
        ("2200", "PPN Keluaran", "liability", "tax", "credit", false, true),
        ("2300", "Utang PPh 23", "liability", "tax", "credit", false, true),
        ("4000", "Pendapatan", "revenue", "operating_revenue", "credit", false, true),
        ("5000", "Beban Operasional", "expense", "operating_expense", "debit", false, true),
    ];
    let mut map = HashMap::new();
    for (code, name, at, st, nb, is_header, is_detail) in coa {
        let id = Uuid::new_v4();
        sqlx::query(
            r#"INSERT INTO accounting.accounts
                (id, company_id, account_number, account_code, name, account_type, account_subtype,
                 normal_balance, is_header, is_detail, status)
               VALUES ($1,$2,$3,$4,$5,$6::account_type,$7::account_subtype,$8::normal_balance,
                       $9,$10,'active'::account_status)"#,
        )
        .bind(id)
        .bind(company_id)
        .bind(code)
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

fn req(company: Uuid, source_type: &str, source_id: Uuid, lines: Vec<PostingLine>) -> PostingRequest {
    let mut r = PostingRequest::original(company, source_type, source_id, chrono::NaiveDate::from_ymd_opt(2026, 6, 15).unwrap());
    r.lines = lines;
    r
}

async fn ledger_count(pool: &PgPool, company: Uuid) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM accounting.ledgers WHERE company_id=$1")
        .bind(company)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn journal_count(pool: &PgPool, company: Uuid) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM accounting.journals WHERE company_id=$1")
        .bind(company)
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

async fn ledger_change(pool: &PgPool, company: Uuid, account_id: Uuid) -> Decimal {
    sqlx::query_scalar(
        "SELECT COALESCE(SUM(balance_change),0) FROM accounting.ledgers WHERE company_id=$1 AND account_id=$2",
    )
    .bind(company)
    .bind(account_id)
    .fetch_one(pool)
    .await
    .unwrap()
}

/// Global double-entry invariant across all ledger rows for a company.
async fn assert_globally_balanced(pool: &PgPool, company: Uuid) {
    let row = sqlx::query(
        "SELECT COALESCE(SUM(debit_amount),0) AS d, COALESCE(SUM(credit_amount),0) AS c FROM accounting.ledgers WHERE company_id=$1",
    )
    .bind(company)
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
    let pool = pool().await;
    let (company, a) = seed_coa(&pool).await;
    let svc = PostingService::new(pool.clone());
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

    assert_eq!(journal_count(&pool, company).await, 1);
    assert_eq!(ledger_count(&pool, company).await, 3);
    assert_eq!(acct_balance(&pool, a["1200"]).await, dec("1110000.00"));
    assert_eq!(acct_balance(&pool, a["4000"]).await, dec("1000000.00"));
    assert_eq!(acct_balance(&pool, a["2200"]).await, dec("110000.00"));
    assert_eq!(party_balance(&pool, a["1200"], cust).await, dec("1110000.00"));
    assert_globally_balanced(&pool, company).await;
}

// ── GC-2: payment settles the A/R back to zero ───────────────────────────────
#[tokio::test]
async fn gc2_payment_settles_ar() {
    let pool = pool().await;
    let (company, a) = seed_coa(&pool).await;
    let svc = PostingService::new(pool.clone());
    let cust = Uuid::new_v4();

    svc.post(
        req(company, "order", Uuid::new_v4(), vec![
            party_line(line(a["1200"], "1110000.00", "0"), "customer", cust),
            line(a["4000"], "0", "1000000.00"),
            line(a["2200"], "0", "110000.00"),
        ]),
        None,
    ).await.unwrap();

    svc.post(
        req(company, "payment", Uuid::new_v4(), vec![
            line(a["1100"], "1110000.00", "0"),
            party_line(line(a["1200"], "0", "1110000.00"), "customer", cust),
        ]),
        None,
    ).await.expect("GC-2 should post");

    assert_eq!(party_balance(&pool, a["1200"], cust).await, dec("0.00"));
    assert_eq!(acct_balance(&pool, a["1100"]).await, dec("1110000.00"));
    assert_globally_balanced(&pool, company).await;
}

// ── GC-3: purchase invoice + PPN Input + PPh 23 withholding ───────────────────
#[tokio::test]
async fn gc3_purchase_invoice() {
    let pool = pool().await;
    let (company, a) = seed_coa(&pool).await;
    let svc = PostingService::new(pool.clone());
    let supp = Uuid::new_v4();

    svc.post(
        req(company, "expense", Uuid::new_v4(), vec![
            line(a["5000"], "500000.00", "0"),
            line(a["1210"], "55000.00", "0"),
            party_line(line(a["2100"], "0", "545000.00"), "supplier", supp),
            line(a["2300"], "0", "10000.00"),
        ]),
        None,
    ).await.expect("GC-3 should post");

    assert_eq!(ledger_count(&pool, company).await, 4);
    assert_eq!(acct_balance(&pool, a["5000"]).await, dec("500000.00"));
    assert_eq!(acct_balance(&pool, a["1210"]).await, dec("55000.00"));
    assert_eq!(acct_balance(&pool, a["2100"]).await, dec("545000.00"));
    assert_eq!(acct_balance(&pool, a["2300"]).await, dec("10000.00"));
    assert_eq!(party_balance(&pool, a["2100"], supp).await, dec("545000.00"));
    assert_globally_balanced(&pool, company).await;
}

// ── GC-4: reversal of the sales invoice → net GL zero ─────────────────────────
#[tokio::test]
async fn gc4_reversal() {
    let pool = pool().await;
    let (company, a) = seed_coa(&pool).await;
    let svc = PostingService::new(pool.clone());
    let cust = Uuid::new_v4();
    let source = Uuid::new_v4();

    let p1 = svc.post(
        req(company, "order", source, vec![
            party_line(line(a["1200"], "1110000.00", "0"), "customer", cust),
            line(a["4000"], "0", "1000000.00"),
            line(a["2200"], "0", "110000.00"),
        ]),
        None,
    ).await.unwrap();

    // Reversal: same source, posting_type=reversal, derives swapped lines from the original.
    let mut rev = PostingRequest::original(company, "order", source, chrono::NaiveDate::from_ymd_opt(2026, 6, 15).unwrap());
    rev.posting_type = "reversal".to_string();
    rev.reverses_post_id = Some(p1.post_id);
    let p2 = svc.post(rev, None).await.expect("GC-4 reversal should post");
    assert_ne!(p2.journal_id, p1.journal_id);

    // Net GL effect across {original, reversal} is zero for every account.
    assert_eq!(ledger_change(&pool, company, a["1200"]).await, dec("0.00"));
    assert_eq!(ledger_change(&pool, company, a["4000"]).await, dec("0.00"));
    assert_eq!(ledger_change(&pool, company, a["2200"]).await, dec("0.00"));
    assert_eq!(party_balance(&pool, a["1200"], cust).await, dec("0.00"));
    assert_globally_balanced(&pool, company).await;

    // Reversal links.
    let is_reversed: bool = sqlx::query_scalar("SELECT is_reversed FROM accounting.journals WHERE id=$1")
        .bind(p1.journal_id).fetch_one(&pool).await.unwrap();
    assert!(is_reversed, "original journal must be flagged reversed");
    let reversed_by: Option<Uuid> = sqlx::query_scalar("SELECT reversed_by_post_id FROM accounting.accounting_posts WHERE id=$1")
        .bind(p1.post_id).fetch_one(&pool).await.unwrap();
    assert_eq!(reversed_by, Some(p2.post_id));
}

// ── GC-8: idempotent retry → original returned, no double write ───────────────
#[tokio::test]
async fn gc8_idempotent_retry() {
    let pool = pool().await;
    let (company, a) = seed_coa(&pool).await;
    let svc = PostingService::new(pool.clone());
    let cust = Uuid::new_v4();
    let source = Uuid::new_v4();

    let build = || req(company, "order", source, vec![
        party_line(line(a["1200"], "1110000.00", "0"), "customer", cust),
        line(a["4000"], "0", "1000000.00"),
        line(a["2200"], "0", "110000.00"),
    ]);

    let p1 = svc.post(build(), None).await.unwrap();
    let p2 = svc.post(build(), None).await.expect("retry returns original");

    assert!(p2.idempotent_reuse);
    assert_eq!(p1.journal_id, p2.journal_id);
    assert_eq!(journal_count(&pool, company).await, 1);
    assert_eq!(ledger_count(&pool, company).await, 3);
    assert_eq!(acct_balance(&pool, a["1200"]).await, dec("1110000.00")); // charged once
}

// ── Rejections (GC-5,6,7,9,10,11): typed error + zero rows written ────────────
async fn assert_rejected_no_write(name: &str, res: Result<impl std::fmt::Debug, backbone_accounting::application::service::posting_service::PostingError>, code: &str, pool: &PgPool, company: Uuid) {
    match res {
        Ok(ok) => panic!("{name}: expected rejection, got Ok({ok:?})"),
        Err(e) => assert_eq!(e.code(), code, "{name}: wrong error code"),
    }
    assert_eq!(journal_count(pool, company).await, 0, "{name}: journal rows written");
    assert_eq!(ledger_count(pool, company).await, 0, "{name}: ledger rows written");
}

#[tokio::test]
async fn gc5_unbalanced() {
    let pool = pool().await;
    let (company, a) = seed_coa(&pool).await;
    let svc = PostingService::new(pool.clone());
    let res = svc.post(req(company, "manual", Uuid::new_v4(), vec![
        line(a["5000"], "100.00", "0"),
        line(a["1100"], "0", "90.00"),
    ]), None).await;
    assert_rejected_no_write("GC-5", res, "unbalanced", &pool, company).await;
}

#[tokio::test]
async fn gc6_missing_party() {
    let pool = pool().await;
    let (company, a) = seed_coa(&pool).await;
    let svc = PostingService::new(pool.clone());
    let res = svc.post(req(company, "order", Uuid::new_v4(), vec![
        line(a["1200"], "1110000.00", "0"), // A/R but NO party
        line(a["4000"], "0", "1000000.00"),
        line(a["2200"], "0", "110000.00"),
    ]), None).await;
    assert_rejected_no_write("GC-6", res, "party_required", &pool, company).await;
}

#[tokio::test]
async fn gc7_party_not_allowed() {
    let pool = pool().await;
    let (company, a) = seed_coa(&pool).await;
    let svc = PostingService::new(pool.clone());
    let cust = Uuid::new_v4();
    let res = svc.post(req(company, "order", Uuid::new_v4(), vec![
        party_line(line(a["1200"], "1110000.00", "0"), "customer", cust),
        party_line(line(a["4000"], "0", "1000000.00"), "customer", cust), // party on revenue → not allowed
        line(a["2200"], "0", "110000.00"),
    ]), None).await;
    assert_rejected_no_write("GC-7", res, "party_not_allowed", &pool, company).await;
}

#[tokio::test]
async fn gc9_single_line() {
    let pool = pool().await;
    let (company, a) = seed_coa(&pool).await;
    let svc = PostingService::new(pool.clone());
    let res = svc.post(req(company, "manual", Uuid::new_v4(), vec![
        line(a["1100"], "100.00", "0"),
    ]), None).await;
    assert_rejected_no_write("GC-9", res, "too_few_lines", &pool, company).await;
}

#[tokio::test]
async fn gc10_closed_period() {
    let pool = pool().await;
    let (company, a) = seed_coa(&pool).await;
    let svc = PostingService::new(pool.clone());
    // A closed fiscal period covering the posting date.
    sqlx::query(
        r#"INSERT INTO accounting.fiscal_periods
            (id, company_id, period_code, name, period_type, fiscal_year, start_date, end_date, status)
           VALUES ($1,$2,'2026-06','June 2026','monthly'::period_type,2026,'2026-06-01','2026-06-30','closed'::period_status)"#,
    )
    .bind(Uuid::new_v4()).bind(company)
    .execute(&pool).await.expect("seed closed period");

    let cust = Uuid::new_v4();
    let res = svc.post(req(company, "order", Uuid::new_v4(), vec![
        party_line(line(a["1200"], "1110000.00", "0"), "customer", cust),
        line(a["4000"], "0", "1000000.00"),
        line(a["2200"], "0", "110000.00"),
    ]), None).await;
    assert_rejected_no_write("GC-10", res, "period_closed", &pool, company).await;
}

// ── Event bus: AccountingPostPosted on success, AccountingPostFailed on reject ───
#[tokio::test]
async fn events_emitted() {
    let pool = pool().await;
    let (company, a) = seed_coa(&pool).await;
    let sink = RecordingSink::default();
    let svc = PostingService::with_sink(pool.clone(), Arc::new(sink.clone()));
    let cust = Uuid::new_v4();

    // success → exactly one AccountingPostPosted
    svc.post(req(company, "order", Uuid::new_v4(), vec![
        party_line(line(a["1200"], "1110000.00", "0"), "customer", cust),
        line(a["4000"], "0", "1000000.00"),
        line(a["2200"], "0", "110000.00"),
    ]), None).await.unwrap();

    // reject → one AccountingPostFailed
    let _ = svc.post(req(company, "manual", Uuid::new_v4(), vec![
        line(a["5000"], "100.00", "0"),
        line(a["1100"], "0", "90.00"),
    ]), None).await;

    let events = sink.events.lock().unwrap();
    let posted = events.iter().filter(|e| matches!(e, PostingEvent::AccountingPostPosted(_))).count();
    let failed = events.iter().filter(|e| matches!(e, PostingEvent::AccountingPostFailed(_))).count();
    assert_eq!(posted, 1, "expected one AccountingPostPosted");
    assert_eq!(failed, 1, "expected one AccountingPostFailed");
}

#[tokio::test]
async fn gc11_header_account() {
    let pool = pool().await;
    let (company, a) = seed_coa(&pool).await;
    let svc = PostingService::new(pool.clone());
    let res = svc.post(req(company, "manual", Uuid::new_v4(), vec![
        line(a["1000"], "100.00", "0"), // header account → non-postable
        line(a["4000"], "0", "100.00"),
    ]), None).await;
    assert_rejected_no_write("GC-11", res, "non_postable_account", &pool, company).await;
}
