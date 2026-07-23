//! Council integrity probes (regression tests for the two write-path holes the council found).
//!
//! Probe 1 — concurrent double-post cannot corrupt the ledger (DB-enforced idempotency).
//! Probe 2 — the posted-GL CRUD write verbs are NOT mounted by the guarded composition.
//! Requires DATABASE_URL (defaults to local dev Postgres on :5433).

use std::collections::HashMap;

use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

use backbone_accounting::application::service::posting_service::{
    PostingLine, PostingRequest, PostingService,
};
use backbone_accounting::presentation::http::create_guarded_accounting_routes;
use backbone_accounting::AccountingModule;

fn dec(s: &str) -> Decimal {
    Decimal::from_str_exact(s).unwrap()
}
async fn pool() -> PgPool {
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://postgres:postgres@localhost:5433/backbone_accounting".to_string());
    PgPool::connect(&url).await.unwrap()
}
async fn seed_coa(pool: &PgPool) -> (Uuid, HashMap<&'static str, Uuid>) {
    let company = Uuid::new_v4();
    let mut m = HashMap::new();
    for (code, name, at, st, nb) in [
        ("1200", "AR", "asset", "accounts_receivable", "debit"),
        ("4000", "Rev", "revenue", "operating_revenue", "credit"),
        ("2200", "PPN", "liability", "tax", "credit"),
    ] {
        let id = Uuid::new_v4();
        sqlx::query(
            r#"INSERT INTO accounting.accounts (id, company_id, account_number, account_code, name, account_type,
                account_subtype, normal_balance, is_detail, is_header, status)
               VALUES ($1,$2,$3,$3,$4,$5::account_type,$6::account_subtype,$7::normal_balance,TRUE,FALSE,'active'::account_status)"#,
        )
        .bind(id).bind(company).bind(code).bind(name).bind(at).bind(st).bind(nb)
        .execute(pool).await.unwrap();
        m.insert(code, id);
    }
    (company, m)
}

// ── Probe 1: concurrent double-post is rejected by the DB; ledger not corrupted ──
#[tokio::test]
async fn concurrent_double_post_does_not_double_count() {
    let pool = pool().await;
    let (company, a) = seed_coa(&pool).await;
    let svc = PostingService::new(std::sync::Arc::new(backbone_accounting::infrastructure::persistence::SqlxPostingRepository::new(pool.clone())));
    let cust = Uuid::new_v4();
    let source = Uuid::new_v4(); // SAME source for both concurrent posts

    let build = || {
        let mut r = PostingRequest::original(company, "order", source, chrono::NaiveDate::from_ymd_opt(2026, 6, 15).unwrap());
        r.lines = vec![
            PostingLine { account_id: a["1200"], debit: dec("1110000.00"), credit: Decimal::ZERO, party_type: Some("customer".into()), party_id: Some(cust), cost_center_id: None, project_id: None, department_id: None, description: None },
            PostingLine { account_id: a["4000"], debit: Decimal::ZERO, credit: dec("1000000.00"), party_type: None, party_id: None, cost_center_id: None, project_id: None, department_id: None, description: None },
            PostingLine { account_id: a["2200"], debit: Decimal::ZERO, credit: dec("110000.00"), party_type: None, party_id: None, cost_center_id: None, project_id: None, department_id: None, description: None },
        ];
        r
    };

    let s1 = svc.clone();
    let s2 = svc.clone();
    let (r1, r2) = tokio::join!(s1.post(build(), None), s2.post(build(), None));
    // Both calls succeed (one posts, the other returns the winner idempotently).
    r1.unwrap();
    r2.unwrap();

    // Exactly one posted entry and one journal's worth of ledger rows — no double-count.
    let posted: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM accounting.accounting_posts WHERE company_id=$1 AND posting_status='posted'")
        .bind(company).fetch_one(&pool).await.unwrap();
    let ledgers: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM accounting.ledgers WHERE company_id=$1")
        .bind(company).fetch_one(&pool).await.unwrap();
    let ar_balance: Decimal = sqlx::query_scalar("SELECT current_balance FROM accounting.accounts WHERE id=$1")
        .bind(a["1200"]).fetch_one(&pool).await.unwrap();

    assert_eq!(posted, 1, "concurrent posts must yield exactly ONE posted entry");
    assert_eq!(ledgers, 3, "ledger must have 3 rows, not 6 (no double-count)");
    assert_eq!(ar_balance, dec("1110000.00"), "A/R charged once, not twice");
}

