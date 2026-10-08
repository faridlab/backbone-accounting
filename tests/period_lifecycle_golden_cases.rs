//! Golden cases for the fiscal period lifecycle: close, lock and reopen, and the
//! transition fence every other writer meets. Requires DATABASE_URL (defaults to
//! local dev Postgres on :5433), migrated through
//! `20261008100000_fiscal_period_status_verbs`.
//!
//! Each test works in its own whole-month window derived from a fresh UUID and
//! sheds its period and journals afterwards, for the same reason the close cases
//! do: the posting guard reads periods by date overlap on an undecorated
//! database, so a leftover closed period would refuse later probes in its month.

use std::sync::Arc;

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
use backbone_accounting::infrastructure::persistence::{SqlxPeriodCloseRepository, SqlxPostingRepository};

static DB_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

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
    retained: Uuid,
    period: Uuid,
    month_start: NaiveDate,
}

async fn seed(pool: &PgPool) -> Setup {
    let company = Uuid::new_v4();
    let (bank, revenue, retained) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    let tag = &company.simple().to_string()[..8];
    for (id, code, at, st, nb) in [
        (bank, format!("11{tag}"), "asset", "cash", "debit"),
        (revenue, format!("41{tag}"), "revenue", "operating_revenue", "credit"),
        (retained, format!("32{tag}"), "equity", "retained_earnings", "credit"),
    ] {
        sqlx::query(
            r#"INSERT INTO accounting.accounts (id, account_number, account_code, name, account_type,
                account_subtype, normal_balance, is_detail, is_header, status)
               VALUES ($1,$2,$2,$2,$3::account_type,$4::account_subtype,$5::normal_balance,TRUE,FALSE,'active'::account_status)"#,
        )
        .bind(id).bind(code).bind(at).bind(st).bind(nb)
        .execute(pool).await.unwrap();
    }
    let offset = (company.as_u128() % 240) as u32;
    let month_start = NaiveDate::from_ymd_opt(2026, 1, 1)
        .unwrap()
        .checked_add_months(chrono::Months::new(offset))
        .unwrap();
    let (y, m) = (month_start.year(), month_start.month());
    let month_end = NaiveDate::from_ymd_opt(if m == 12 { y + 1 } else { y }, m % 12 + 1, 1).unwrap()
        - chrono::Duration::days(1);
    let period = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO accounting.fiscal_periods (id, period_code, name, period_type, fiscal_year,
            start_date, end_date, status)
           VALUES ($1,$2,$2,'monthly'::period_type,$3,$4,$5,'open'::period_status)"#,
    )
    .bind(period)
    .bind(format!("PLG-{tag}"))
    .bind(y)
    .bind(month_start)
    .bind(month_end)
    .execute(pool).await.unwrap();
    Setup { company, bank, revenue, retained, period, month_start }
}

fn services(pool: &PgPool) -> (PostingService, PeriodCloseService) {
    let posting = PostingService::new(Arc::new(SqlxPostingRepository::new(pool.clone())));
    let closer = PeriodCloseService::new(
        Arc::new(SqlxPostingRepository::new(pool.clone())),
        Arc::new(SqlxPeriodCloseRepository::new(pool.clone())),
    );
    (posting, closer)
}

/// Sell `amount` for cash inside the test's month.
async fn sell(posting: &PostingService, s: &Setup, amount: &str) {
    let amount = Decimal::from_str_exact(amount).unwrap();
    let line = |account, debit, credit| PostingLine {
        account_id: account,
        debit,
        credit,
        party_type: None,
        party_id: None,
        cost_center_id: None,
        project_id: None,
        department_id: None,
        description: None,
    };
    let mut r = PostingRequest::original(s.company, "manual", Uuid::new_v4(), s.month_start + chrono::Duration::days(14));
    r.lines = vec![line(s.bank, amount, Decimal::ZERO), line(s.revenue, Decimal::ZERO, amount)];
    posting.post(r, None).await.unwrap();
}

async fn shed(pool: &PgPool, period: Uuid) {
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

async fn status(pool: &PgPool, period: Uuid) -> String {
    sqlx::query_scalar("SELECT status::text FROM accounting.fiscal_periods WHERE id=$1")
        .bind(period).fetch_one(pool).await.unwrap()
}

async fn closing_entries(pool: &PgPool, period: Uuid) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM accounting.journals WHERE fiscal_period_id=$1 AND description='Period close'")
        .bind(period).fetch_one(pool).await.unwrap()
}

async fn balance(pool: &PgPool, account: Uuid) -> Decimal {
    sqlx::query_scalar("SELECT current_balance FROM accounting.accounts WHERE id=$1")
        .bind(account).fetch_one(pool).await.unwrap()
}

