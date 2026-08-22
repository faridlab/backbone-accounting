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
        (
            "1200",
            "Piutang Usaha",
            "asset",
            "accounts_receivable",
            "debit",
        ),
        ("1210", "PPN Masukan", "asset", "tax", "debit"),
        (
            "2100",
            "Utang Usaha",
            "liability",
            "accounts_payable",
            "credit",
        ),
        ("2200", "PPN Keluaran", "liability", "tax", "credit"),
        ("2300", "Utang PPh 23", "liability", "tax", "credit"),
        (
            "4000",
            "Pendapatan",
            "revenue",
            "operating_revenue",
            "credit",
        ),
        (
            "5000",
            "Beban Operasional",
            "expense",
            "operating_expense",
            "debit",
        ),
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
        .bind(id)
        .bind(company_id)
        .bind(code)
        .bind(code)
        .bind(name)
        .bind(at)
        .bind(st)
        .bind(nb)
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
fn req(company: Uuid, source_type: &str, lines: Vec<PostingLine>) -> PostingRequest {
    let mut r = PostingRequest::original(company, source_type, Uuid::new_v4(), d15());
    r.lines = lines;
    r
}

/// Post GC-1 (sales invoice + PPN Output 11%).
async fn post_sales_invoice(svc: &PostingService, company: Uuid, a: &HashMap<&str, Uuid>) {
    let cust = Uuid::new_v4();
    svc.post(
        req(
            company,
            "order",
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
}

/// Post GC-3 (purchase invoice + PPN Input + PPh 23 withholding).
async fn post_purchase_invoice(svc: &PostingService, company: Uuid, a: &HashMap<&str, Uuid>) {
    let supp = Uuid::new_v4();
    svc.post(
        req(
            company,
            "expense",
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
    .unwrap();
}

// RGC-1 — reports after a single sales invoice ────────────────────────────────
#[tokio::test]
async fn rgc1_after_sales_invoice() {
    let pool = pool().await;
    let (company, a) = seed_coa(&pool).await;
    let posting = PostingService::new(std::sync::Arc::new(
        backbone_accounting::infrastructure::persistence::SqlxPostingRepository::new(pool.clone()),
    ));
    let reports = ReportingService::new(std::sync::Arc::new(
        backbone_accounting::infrastructure::persistence::SqlxReportingRepository::new(
            pool.clone(),
        ),
    ));
    post_sales_invoice(&posting, company, &a).await;

    let as_of = NaiveDate::from_ymd_opt(2026, 6, 30).unwrap();

    // Trial balance foots at 1,110,000.
    let tb = reports.trial_balance(company, as_of).await.unwrap();
    assert!(tb.balanced);
    assert_eq!(tb.total_debit, dec("1110000.00"));
    assert_eq!(tb.total_credit, dec("1110000.00"));
    assert_eq!(tb.lines.len(), 3);

    // Income statement: revenue 1,000,000, net income 1,000,000.
    let is = reports
        .income_statement(company, NaiveDate::from_ymd_opt(2026, 6, 1).unwrap(), as_of)
        .await
        .unwrap();
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
    let posting = PostingService::new(std::sync::Arc::new(
        backbone_accounting::infrastructure::persistence::SqlxPostingRepository::new(pool.clone()),
    ));
    let reports = ReportingService::new(std::sync::Arc::new(
        backbone_accounting::infrastructure::persistence::SqlxReportingRepository::new(
            pool.clone(),
        ),
    ));
    post_sales_invoice(&posting, company, &a).await;
    post_purchase_invoice(&posting, company, &a).await;

    let as_of = NaiveDate::from_ymd_opt(2026, 6, 30).unwrap();

    let tb = reports.trial_balance(company, as_of).await.unwrap();
    assert!(tb.balanced);
    assert_eq!(tb.total_debit, dec("1665000.00")); // AR 1.11M + Expense 500k + PPN In 55k

    let is = reports
        .income_statement(company, NaiveDate::from_ymd_opt(2026, 6, 1).unwrap(), as_of)
        .await
        .unwrap();
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
    let posting = PostingService::new(std::sync::Arc::new(
        backbone_accounting::infrastructure::persistence::SqlxPostingRepository::new(pool.clone()),
    ));
    let reports = ReportingService::new(std::sync::Arc::new(
        backbone_accounting::infrastructure::persistence::SqlxReportingRepository::new(
            pool.clone(),
        ),
    ));
    post_sales_invoice(&posting, company, &a).await; // posted on 2026-06-15

    // A July period contains no activity → revenue 0.
    let is = reports
        .income_statement(
            company,
            NaiveDate::from_ymd_opt(2026, 7, 1).unwrap(),
            NaiveDate::from_ymd_opt(2026, 7, 31).unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(is.revenue, dec("0"));
    assert_eq!(is.net_income, dec("0"));

    // A balance sheet as-of before the posting date shows nothing.
    let bs = reports
        .balance_sheet(company, NaiveDate::from_ymd_opt(2026, 6, 1).unwrap())
        .await
        .unwrap();
    assert_eq!(bs.assets, dec("0"));
    assert!(bs.balanced);
}

// Helper — insert one account row with explicit hierarchy flags/parent.
async fn seed_account(
    pool: &PgPool,
    company_id: Uuid,
    id: Uuid,
    parent: Option<Uuid>,
    code: &str,
    name: &str,
    at: &str,
    st: &str,
    nb: &str,
    is_header: bool,
    level: i32,
) {
    sqlx::query(
        r#"INSERT INTO accounting.accounts
            (id, company_id, parent_id, account_number, account_code, name, account_type,
             account_subtype, normal_balance, is_detail, is_header, level, status)
           VALUES ($1,$2,$3,$4,$4,$5,$6::account_type,$7::account_subtype,$8::normal_balance,
                   $9,$10,$11,'active'::account_status)"#,
    )
    .bind(id)
    .bind(company_id)
    .bind(parent)
    .bind(code)
    .bind(name)
    .bind(at)
    .bind(st)
    .bind(nb)
    .bind(!is_header)
    .bind(is_header)
    .bind(level)
    .execute(pool)
    .await
    .expect("seed account row");
}

// RGC-4 — general ledger: per-account sections with opening/closing from the running balance.
#[tokio::test]
async fn rgc4_general_ledger() {
    let pool = pool().await;
    let (company, a) = seed_coa(&pool).await;
    let posting = PostingService::new(std::sync::Arc::new(
        backbone_accounting::infrastructure::persistence::SqlxPostingRepository::new(pool.clone()),
    ));
    let reports = ReportingService::new(std::sync::Arc::new(
        backbone_accounting::infrastructure::persistence::SqlxReportingRepository::new(
            pool.clone(),
        ),
    ));
    post_sales_invoice(&posting, company, &a).await; // AR 1,110,000 / revenue 1,000,000 / PPN 110,000
    post_purchase_invoice(&posting, company, &a).await; // expense 500,000 / PPN-in 55,000 / AP 545,000 / PPh 10,000

    // Whole-June window, no account filter: one section per touched account (7 lines total).
    let gl = reports
        .general_ledger(
            company,
            None,
            Some(NaiveDate::from_ymd_opt(2026, 6, 1).unwrap()),
            NaiveDate::from_ymd_opt(2026, 6, 30).unwrap(),
            Some(50),
            0,
        )
        .await
        .unwrap();
    let total_lines: usize = gl.sections.iter().map(|s| s.lines.len()).sum();
    assert_eq!(total_lines, 7);
    assert_eq!(gl.sections.len(), 7);

    // Single-account window: the AR account opens at 0 and closes at its face.
    let ar = reports
        .general_ledger(
            company,
            Some(a["1200"]),
            None,
            NaiveDate::from_ymd_opt(2026, 6, 30).unwrap(),
            Some(50),
            0,
        )
        .await
        .unwrap();
    assert_eq!(ar.sections.len(), 1);
    let s = &ar.sections[0];
    assert_eq!(s.account_number, "1200");
    assert_eq!(s.opening_balance, dec("0"));
    assert_eq!(s.total_debit, dec("1110000.00"));
    assert_eq!(s.total_credit, dec("0"));
    assert_eq!(s.closing_balance, dec("1110000.00"));
    assert_eq!(s.lines.len(), 1);
    assert_eq!(s.lines[0].debit, dec("1110000.00"));
    assert!(s.lines[0].party_id.is_some());

    // A window before any activity shows no sections.
    let empty = reports
        .general_ledger(
            company,
            None,
            None,
            NaiveDate::from_ymd_opt(2026, 5, 31).unwrap(),
            Some(50),
            0,
        )
        .await
        .unwrap();
    assert!(empty.sections.is_empty());
}

// RGC-5 — partner ledger + aged AR/AP over open residuals.
#[tokio::test]
async fn rgc5_partner_ledger_and_aging() {
    let pool = pool().await;
    let (company, a) = seed_coa(&pool).await;
    let posting = PostingService::new(std::sync::Arc::new(
        backbone_accounting::infrastructure::persistence::SqlxPostingRepository::new(pool.clone()),
    ));
    let reports = ReportingService::new(std::sync::Arc::new(
        backbone_accounting::infrastructure::persistence::SqlxReportingRepository::new(
            pool.clone(),
        ),
    ));
    let cust = Uuid::new_v4();
    let supp = Uuid::new_v4();
    posting
        .post(
            req(
                company,
                "order",
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
    posting
        .post(
            req(
                company,
                "expense",
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
        .unwrap();

    // Partner ledger for the customer: one open AR line at face.
    let pl = reports
        .partner_ledger(company, "customer", cust, d15())
        .await
        .unwrap();
    assert_eq!(pl.lines.len(), 1);
    assert_eq!(pl.lines[0].account_number, "1200");
    assert_eq!(pl.lines[0].residual, dec("1110000.00"));
    assert_eq!(pl.total_debit, dec("1110000.00"));
    assert_eq!(pl.open_residual, dec("1110000.00"));

    // An unknown party has no lines.
    let none = reports
        .partner_ledger(company, "customer", Uuid::new_v4(), d15())
        .await
        .unwrap();
    assert!(none.lines.is_empty());
    assert_eq!(none.open_residual, dec("0"));

    // Aging on the invoice date: everything current.
    let ar0 = reports.aged_receivables(company, d15()).await.unwrap();
    assert_eq!(ar0.parties.len(), 1);
    assert_eq!(ar0.parties[0].party_id, cust);
    assert_eq!(ar0.parties[0].bucket_0_30, dec("1110000.00"));
    assert_eq!(ar0.parties[0].total, dec("1110000.00"));
    assert_eq!(ar0.totals.bucket_0_30, dec("1110000.00"));
    assert_eq!(ar0.totals.total, dec("1110000.00"));

    // 75 days later: 61–90 bucket. 91+ days later: the oldest bucket.
    let d75 = d15() + chrono::Duration::days(75);
    let ar75 = reports.aged_receivables(company, d75).await.unwrap();
    assert_eq!(ar75.parties[0].bucket_61_90, dec("1110000.00"));
    let d91 = d15() + chrono::Duration::days(91);
    let ar91 = reports.aged_receivables(company, d91).await.unwrap();
    assert_eq!(ar91.parties[0].bucket_91_plus, dec("1110000.00"));

    // The day before the invoice: nothing is open yet.
    let dprev = d15() - chrono::Duration::days(1);
    let arprev = reports.aged_receivables(company, dprev).await.unwrap();
    assert!(arprev.parties.is_empty());
    assert_eq!(arprev.totals.total, dec("0"));

    // Payables age on the supplier side.
    let ap = reports.aged_payables(company, d15()).await.unwrap();
    assert_eq!(ap.parties.len(), 1);
    assert_eq!(ap.parties[0].party_id, supp);
    assert_eq!(ap.parties[0].bucket_0_30, dec("545000.00"));
    assert_eq!(ap.totals.total, dec("545000.00"));
}

// RGC-6 — trial-balance tree: headers aggregate their detail descendants.
#[tokio::test]
async fn rgc6_trial_balance_tree() {
    let pool = pool().await;
    let company = Uuid::new_v4();
    let header = Uuid::new_v4();
    let bank = Uuid::new_v4();
    let ar = Uuid::new_v4();
    let revenue = Uuid::new_v4();
    seed_account(
        &pool,
        company,
        header,
        None,
        "1000",
        "Aset (header)",
        "asset",
        "current_asset",
        "debit",
        true,
        0,
    )
    .await;
    seed_account(
        &pool,
        company,
        bank,
        Some(header),
        "1100",
        "Bank",
        "asset",
        "bank",
        "debit",
        false,
        1,
    )
    .await;
    seed_account(
        &pool,
        company,
        ar,
        Some(header),
        "1200",
        "Piutang",
        "asset",
        "accounts_receivable",
        "debit",
        false,
        1,
    )
    .await;
    seed_account(
        &pool,
        company,
        revenue,
        None,
        "4000",
        "Pendapatan",
        "revenue",
        "operating_revenue",
        "credit",
        false,
        0,
    )
    .await;

    let posting = PostingService::new(std::sync::Arc::new(
        backbone_accounting::infrastructure::persistence::SqlxPostingRepository::new(pool.clone()),
    ));
    let reports = ReportingService::new(std::sync::Arc::new(
        backbone_accounting::infrastructure::persistence::SqlxReportingRepository::new(
            pool.clone(),
        ),
    ));
    posting
        .post(
            req(
                company,
                "order",
                vec![
                    line(bank, "500000.00", "0"),
                    party_line(line(ar, "610000.00", "0"), "customer", Uuid::new_v4()),
                    line(revenue, "0", "1110000.00"),
                ],
            ),
            None,
        )
        .await
        .unwrap();

    let tb = reports
        .trial_balance(company, NaiveDate::from_ymd_opt(2026, 6, 30).unwrap())
        .await
        .unwrap();
    assert!(tb.balanced);
    // Flat detail lines unchanged: Bank, AR, Revenue.
    assert_eq!(tb.lines.len(), 3);

    // Tree: the header root aggregates both asset children; revenue is a bare root.
    assert_eq!(tb.tree.len(), 2);
    let root = &tb.tree[0];
    assert_eq!(root.account_number, "1000");
    assert!(root.is_header);
    assert_eq!(root.debit, dec("1110000.00"));
    assert_eq!(root.credit, dec("0"));
    assert_eq!(root.children.len(), 2);
    assert_eq!(root.children[0].account_number, "1100");
    assert_eq!(root.children[0].debit, dec("500000.00"));
    assert_eq!(root.children[1].account_number, "1200");
    assert_eq!(root.children[1].debit, dec("610000.00"));
    let rev = &tb.tree[1];
    assert_eq!(rev.account_number, "4000");
    assert_eq!(rev.credit, dec("1110000.00"));
}

// RGC-7 — backdated posting: the GL running balance must follow DATE order, not the
// insertion order the materialized balance_before/balance_after columns chain in.
// A journal posted 06-15 followed by one posted 06-10 (backdated) must display the
// 06-10 line first with a date-ordered running balance; a window starting after the
// backdate must open at the backdated amount. The stored columns would show the
// 06-15 chain (500,000 then 800,000) regardless of date — silently inconsistent.
#[tokio::test]
async fn rgc7_backdated_posting_gl_consistency() {
    let pool = pool().await;
    let (company, a) = seed_coa(&pool).await;
    let posting = PostingService::new(std::sync::Arc::new(
        backbone_accounting::infrastructure::persistence::SqlxPostingRepository::new(pool.clone()),
    ));
    let reports = ReportingService::new(std::sync::Arc::new(
        backbone_accounting::infrastructure::persistence::SqlxReportingRepository::new(
            pool.clone(),
        ),
    ));

    let req_on = |date, bank_amount: &str| {
        let mut r = PostingRequest::original(company, "order", Uuid::new_v4(), date);
        r.lines = vec![
            line(a["1100"], bank_amount, "0"),
            line(a["4000"], "0", bank_amount),
        ];
        r
    };
    posting
        .post(req_on(d15(), "500000.00"), None)
        .await
        .unwrap();
    // Backdated five days earlier (300,000), inserted AFTER the 06-15 journal.
    posting
        .post(
            req_on(NaiveDate::from_ymd_opt(2026, 6, 10).unwrap(), "300000.00"),
            None,
        )
        .await
        .unwrap();

    // Full window on the bank account: the 06-10 line displays first and carries the
    // date-ordered running balance (300,000 → 800,000), not the insertion-ordered one.
    let gl = reports
        .general_ledger(
            company,
            Some(a["1100"]),
            None,
            NaiveDate::from_ymd_opt(2026, 6, 30).unwrap(),
            Some(50),
            0,
        )
        .await
        .unwrap();
    let s = &gl.sections[0];
    assert_eq!(s.lines.len(), 2);
    assert_eq!(
        s.lines[0].transaction_date,
        NaiveDate::from_ymd_opt(2026, 6, 10).unwrap(),
        "date order must win over insertion order"
    );
    assert_eq!(s.opening_balance, dec("0"));
    assert_eq!(s.lines[0].balance_after, dec("300000.00"));
    assert_eq!(s.lines[1].balance_after, dec("800000.00"));
    assert_eq!(s.closing_balance, dec("800000.00"));
    // Internal consistency (debit-normal account): closing = opening + Σdr − Σcr.
    assert_eq!(
        s.closing_balance,
        s.opening_balance + s.total_debit - s.total_credit
    );

    // Window starting AFTER the backdate (06-12): opens at the backdated 300,000 —
    // under the materialized columns it would open at 0 (the 06-15 row's balance_before).
    let later = reports
        .general_ledger(
            company,
            Some(a["1100"]),
            Some(NaiveDate::from_ymd_opt(2026, 6, 12).unwrap()),
            NaiveDate::from_ymd_opt(2026, 6, 30).unwrap(),
            Some(50),
            0,
        )
        .await
        .unwrap();
    let s2 = &later.sections[0];
    assert_eq!(s2.lines.len(), 1);
    assert_eq!(s2.opening_balance, dec("300000.00"));
    assert_eq!(s2.closing_balance, dec("800000.00"));

    // The GL closing must tie to the trial balance's net debit on the same account.
    let tb = reports
        .trial_balance(company, NaiveDate::from_ymd_opt(2026, 6, 30).unwrap())
        .await
        .unwrap();
    let tb_bank = tb
        .lines
        .iter()
        .find(|l| l.account_number == "1100")
        .expect("bank on TB");
    assert_eq!(tb_bank.debit - tb_bank.credit, s.closing_balance);
}

// The reporting reads take `company_id` from the query string; when a host has
// mounted an ambient company scope (company_auth's `with_company_scope`
// task-local), the query's company must agree with it — a mismatched request
// answers 403 company_mismatch instead of a misleading empty-but-balanced
// report branded with the foreign id (the database fence returns no rows either
// way; this pins the explicit contract on every route). Without an ambient
// scope the reads keep their standalone (trusted-host) shape.
#[tokio::test]
async fn reporting_reads_refuse_company_mismatch_under_ambient_scope() {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    let pool = pool().await;
    let (company, _a) = seed_coa(&pool).await;
    let other = Uuid::new_v4();
    let app =
        backbone_accounting::presentation::http::reporting_handler::create_reporting_routes(
            std::sync::Arc::new(ReportingService::new(std::sync::Arc::new(
                backbone_accounting::infrastructure::persistence::SqlxReportingRepository::new(
                    pool.clone(),
                ),
            ))),
        );

    let as_of = "2026-06-30";
    let paths = [
        format!("/accounting/reports/trial-balance?company_id={company}&as_of={as_of}"),
        format!("/accounting/reports/balance-sheet?company_id={company}&as_of={as_of}"),
        format!(
            "/accounting/reports/income-statement?company_id={company}&period_start=2026-06-01&period_end={as_of}"
        ),
        format!("/accounting/reports/general-ledger?company_id={company}&to_date={as_of}"),
        format!(
            "/accounting/reports/partner-ledger?company_id={company}&party_type=customer&party_id={}&as_of={as_of}",
            Uuid::new_v4()
        ),
        format!("/accounting/reports/aged-receivables?company_id={company}&as_of={as_of}"),
        format!("/accounting/reports/aged-payables?company_id={company}&as_of={as_of}"),
    ];
    for path in paths {
        let resp = backbone_orm::with_company_scope(Some(other), async {
            app.clone()
                .oneshot(
                    Request::builder()
                        .method("GET")
                        .uri(&path)
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap()
        })
        .await;
        assert_eq!(resp.status(), StatusCode::FORBIDDEN, "path: {path}");
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let text = String::from_utf8_lossy(&bytes).to_string();
        assert!(text.contains("company_mismatch"), "path: {path}, got: {text}");
    }

    // Matching ambient scope passes the tenant gate and answers 200 (the fresh
    // company has no postings, so an empty balanced report is the real answer).
    let resp = backbone_orm::with_company_scope(Some(company), async {
        app.clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!(
                        "/accounting/reports/trial-balance?company_id={company}&as_of={as_of}"
                    ))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap()
    })
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
}
