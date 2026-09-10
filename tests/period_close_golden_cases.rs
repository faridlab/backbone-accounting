//! Golden cases for fiscal-period close. Requires DATABASE_URL (defaults to local dev
//! Postgres on :5433).
//!
//! Tenancy: the module ships NONE (ADR-0029) — no table carries a tenant column and an
//! undecorated database has no fence. Each test derives its own whole-month period window
//! from a fresh UUID (the fiscal-period guard reads periods by date overlap globally
//! undecorated), runs under one lock, and SHEDS the period and its journals afterwards —
//! a leftover closed period would refuse every later probe posting inside that month.

use chrono::{Datelike, NaiveDate};
use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

use backbone_accounting::application::service::period_close_service::{
    PeriodCloseError, PeriodCloseService,
};
use backbone_accounting::application::service::posting_service::{
    PostingLine, PostingRequest, PostingService,
};

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
    let (bank, revenue, expense, retained) = (
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
    );
    for (id, code, name, at, st, nb) in [
        (bank, "1100", "Bank", "asset", "bank", "debit"),
        (
            revenue,
            "4000",
            "Pendapatan",
            "revenue",
            "operating_revenue",
            "credit",
        ),
        (
            expense,
            "5000",
            "Beban",
            "expense",
            "operating_expense",
            "debit",
        ),
        (
            retained,
            "3200",
            "Laba Ditahan",
            "equity",
            "retained_earnings",
            "credit",
        ),
    ] {
        sqlx::query(
            r#"INSERT INTO accounting.accounts (id, account_number, account_code, name, account_type,
                account_subtype, normal_balance, is_detail, is_header, status)
               VALUES ($1,$2,$2,$3,$4::account_type,$5::account_subtype,$6::normal_balance,TRUE,FALSE,'active'::account_status)"#,
        )
        .bind(id).bind(code).bind(name).bind(at).bind(st).bind(nb)
        .execute(pool).await.unwrap();
    }
    // Whole-month per-test window: see the header note on the globally-read
    // fiscal-period guard.
    let offset = (company.as_u128() % 240) as u32;
    let month_start = NaiveDate::from_ymd_opt(2026, 1, 1)
        .unwrap()
        .checked_add_months(chrono::Months::new(offset))
        .unwrap();
    let (y, m) = (month_start.year(), month_start.month());
    let month_end = NaiveDate::from_ymd_opt(if m == 12 { y + 1 } else { y }, m % 12 + 1, 1)
        .unwrap()
        - chrono::Duration::days(1);
    let period = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO accounting.fiscal_periods (id, period_code, name, period_type, fiscal_year,
            start_date, end_date, status)
           VALUES ($1,$2,$2,'monthly'::period_type,$3,$4,$5,'open'::period_status)"#,
    )
    .bind(period)
    .bind(format!("{y:04}-{m:02}"))
    .bind(y as i32)
    .bind(month_start)
    .bind(month_end)
    .execute(pool).await.unwrap();
    Setup {
        company,
        bank,
        revenue,
        expense,
        retained,
        period,
    }
}