// PLG-1 — a close claims the period before it posts. Two closes racing for one
// period meet at that claim: the second claim fails, and a close that finds the
// period already claimed posts nothing. Driven through the claim directly, so
// the race is reproduced every run rather than left to scheduling.
#[tokio::test]
async fn plg1_a_claimed_period_cannot_be_closed_twice() {
    use backbone_accounting::domain::repositories::period_close_repository::PeriodCloseRepository;

    let _guard = DB_LOCK.lock().await;
    let pool = pool().await;
    let s = seed(&pool).await;
    let (posting, closer) = services(&pool);
    sell(&posting, &s, "1000000").await;
    let repo = SqlxPeriodCloseRepository::new(pool.clone());

    assert_eq!(repo.begin_close(s.period, None).await.unwrap().as_deref(), Some("open"));
    assert_eq!(repo.begin_close(s.period, None).await.unwrap(), None, "a second claim of the same period fails");

    let res = closer.close_period(s.period, s.retained).await;
    assert!(matches!(res, Err(PeriodCloseError::CloseInProgress)), "{res:?}");
    assert_eq!(closing_entries(&pool, s.period).await, 0, "the refused close posted nothing");

    // The holder of the claim finishes: one closing entry, period closed.
    repo.abort_close(s.period, "open").await.unwrap();
    closer.close_period(s.period, s.retained).await.unwrap();
    assert_eq!(closing_entries(&pool, s.period).await, 1);
    assert_eq!(status(&pool, s.period).await, "closed");
    shed(&pool, s.period).await;
}

// PLG-2 — lock needs a closed period and a reason, and records who and why.
#[tokio::test]
async fn plg2_lock_needs_a_closed_period_and_a_reason() {
    let _guard = DB_LOCK.lock().await;
    let pool = pool().await;
    let s = seed(&pool).await;
    let (_, closer) = services(&pool);
    let actor = Uuid::new_v4();

    let open = closer.lock_period(s.period, Some(actor), "year end").await;
    assert!(matches!(open, Err(PeriodCloseError::NotClosed { ref status }) if status == "open"), "{open:?}");

    closer.close_period(s.period, s.retained).await.unwrap();
    let blank = closer.lock_period(s.period, Some(actor), "   ").await;
    assert!(matches!(blank, Err(PeriodCloseError::ReasonRequired)), "{blank:?}");
    assert_eq!(status(&pool, s.period).await, "closed");

    closer.lock_period(s.period, Some(actor), "filed with the tax office").await.unwrap();
    let (st, by, why): (String, Option<Uuid>, Option<String>) = sqlx::query_as(
        "SELECT status::text, locked_by, lock_reason FROM accounting.fiscal_periods WHERE id=$1",
    )
    .bind(s.period).fetch_one(&pool).await.unwrap();
    assert_eq!((st.as_str(), by, why.as_deref()), ("locked", Some(actor), Some("filed with the tax office")));
    shed(&pool, s.period).await;
}

// PLG-3 — a locked period is final: it is neither reopened nor locked again.
#[tokio::test]
async fn plg3_a_locked_period_is_final() {
    let _guard = DB_LOCK.lock().await;
    let pool = pool().await;
    let s = seed(&pool).await;
    let (_, closer) = services(&pool);
    closer.close_period(s.period, s.retained).await.unwrap();
    closer.lock_period(s.period, None, "final").await.unwrap();

    let reopen = closer.reopen_period(s.period, None, "a late invoice").await;
    assert!(matches!(reopen, Err(PeriodCloseError::Locked)), "{reopen:?}");
    let relock = closer.lock_period(s.period, None, "again").await;
    assert!(matches!(relock, Err(PeriodCloseError::Locked)), "{relock:?}");
    assert_eq!(status(&pool, s.period).await, "locked");
    shed(&pool, s.period).await;
}

// PLG-4 — reopen records who and why, and the next close closes only what changed since:
// revenue 1,000,000 closed, reopened, 250,000 more sold, closed again →
// retained earnings 1,250,000 and revenue back to zero, with two closing entries.
#[tokio::test]
async fn plg4_reopen_then_close_again_closes_the_new_activity() {
    let _guard = DB_LOCK.lock().await;
    let pool = pool().await;
    let s = seed(&pool).await;
    let (posting, closer) = services(&pool);
    let actor = Uuid::new_v4();

    let blank = closer.reopen_period(s.period, Some(actor), "").await;
    assert!(matches!(blank, Err(PeriodCloseError::ReasonRequired)), "{blank:?}");
    let not_closed = closer.reopen_period(s.period, Some(actor), "why").await;
    assert!(matches!(not_closed, Err(PeriodCloseError::NotClosed { .. })), "{not_closed:?}");

    sell(&posting, &s, "1000000").await;
    closer.close_period(s.period, s.retained).await.unwrap();
    closer.reopen_period(s.period, Some(actor), "a late sale").await.unwrap();
    let (st, by, why): (String, Option<Uuid>, Option<String>) = sqlx::query_as(
        "SELECT status::text, reopened_by, reopen_reason FROM accounting.fiscal_periods WHERE id=$1",
    )
    .bind(s.period).fetch_one(&pool).await.unwrap();
    assert_eq!((st.as_str(), by, why.as_deref()), ("open", Some(actor), Some("a late sale")));

    sell(&posting, &s, "250000").await;
    let second = closer.close_period(s.period, s.retained).await.unwrap();
    assert_eq!(second.net_income, Decimal::from(250_000));
    assert!(second.closing_post_id.is_some(), "the second close posts its own entry");
    assert_eq!(closing_entries(&pool, s.period).await, 2);
    assert_eq!(balance(&pool, s.retained).await, Decimal::from(1_250_000));
    assert_eq!(balance(&pool, s.revenue).await, Decimal::ZERO);
    shed(&pool, s.period).await;
}

