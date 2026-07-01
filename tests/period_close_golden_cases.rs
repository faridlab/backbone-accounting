//! Golden cases for fiscal-period close. Requires DATABASE_URL (defaults to local dev
//! Postgres on :5433). Fresh company per test → isolated & parallel-safe.

use chrono::NaiveDate;
use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

use backbone_accounting::application::service::period_close_service::{
    PeriodCloseError, PeriodCloseService,
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

struct Setup {
    company: Uuid,
    bank: Uuid,
    revenue: Uuid,
    expense: Uuid,
    retained: Uuid,
    period: Uuid,
}

async fn seed(pool: &PgPool) -> Setup {
    let company = Uuid::new_v4();
    let (bank, revenue, expense, retained) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    for (id, code, name, at, st, nb) in [
        (bank, "1100", "Bank", "asset", "bank", "debit"),
        (revenue, "4000", "Pendapatan", "revenue", "operating_revenue", "credit"),
        (expense, "5000", "Beban", "expense", "operating_expense", "debit"),
        (retained, "3200", "Laba Ditahan", "equity", "retained_earnings", "credit"),
    ] {
        sqlx::query(
            r#"INSERT INTO accounts (id, company_id, account_number, account_code, name, account_type,
                account_subtype, normal_balance, is_detail, is_header, status)
               VALUES ($1,$2,$3,$3,$4,$5::account_type,$6::account_subtype,$7::normal_balance,TRUE,FALSE,'active'::account_status)"#,
        )
        .bind(id).bind(company).bind(code).bind(name).bind(at).bind(st).bind(nb)
        .execute(pool).await.unwrap();
    }
    let period = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO fiscal_periods (id, company_id, period_code, name, period_type, fiscal_year,
            start_date, end_date, status)
           VALUES ($1,$2,'2026-06','June 2026','monthly'::period_type,2026,'2026-06-01','2026-06-30','open'::period_status)"#,
    )
    .bind(period).bind(company).execute(pool).await.unwrap();
    Setup { company, bank, revenue, expense, retained, period }
}

fn line(account: Uuid, debit: &str, credit: &str) -> PostingLine {
    PostingLine { account_id: account, debit: dec(debit), credit: dec(credit), party_type: None, party_id: None, cost_center_id: None, project_id: None, department_id: None, description: None }
}
async fn post(svc: &PostingService, company: Uuid, lines: Vec<PostingLine>) {
    let mut r = PostingRequest::original(company, "manual", Uuid::new_v4(), NaiveDate::from_ymd_opt(2026, 6, 15).unwrap());
    r.lines = lines;
    svc.post(r, None).await.unwrap();
}
async fn balance(pool: &PgPool, id: Uuid) -> Decimal {
    sqlx::query_scalar("SELECT current_balance FROM accounts WHERE id=$1").bind(id).fetch_one(pool).await.unwrap()
}
async fn period_status(pool: &PgPool, id: Uuid) -> String {
    sqlx::query_scalar("SELECT status::text FROM fiscal_periods WHERE id=$1").bind(id).fetch_one(pool).await.unwrap()
}

// PCG-1 — close rolls net income (Revenue 1,000,000 − Expense 400,000) into Retained Earnings ─
#[tokio::test]
async fn pcg1_close_rolls_net_income() {
    let pool = pool().await;
    let s = seed(&pool).await;
    let posting = PostingService::new(pool.clone());
    let closer = PeriodCloseService::new(pool.clone());

    post(&posting, s.company, vec![line(s.bank, "1000000.00", "0"), line(s.revenue, "0", "1000000.00")]).await;
    post(&posting, s.company, vec![line(s.expense, "400000.00", "0"), line(s.bank, "0", "400000.00")]).await;

    let res = closer.close_period(s.company, s.period, s.retained).await.unwrap();

    assert_eq!(res.net_income, dec("600000.00"));
    assert!(res.closing_journal_id.is_some());
    // P&L accounts zeroed; net income sits in Retained Earnings.
    assert_eq!(balance(&pool, s.revenue).await, dec("0.00"));
    assert_eq!(balance(&pool, s.expense).await, dec("0.00"));
    assert_eq!(balance(&pool, s.retained).await, dec("600000.00"));
    // Bank untouched by the close (1,000,000 − 400,000).
    assert_eq!(balance(&pool, s.bank).await, dec("600000.00"));
    // Period locked.
    assert_eq!(period_status(&pool, s.period).await, "closed");
}

// PCG-2 — closing an already-closed period is rejected ────────────────────────────────────
#[tokio::test]
async fn pcg2_double_close_rejected() {
    let pool = pool().await;
    let s = seed(&pool).await;
    let posting = PostingService::new(pool.clone());
    let closer = PeriodCloseService::new(pool.clone());
    post(&posting, s.company, vec![line(s.bank, "500000.00", "0"), line(s.revenue, "0", "500000.00")]).await;

    closer.close_period(s.company, s.period, s.retained).await.unwrap();
    let again = closer.close_period(s.company, s.period, s.retained).await;
    assert!(matches!(again, Err(PeriodCloseError::AlreadyClosed)));
}