fn line(account: Uuid, debit: &str, credit: &str) -> PostingLine {
    PostingLine {
        account_id: account,
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
async fn post(svc: &PostingService, company: Uuid, lines: Vec<PostingLine>) {
    let offset = (company.as_u128() % 240) as u32;
    let date = NaiveDate::from_ymd_opt(2026, 1, 15)
        .unwrap()
        .checked_add_months(chrono::Months::new(offset))
        .unwrap();
    let mut r = PostingRequest::original(company, "manual", Uuid::new_v4(), date);
    r.lines = lines;
    svc.post(r, None).await.unwrap();
}

/// Remove the test's period and every journal that references it (the regular
/// posts land in the period too, via the date lookup). The period guard reads
/// the table globally on an undecorated database — a leftover CLOSED period
/// would refuse every later probe whose window overlaps the month.
async fn shed_period_surface(pool: &PgPool, period: Uuid) {
    // journal_lines.ledger_id <-> ledgers.journal_line_id is a circular FK pair
    // (the line side nullable) — sever it before either side is deleted.
    for sql in [
        "UPDATE accounting.journal_lines SET ledger_id=NULL WHERE journal_id IN (SELECT id FROM accounting.journals WHERE fiscal_period_id=$1)",
        "DELETE FROM accounting.ledgers WHERE journal_id IN (SELECT id FROM accounting.journals WHERE fiscal_period_id=$1)",
        "DELETE FROM accounting.journal_lines WHERE journal_id IN (SELECT id FROM accounting.journals WHERE fiscal_period_id=$1)",
        "DELETE FROM accounting.accounting_posts WHERE journal_id IN (SELECT id FROM accounting.journals WHERE fiscal_period_id=$1)",
        "DELETE FROM accounting.journals WHERE fiscal_period_id=$1",
        "DELETE FROM accounting.fiscal_periods WHERE id=$1",
    ] {
        sqlx::query(sql).bind(period).execute(pool).await.unwrap();
    }
}
async fn balance(pool: &PgPool, id: Uuid) -> Decimal {
    sqlx::query_scalar("SELECT current_balance FROM accounting.accounts WHERE id=$1")
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap()
}
async fn period_status(pool: &PgPool, id: Uuid) -> String {
    sqlx::query_scalar("SELECT status::text FROM accounting.fiscal_periods WHERE id=$1")
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap()
}

// PCG-1 — close rolls net income (Revenue 1,000,000 − Expense 400,000) into Retained Earnings ─
#[tokio::test]
async fn pcg1_close_rolls_net_income() {
    let _guard = DB_LOCK.lock().await;
    let pool = pool().await;
    let s = seed(&pool).await;
    let posting = PostingService::new(std::sync::Arc::new(
        backbone_accounting::infrastructure::persistence::SqlxPostingRepository::new(pool.clone()),
    ));
    let closer = PeriodCloseService::new(
        std::sync::Arc::new(
            backbone_accounting::infrastructure::persistence::SqlxPostingRepository::new(
                pool.clone(),
            ),
        ),
        std::sync::Arc::new(
            backbone_accounting::infrastructure::persistence::SqlxPeriodCloseRepository::new(
                pool.clone(),
            ),
        ),
    );

    post(
        &posting,
        s.company,
        vec![
            line(s.bank, "1000000.00", "0"),
            line(s.revenue, "0", "1000000.00"),
        ],
    )
    .await;
    post(
        &posting,
        s.company,
        vec![
            line(s.expense, "400000.00", "0"),
            line(s.bank, "0", "400000.00"),
        ],
    )
    .await;

    let res = closer
        .close_period(s.company, s.period, s.retained)
        .await
        .unwrap();

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

    shed_period_surface(&pool, s.period).await;
}

// PCG-2 — closing an already-closed period is rejected ────────────────────────────────────
#[tokio::test]
async fn pcg2_double_close_rejected() {
    let _guard = DB_LOCK.lock().await;
    let pool = pool().await;
    let s = seed(&pool).await;
    let posting = PostingService::new(std::sync::Arc::new(
        backbone_accounting::infrastructure::persistence::SqlxPostingRepository::new(pool.clone()),
    ));
    let closer = PeriodCloseService::new(
        std::sync::Arc::new(
            backbone_accounting::infrastructure::persistence::SqlxPostingRepository::new(
                pool.clone(),
            ),
        ),
        std::sync::Arc::new(
            backbone_accounting::infrastructure::persistence::SqlxPeriodCloseRepository::new(
                pool.clone(),
            ),
        ),
    );
    post(
        &posting,
        s.company,
        vec![
            line(s.bank, "500000.00", "0"),
            line(s.revenue, "0", "500000.00"),
        ],
    )
    .await;

    closer
        .close_period(s.company, s.period, s.retained)
        .await
        .unwrap();
    let again = closer.close_period(s.company, s.period, s.retained).await;
    assert!(matches!(again, Err(PeriodCloseError::AlreadyClosed)));

    shed_period_surface(&pool, s.period).await;
}