// PLG-5 — the transition fence holds a writer that bypasses the service.
#[tokio::test]
async fn plg5_the_database_refuses_a_move_outside_the_lifecycle() {
    let _guard = DB_LOCK.lock().await;
    let pool = pool().await;
    let s = seed(&pool).await;
    let (_, closer) = services(&pool);
    let set = |to: &'static str| {
        sqlx::query("UPDATE accounting.fiscal_periods SET status=$2::period_status WHERE id=$1")
            .bind(s.period)
            .bind(to)
    };

    // open → closed skips the close and its closing entry.
    let skip = set("closed").execute(&pool).await.unwrap_err();
    assert_eq!(skip.as_database_error().and_then(|e| e.constraint()), Some("fiscal_period_status_transition"));

    closer.close_period(s.period, s.retained).await.unwrap();
    closer.lock_period(s.period, None, "final").await.unwrap();
    // locked → open is the reopen a locked period may never have.
    let unlock = set("open").execute(&pool).await.unwrap_err();
    assert_eq!(unlock.as_database_error().and_then(|e| e.code()).as_deref(), Some("23514"));
    assert_eq!(status(&pool, s.period).await, "locked");

    // An edit that leaves the status alone passes the fence.
    sqlx::query("UPDATE accounting.fiscal_periods SET notes='checked' WHERE id=$1")
        .bind(s.period).execute(&pool).await.unwrap();
    shed(&pool, s.period).await;
}

// PLG-6 — a close whose closing entry is refused gives the period back as it was.
#[tokio::test]
async fn plg6_a_failed_close_returns_the_period_to_open() {
    let _guard = DB_LOCK.lock().await;
    let pool = pool().await;
    let s = seed(&pool).await;
    let (posting, closer) = services(&pool);
    sell(&posting, &s, "1000000").await;

    // A retained-earnings account that does not exist: the posting rules refuse the entry.
    // The close names an actor, so the claim it took is visible until it is given back.
    let res = closer.close_period_as(s.period, Uuid::new_v4(), Some(Uuid::new_v4())).await;
    assert!(matches!(res, Err(PeriodCloseError::Posting(_))), "{res:?}");
    let (st, started): (String, Option<Uuid>) = sqlx::query_as(
        "SELECT status::text, closing_started_by FROM accounting.fiscal_periods WHERE id=$1",
    )
    .bind(s.period).fetch_one(&pool).await.unwrap();
    assert_eq!((st.as_str(), started), ("open", None));
    assert_eq!(closing_entries(&pool, s.period).await, 0);
    shed(&pool, s.period).await;
}

// PLG-7 — the generic write refuses the status and the verb-owned trail, and
// still takes an ordinary edit of the same period.
#[tokio::test]
async fn plg7_a_generic_patch_cannot_move_the_status() {
    use backbone_accounting::application::service::FiscalPeriodService;
    use backbone_accounting::infrastructure::persistence::FiscalPeriodRepository;
    use backbone_core::ServiceError;

    let _guard = DB_LOCK.lock().await;
    let pool = pool().await;
    let s = seed(&pool).await;
    let (_, closer) = services(&pool);
    closer.close_period(s.period, s.retained).await.unwrap();
    closer.lock_period(s.period, None, "final").await.unwrap();
    let periods = FiscalPeriodService::with_repository(Arc::new(FiscalPeriodRepository::new(pool.clone())));
    let id = s.period.to_string();

    for (field, value) in [
        ("status", serde_json::json!("open")),
        ("lock_reason", serde_json::json!("never mind")),
        ("reopen_reason", serde_json::json!("forged")),
    ] {
        let res = periods.partial_update(&id, [(field.to_string(), value)].into()).await;
        assert!(
            matches!(&res, Err(ServiceError::Validation(m)) if m.contains("field_not_writable") && m.contains(field)),
            "{field}: {res:?}"
        );
    }
    assert_eq!(status(&pool, s.period).await, "locked");

    let edited = periods
        .partial_update(&id, [("notes".to_string(), serde_json::json!("audited"))].into())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(edited.notes.as_deref(), Some("audited"));
    shed(&pool, s.period).await;
}
