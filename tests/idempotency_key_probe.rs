//! Pins the GL-post dedup semantics after the program-hardening audit (2026-07-11) made `idempotency_key`
//! a REAL dedup key. Three properties, so a future change to the dedup grain fails loudly here:
//!   IKP-1  key-based dedup: two posts with the SAME idempotency_key collapse to one (even different source_ids).
//!   IKP-2  the WIN: a producer emitting TWO distinct originals for ONE source_id, disambiguated by DISTINCT
//!          idempotency_keys, gets TWO journals — no more hand-namespacing source_id via Uuid::new_v5.
//!   IKP-3  backward compatible: keyless posts still dedup on the tuple (company, source_type, source_id,
//!          posting_type) — every existing producer (whose adapter drops the key) is unaffected.

use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

use backbone_accounting::application::service::posting_service::{PostingLine, PostingRequest, PostingService};

fn dec(s: &str) -> Decimal { Decimal::from_str_exact(s).unwrap() }
fn today() -> chrono::NaiveDate { chrono::Utc::now().date_naive() }

async fn pool() -> PgPool {
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://postgres:postgres@localhost:5433/backbone_accounting".to_string());
    PgPool::connect(&url).await.expect("connect DB")
}

async fn account(pool: &PgPool, company: Uuid, code: &str, atype: &str, subtype: &str, normal: &str) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO accounting.accounts
             (id, company_id, account_number, account_code, name, account_type, account_subtype,
              normal_balance, is_header, is_detail, status)
           VALUES ($1,$2,$3,$4,$5,$6::account_type,$7::account_subtype,$8::normal_balance,
                   false,true,'active'::account_status)"#,
    )
    .bind(id).bind(company).bind(code).bind(code).bind(code).bind(atype).bind(subtype).bind(normal)
    .execute(pool).await.expect("seed account");
    id
}

/// A balanced 2-line original (Dr expense · Cr bank) for `amount`.
fn balanced(company: Uuid, source_id: Uuid, dr: Uuid, cr: Uuid, amount: Decimal) -> PostingRequest {
    let mut r = PostingRequest::original(company, "manual", source_id, today());
    r.lines = vec![
        PostingLine { account_id: dr, debit: amount, credit: Decimal::ZERO, party_type: None, party_id: None,
            cost_center_id: None, project_id: None, department_id: None, description: None },
        PostingLine { account_id: cr, debit: Decimal::ZERO, credit: amount, party_type: None, party_id: None,
            cost_center_id: None, project_id: None, department_id: None, description: None },
    ];
    r
}

// IKP-1 — same idempotency_key ⇒ one journal (the second is an idempotent reuse), even with different source_ids.
#[tokio::test]
async fn ikp1_same_key_dedups() {
    let pool = pool().await;
    let company = Uuid::new_v4();
    let exp = account(&pool, company, "5000-K", "expense", "operating_expense", "debit").await;
    let bank = account(&pool, company, "1000-K", "asset", "bank", "debit").await;
    let svc = PostingService::new(std::sync::Arc::new(backbone_accounting::infrastructure::persistence::SqlxPostingRepository::new(pool.clone())));
    let key = format!("run:{}", Uuid::new_v4());

    let a = svc.post(balanced(company, Uuid::new_v4(), exp, bank, dec("1000")).with_idempotency_key(&key), None).await.unwrap();
    let b = svc.post(balanced(company, Uuid::new_v4(), exp, bank, dec("1000")).with_idempotency_key(&key), None).await.unwrap();
    assert!(!a.idempotent_reuse);
    assert!(b.idempotent_reuse, "same idempotency_key ⇒ deduped");
    assert_eq!(a.journal_id, b.journal_id, "one journal, not two");
}

// IKP-2 — the WIN: two DISTINCT originals for the SAME source_id, disambiguated by distinct keys, both land.
// This is what removes the need to hand-namespace source_id (Uuid::new_v5) for multi-post producers.
#[tokio::test]
async fn ikp2_distinct_keys_same_source_id_both_post() {
    let pool = pool().await;
    let company = Uuid::new_v4();
    let exp = account(&pool, company, "5000-K", "expense", "operating_expense", "debit").await;
    let bank = account(&pool, company, "1000-K", "asset", "bank", "debit").await;
    let svc = PostingService::new(std::sync::Arc::new(backbone_accounting::infrastructure::persistence::SqlxPostingRepository::new(pool.clone())));
    let one_document = Uuid::new_v4(); // e.g. a work order or a dividend — ONE source id, TWO posts

    let a = svc.post(balanced(company, one_document, exp, bank, dec("1000")).with_idempotency_key(format!("{one_document}:consume")), None).await.unwrap();
    let b = svc.post(balanced(company, one_document, exp, bank, dec("2000")).with_idempotency_key(format!("{one_document}:receive")), None).await.unwrap();
    assert!(!a.idempotent_reuse);
    assert!(!b.idempotent_reuse, "a distinct key posts a distinct journal even for the same source_id");
    assert_ne!(a.journal_id, b.journal_id, "TWO journals — the multi-post producer no longer loses the 2nd");
}

// IKP-3 — backward compatible: keyless posts still dedup on the tuple (existing producers unaffected).
#[tokio::test]
async fn ikp3_keyless_still_dedups_on_tuple() {
    let pool = pool().await;
    let company = Uuid::new_v4();
    let exp = account(&pool, company, "5000-K", "expense", "operating_expense", "debit").await;
    let bank = account(&pool, company, "1000-K", "asset", "bank", "debit").await;
    let svc = PostingService::new(std::sync::Arc::new(backbone_accounting::infrastructure::persistence::SqlxPostingRepository::new(pool.clone())));
    let source = Uuid::new_v4();

    // No idempotency_key set — legacy tuple dedup applies.
    let a = svc.post(balanced(company, source, exp, bank, dec("1000")), None).await.unwrap();
    let b = svc.post(balanced(company, source, exp, bank, dec("1000")), None).await.unwrap();
    assert!(!a.idempotent_reuse);
    assert!(b.idempotent_reuse, "keyless: same tuple still dedups (backward compatible)");
    assert_eq!(a.journal_id, b.journal_id);
}
