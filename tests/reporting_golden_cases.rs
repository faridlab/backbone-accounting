//! Golden-case oracle for financial-statement generation (Trial Balance, Balance Sheet,
//! Income Statement). Numbers are derived exactly from the posting golden cases (GC-1, GC-3).
//! Requires DATABASE_URL (defaults to local dev Postgres on :5433). Each test uses a fresh
//! company_id → isolated and parallel-safe.

use std::collections::HashMap;

use chrono::NaiveDate;
use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

use backbone_accounting::application::service::posting_service::{
    PostingLine, PostingRequest, PostingService,
};
use backbone_accounting::application::service::reporting_service::ReportingService;

fn dec(s: &str) -> Decimal {
    Decimal::from_str_exact(s).unwrap()
}
fn d15() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 6, 15).unwrap()
}

async fn pool() -> PgPool {
    let url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgresql://postgres:postgres@localhost:5433/backbone_accounting".to_string()
    });
    PgPool::connect(&url).await.expect("connect DB")
}

async fn seed_coa(pool: &PgPool) -> (Uuid, HashMap<&'static str, Uuid>) {
    let company_id = Uuid::new_v4();
    let coa: &[(&str, &str, &str, &str, &str)] = &[
        ("1100", "Bank BCA", "asset", "bank", "debit"),
        ("1200", "Piutang Usaha", "asset", "accounts_receivable", "debit"),
        ("1210", "PPN Masukan", "asset", "tax", "debit"),
        ("2100", "Utang Usaha", "liability", "accounts_payable", "credit"),
        ("2200", "PPN Keluaran", "liability", "tax", "credit"),
        ("2300", "Utang PPh 23", "liability", "tax", "credit"),
        ("4000", "Pendapatan", "revenue", "operating_revenue", "credit"),
        ("5000", "Beban Operasional", "expense", "operating_expense", "debit"),
    ];
    let mut map = HashMap::new();
    for (code, name, at, st, nb) in coa {
        let id = Uuid::new_v4();
        sqlx::query(
            r#"INSERT INTO accounting.accounts
                (id, company_id, account_number, account_code, name, account_type, account_subtype,
                 normal_balance, is_detail, is_header, status)
               VALUES ($1,$2,$3,$4,$5,$6::account_type,$7::account_subtype,$8::normal_balance,
                       TRUE, FALSE, 'active'::account_status)"#,
        )
        .bind(id).bind(company_id).bind(code).bind(code).bind(name).bind(at).bind(st).bind(nb)
        .execute(pool).await.expect("seed account");
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
fn req(company: Uuid, source_type: &str, lines: Vec<PostingLine>) -> PostingRequest {
    let mut r = PostingRequest::original(company, source_type, Uuid::new_v4(), d15());
    r.lines = lines;
    r
}

/// Post GC-1 (sales invoice + PPN Output 11%).
async fn post_sales_invoice(svc: &PostingService, company: Uuid, a: &HashMap<&str, Uuid>) {
    let cust = Uuid::new_v4();
    svc.post(req(company, "order", vec![
        party_line(line(a["1200"], "1110000.00", "0"), "customer", cust),
        line(a["4000"], "0", "1000000.00"),
        line(a["2200"], "0", "110000.00"),
    ]), None).await.unwrap();
}

/// Post GC-3 (purchase invoice + PPN Input + PPh 23 withholding).
async fn post_purchase_invoice(svc: &PostingService, company: Uuid, a: &HashMap<&str, Uuid>) {
    let supp = Uuid::new_v4();
    svc.post(req(company, "expense", vec![
        line(a["5000"], "500000.00", "0"),
        line(a["1210"], "55000.00", "0"),
        party_line(line(a["2100"], "0", "545000.00"), "supplier", supp),
        line(a["2300"], "0", "10000.00"),
    ]), None).await.unwrap();
}

// RGC-1 — reports after a single sales invoice ────────────────────────────────
#[tokio::test]
async fn rgc1_after_sales_invoice() {
    let pool = pool().await;
    let (company, a) = seed_coa(&pool).await;
    let posting = PostingService::new(pool.clone());
    let reports = ReportingService::new(pool.clone());
    post_sales_invoice(&posting, company, &a).await;

    let as_of = NaiveDate::from_ymd_opt(2026, 6, 30).unwrap();

    // Trial balance foots at 1,110,000.
    let tb = reports.trial_balance(company, as_of).await.unwrap();
    assert!(tb.balanced);
    assert_eq!(tb.total_debit, dec("1110000.00"));
    assert_eq!(tb.total_credit, dec("1110000.00"));
    assert_eq!(tb.lines.len(), 3);

    // Income statement: revenue 1,000,000, net income 1,000,000.
    let is = reports.income_statement(company, NaiveDate::from_ymd_opt(2026, 6, 1).unwrap(), as_of).await.unwrap();
    assert_eq!(is.revenue, dec("1000000.00"));
    assert_eq!(is.expenses, dec("0"));
    assert_eq!(is.net_income, dec("1000000.00"));

    // Balance sheet: Assets 1,110,000 = Liabilities 110,000 + Equity 0 + Current earnings 1,000,000.
    let bs = reports.balance_sheet(company, as_of).await.unwrap();
    assert_eq!(bs.assets, dec("1110000.00"));
    assert_eq!(bs.liabilities, dec("110000.00"));
    assert_eq!(bs.equity, dec("0"));
    assert_eq!(bs.current_earnings, dec("1000000.00"));
    assert!(bs.balanced);
}

// RGC-2 — reports after sales + purchase ──────────────────────────────────────
#[tokio::test]
async fn rgc2_after_sales_and_purchase() {
    let pool = pool().await;
    let (company, a) = seed_coa(&pool).await;
    let posting = PostingService::new(pool.clone());
    let reports = ReportingService::new(pool.clone());
    post_sales_invoice(&posting, company, &a).await;
    post_purchase_invoice(&posting, company, &a).await;

    let as_of = NaiveDate::from_ymd_opt(2026, 6, 30).unwrap();

    let tb = reports.trial_balance(company, as_of).await.unwrap();
    assert!(tb.balanced);
    assert_eq!(tb.total_debit, dec("1665000.00")); // AR 1.11M + Expense 500k + PPN In 55k

    let is = reports.income_statement(company, NaiveDate::from_ymd_opt(2026, 6, 1).unwrap(), as_of).await.unwrap();
    assert_eq!(is.revenue, dec("1000000.00"));
    assert_eq!(is.expenses, dec("500000.00"));
    assert_eq!(is.net_income, dec("500000.00"));

    let bs = reports.balance_sheet(company, as_of).await.unwrap();
    assert_eq!(bs.assets, dec("1165000.00")); // AR 1,110,000 + PPN Input 55,000
    assert_eq!(bs.liabilities, dec("665000.00")); // PPN Out 110k + AP 545k + PPh 10k
    assert_eq!(bs.current_earnings, dec("500000.00"));
    assert!(bs.balanced);
    assert_eq!(bs.assets, bs.total_liabilities_and_equity);
}

// RGC-3 — period filter excludes activity outside the window ──────────────────
#[tokio::test]
async fn rgc3_period_filter() {
    let pool = pool().await;
    let (company, a) = seed_coa(&pool).await;
    let posting = PostingService::new(pool.clone());
    let reports = ReportingService::new(pool.clone());
    post_sales_invoice(&posting, company, &a).await; // posted on 2026-06-15

    // A July period contains no activity → revenue 0.
    let is = reports.income_statement(
        company,
        NaiveDate::from_ymd_opt(2026, 7, 1).unwrap(),
        NaiveDate::from_ymd_opt(2026, 7, 31).unwrap(),
    ).await.unwrap();
    assert_eq!(is.revenue, dec("0"));
    assert_eq!(is.net_income, dec("0"));

    // A balance sheet as-of before the posting date shows nothing.
    let bs = reports.balance_sheet(company, NaiveDate::from_ymd_opt(2026, 6, 1).unwrap()).await.unwrap();
    assert_eq!(bs.assets, dec("0"));
    assert!(bs.balanced);
}
