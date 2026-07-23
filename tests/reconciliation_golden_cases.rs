//! Golden cases for bank reconciliation matching. Requires DATABASE_URL (defaults to local
//! dev Postgres on :5433). Fresh company per test → isolated & parallel-safe.

use chrono::NaiveDate;
use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

use backbone_accounting::application::service::bank_reconciliation_service::{
    BankReconciliationService, ReconcileRequest, StatementLine,
};
use backbone_accounting::application::service::posting_service::{
    PostingLine, PostingRequest, PostingService,
};

fn dec(s: &str) -> Decimal {
    Decimal::from_str_exact(s).unwrap()
}
async fn pool() -> PgPool {
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://postgres:postgres@localhost:5433/backbone_accounting".to_string());
    PgPool::connect(&url).await.unwrap()
}

/// Seed a bank + revenue account; return (company, bank_id, revenue_id).
async fn seed(pool: &PgPool) -> (Uuid, Uuid, Uuid) {
    let company = Uuid::new_v4();
    let bank = Uuid::new_v4();
    let revenue = Uuid::new_v4();
    for (id, code, name, at, st, nb) in [
        (bank, "1100", "Bank BCA", "asset", "bank", "debit"),
        (revenue, "4000", "Pendapatan", "revenue", "operating_revenue", "credit"),
    ] {
        sqlx::query(
            r#"INSERT INTO accounting.accounts (id, company_id, account_number, account_code, name, account_type,
                account_subtype, normal_balance, is_detail, is_header, status)
               VALUES ($1,$2,$3,$3,$4,$5::account_type,$6::account_subtype,$7::normal_balance,TRUE,FALSE,'active'::account_status)"#,
        )
        .bind(id).bind(company).bind(code).bind(name).bind(at).bind(st).bind(nb)
        .execute(pool).await.unwrap();
    }
    (company, bank, revenue)
}

/// Post a cash receipt: Dr Bank amount · Cr Revenue amount → one +amount ledger row on bank.
async fn receipt(svc: &PostingService, company: Uuid, bank: Uuid, revenue: Uuid, amount: &str) {
    let mut r = PostingRequest::original(company, "payment", Uuid::new_v4(), NaiveDate::from_ymd_opt(2026, 6, 15).unwrap());
    r.lines = vec![
        PostingLine { account_id: bank, debit: dec(amount), credit: Decimal::ZERO, party_type: None, party_id: None, cost_center_id: None, project_id: None, department_id: None, description: None },
        PostingLine { account_id: revenue, debit: Decimal::ZERO, credit: dec(amount), party_type: None, party_id: None, cost_center_id: None, project_id: None, department_id: None, description: None },
    ];
    svc.post(r, None).await.unwrap();
}

async fn is_reconciled_count(pool: &PgPool, company: Uuid, account: Uuid) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM accounting.ledgers WHERE company_id=$1 AND account_id=$2 AND is_reconciled=TRUE")
        .bind(company).bind(account).fetch_one(pool).await.unwrap()
}

fn stmt(amount: &str) -> StatementLine {
    StatementLine { date: NaiveDate::from_ymd_opt(2026, 6, 20).unwrap(), amount: dec(amount), reference: None }
}

fn req(company: Uuid, bank: Uuid, lines: Vec<StatementLine>) -> ReconcileRequest {
    ReconcileRequest {
        company_id: company,
        account_id: bank,
        period_start: NaiveDate::from_ymd_opt(2026, 6, 1).unwrap(),
        period_end: NaiveDate::from_ymd_opt(2026, 6, 30).unwrap(),
        statement_date: NaiveDate::from_ymd_opt(2026, 6, 30).unwrap(),
        statement_lines: lines,
    }
}

// RCG-1 — partial match: 2 of 3 statement lines match, 1 book + 1 statement outstanding ──────
#[tokio::test]
async fn rcg1_partial_match() {
    let pool = pool().await;
    let (company, bank, revenue) = seed(&pool).await;
    let posting = PostingService::new(std::sync::Arc::new(backbone_accounting::infrastructure::persistence::SqlxPostingRepository::new(pool.clone())));
    let recon = BankReconciliationService::new(std::sync::Arc::new(backbone_accounting::infrastructure::persistence::SqlxBankReconciliationRepository::new(pool.clone())));

    receipt(&posting, company, bank, revenue, "100.00").await;
    receipt(&posting, company, bank, revenue, "200.00").await;
    receipt(&posting, company, bank, revenue, "300.00").await;

    // Statement: 100 & 200 match; 999 does not.
    let res = recon.reconcile(req(company, bank, vec![stmt("100.00"), stmt("200.00"), stmt("999.00")])).await.unwrap();

    assert_eq!(res.matched_count, 2);
    assert_eq!(res.unmatched_book, 1);       // the 300 receipt
    assert_eq!(res.unmatched_statement, 1);  // the 999 line
    assert!(!res.is_balanced);
    assert_eq!(res.closing_book_balance, dec("600.00"));
    assert_eq!(res.closing_statement_balance, dec("1299.00"));
    // Exactly the 2 matched ledger rows are flagged reconciled.
    assert_eq!(is_reconciled_count(&pool, company, bank).await, 2);
}

// RCG-2 — full match: everything reconciles, difference zero ──────────────────────────────
#[tokio::test]
async fn rcg2_full_match() {
    let pool = pool().await;
    let (company, bank, revenue) = seed(&pool).await;
    let posting = PostingService::new(std::sync::Arc::new(backbone_accounting::infrastructure::persistence::SqlxPostingRepository::new(pool.clone())));
    let recon = BankReconciliationService::new(std::sync::Arc::new(backbone_accounting::infrastructure::persistence::SqlxBankReconciliationRepository::new(pool.clone())));

    receipt(&posting, company, bank, revenue, "100.00").await;
    receipt(&posting, company, bank, revenue, "200.00").await;
    receipt(&posting, company, bank, revenue, "300.00").await;

    let res = recon.reconcile(req(company, bank, vec![stmt("100.00"), stmt("200.00"), stmt("300.00")])).await.unwrap();

    assert_eq!(res.matched_count, 3);
    assert_eq!(res.unmatched_book, 0);
    assert_eq!(res.unmatched_statement, 0);
    assert!(res.is_balanced);
    assert_eq!(res.difference, dec("0.00"));
    assert_eq!(is_reconciled_count(&pool, company, bank).await, 3);
}