// ── Probe 2: DISTINCT sources posting concurrently to the SAME account cannot corrupt the
//            running-balance chain (per-account FOR UPDATE fence). This is the case ADR-0010's
//            probe did NOT cover — it only tested same-source dedup.
#[tokio::test]
async fn concurrent_distinct_sources_one_account_keeps_balance_chain() {
    let pool = pool().await;
    let company = Uuid::new_v4();
    // Two accounts: Cash (debit-normal) and Revenue (credit-normal). 8 posts will hit BOTH.
    let cash = Uuid::new_v4();
    let rev = Uuid::new_v4();
    for (id, code, name, at, st, nb) in [
        (cash, "1100", "Cash", "asset", "cash", "debit"),
        (rev, "4000", "Revenue", "revenue", "operating_revenue", "credit"),
    ] {
        sqlx::query(
            r#"INSERT INTO accounting.accounts (id, company_id, account_number, account_code, name, account_type,
                account_subtype, normal_balance, is_detail, is_header, status)
               VALUES ($1,$2,$3,$3,$4,$5::account_type,$6::account_subtype,$7::normal_balance,TRUE,FALSE,'active'::account_status)"#,
        )
        .bind(id).bind(company).bind(code).bind(name).bind(at).bind(st).bind(nb)
        .execute(&pool).await.unwrap();
    }
    let svc = PostingService::new(std::sync::Arc::new(backbone_accounting::infrastructure::persistence::SqlxPostingRepository::new(pool.clone())));
    let amount = dec("100000.00");
    let n = 8;

    // N posts, each with a DISTINCT source_id + idempotency_key, all hitting Cash + Revenue.
    let mut handles = Vec::new();
    for i in 0..n {
        let svc = svc.clone();
        let source = Uuid::new_v4();
        handles.push(tokio::spawn(async move {
            let mut r = PostingRequest::original(
                company,
                "order",
                source,
                chrono::NaiveDate::from_ymd_opt(2026, 6, 15).unwrap(),
            )
            .with_idempotency_key(format!("probe2-{i}"));
            r.lines = vec![
                PostingLine { account_id: cash, debit: amount, credit: Decimal::ZERO, party_type: None, party_id: None, cost_center_id: None, project_id: None, department_id: None, description: None },
                PostingLine { account_id: rev, debit: Decimal::ZERO, credit: amount, party_type: None, party_id: None, cost_center_id: None, project_id: None, department_id: None, description: None },
            ];
            svc.post(r, None).await.unwrap()
        }));
    }
    for h in handles {
        h.await.unwrap();
    }

    let expected = dec("800000.00"); // 8 * 100000

    // (a) No duplicate sequence_number per account — the running-balance chain is unbroken.
    for acct in [cash, rev] {
        let dups: i64 = sqlx::query_scalar(
            r#"SELECT COUNT(*) FROM (
                 SELECT sequence_number FROM accounting.ledgers
                 WHERE company_id=$1 AND account_id=$2
                 GROUP BY sequence_number HAVING COUNT(*) > 1
               ) t"#,
        )
        .bind(company).bind(acct).fetch_one(&pool).await.unwrap();
        assert_eq!(dups, 0, "account {acct} has duplicate sequence_number — balance chain corrupted");
    }

    // (b) balance_after, ordered by sequence_number, is strictly monotonic and continuous
    //     (each row's balance_before == prior row's balance_after, first == 0).
    for acct in [cash, rev] {
        let rows: Vec<(Decimal, Decimal, Decimal)> = sqlx::query_as(
            r#"SELECT balance_before, balance_change, balance_after FROM accounting.ledgers
               WHERE company_id=$1 AND account_id=$2 ORDER BY sequence_number"#,
        )
        .bind(company).bind(acct).fetch_all(&pool).await.unwrap();
        assert_eq!(rows.len() as i32, n, "expected {n} ledger rows per account");
        let mut prev_after = Decimal::ZERO;
        for (k, (before, change, after)) in rows.iter().enumerate() {
            assert_eq!(before, &prev_after, "row {k}: balance_before != prior balance_after (chain broken)");
            assert_eq!(*after, *before + *change, "row {k}: balance_after != before + change");
            prev_after = *after;
        }
        assert_eq!(prev_after, expected, "final balance_after != expected sum");
    }

    // (c) Cached account balance agrees with the final ledger balance_after.
    for acct in [cash, rev] {
        let bal: Decimal = sqlx::query_scalar("SELECT current_balance FROM accounting.accounts WHERE id=$1")
            .bind(acct).fetch_one(&pool).await.unwrap();
        assert_eq!(bal, expected, "account {acct} current_balance drifted from ledger sum");
    }
}

// ── Probe 3: guarded composition does NOT expose write verbs on posted GL entities ──#[tokio::test]
async fn guarded_routes_lock_posted_gl_writes() {
    let pool = pool().await;
    let module = AccountingModule::builder().with_database(pool).build().unwrap();
    let router = create_guarded_accounting_routes(&module);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    // let the server bind
    tokio::time::sleep(std::time::Duration::from_millis(150)).await;

    let client = reqwest::Client::new();
    let base = format!("http://{addr}");

    // Read is allowed on posted GL entities.
    let get = client.get(format!("{base}/journals")).send().await.unwrap();
    assert!(get.status().is_success(), "GET /journals should be allowed");

    // Write verbs are NOT mounted → 405 Method Not Allowed (the path exists, POST doesn't).
    for path in ["/journals", "/ledgers", "/accounting_posts"] {
        let resp = client.post(format!("{base}{path}")).json(&serde_json::json!({})).send().await.unwrap();
        assert_eq!(
            resp.status().as_u16(), 405,
            "POST {path} must be locked (405), got {}", resp.status()
        );
    }
}
