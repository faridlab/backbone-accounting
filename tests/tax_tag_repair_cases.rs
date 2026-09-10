//! Tax-tag legal-change repair cases — the guard set and the recompute —
//! against a real Postgres. Requires DATABASE_URL (defaults to the local
//! scratch Postgres on :5433).
//!
//! Tenancy: the module ships NONE (ADR-0029) — the request shapes keep the
//! legacy company twin but no table carries a tenant column, and an undecorated
//! database has no fence. Each test therefore isolates itself by identifier and
//! key: the fiscal-period window is offset by a distinct month count derived
//! from the test's company UUID (the locked/closed guards read periods by date
//! overlap globally, so overlapping windows would cross-fire), verification
//! queries key on the seeded journal id, and the audit ledger is filtered by
//! the run id the report returned instead of any tenant predicate. The probes
//! run under one lock and every seeded period is shed afterwards — the shared
//! global period table must never carry a (blocking) closed/locked row beyond
//! the probe that means to have one.

use chrono::{Datelike, Duration, Months, NaiveDate};
use sqlx::PgPool;
use uuid::Uuid;

use backbone_accounting::application::service::tax_tag_repair_service::{
    RepairRequest, TaxTagRepairError, TaxTagRepairService, TaxTagRule,
};

/// Serializes the database-touching probes in this file (see the header note): the
/// fiscal-period table is a global singleton surface on an undecorated database.
static DB_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

async fn pool() -> PgPool {
    let url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgresql://postgres:postgres@localhost:5433/backbone_accounting".to_string()
    });
    PgPool::connect(&url).await.expect("connect DB")
}

/// A distinct date window per test: January 2026 shifted by a month count
/// derived from the company UUID. Keeps every test's period rows disjoint from
/// every other test's, which is what the guard reads key on.
fn window(company: Uuid) -> (NaiveDate, NaiveDate) {
    let offset = (company.as_u128() % 240) as u32;
    let from = NaiveDate::from_ymd_opt(2026, 1, 1)
        .unwrap()
        .checked_add_months(Months::new(offset))
        .unwrap();
    (from, from + Duration::days(31))
}

/// The fiscal-period code the window falls in ("YYYY-MM" of the window start).
fn period_code(company: Uuid) -> String {
    let (from, _) = window(company);
    format!("{:04}-{:02}", from.year(), from.month())
}

/// Seed one fiscal period with the given status covering the test's window.
async fn seed_period(pool: &PgPool, company: Uuid, status: &str) {
    let (from, to) = window(company);
    sqlx::query(
        r#"INSERT INTO accounting.fiscal_periods
             (period_code, name, start_date, end_date, fiscal_year, status)
           VALUES ($1,$2,$3,$4,$5,$6::period_status)"#,
    )
    .bind(period_code(company))
    .bind(format!("FY26/{}", period_code(company)))
    .bind(from)
    .bind(to)
    .bind(from.year())
    .bind(status)
    .execute(pool)
    .await
    .unwrap();
}

/// Remove the test's period row again — the period guard reads the table
/// globally on an undecorated database, so a leftover closed/locked row would
/// refuse every later probe whose window overlaps it.
async fn shed_period(pool: &PgPool, company: Uuid) {
    sqlx::query("DELETE FROM accounting.fiscal_periods WHERE period_code = $1")
        .bind(period_code(company))
        .execute(pool)
        .await
        .unwrap();
}

/// Seed one POSTED journal at the window start with one tax line and one base
/// line, both carrying the stale tag set. Returns the tax line's account id and
/// the journal id (the key every verification query below reads through).
async fn seed_posted_journal(pool: &PgPool, company: Uuid, stale_tags: &str) -> (Uuid, Uuid) {
    // journal_lines.account_id carries a real FK into accounts — seed the two
    // chart rows the lines post on.
    let tax_account: Uuid = sqlx::query_scalar(
        r#"INSERT INTO accounting.accounts
             (account_number, account_code, name,
              account_type, account_subtype, normal_balance)
           VALUES ($1,$1,$2,'liability'::account_type,'tax'::account_subtype,'credit'::normal_balance)
           RETURNING id"#,
    )
    .bind(format!("2101-{}", company.simple()))
    .bind("VAT OUT")
    .fetch_one(pool)
    .await
    .unwrap();
    let base_account: Uuid = sqlx::query_scalar(
        r#"INSERT INTO accounting.accounts
             (account_number, account_code, name,
              account_type, account_subtype, normal_balance)
           VALUES ($1,$1,$2,'revenue'::account_type,'operating_revenue'::account_subtype,'credit'::normal_balance)
           RETURNING id"#,
    )
    .bind(format!("4101-{}", company.simple()))
    .bind("SALES")
    .fetch_one(pool)
    .await
    .unwrap();

    let (from, _) = window(company);
    let journal: Uuid = sqlx::query_scalar(
        r#"INSERT INTO accounting.journals
             (journal_number, description, transaction_date, status)
           VALUES ($1,$2,$3,'posted')
           RETURNING id"#,
    )
    .bind(format!("J-{}", Uuid::new_v4().simple()))
    .bind("stale tax tagging")
    .bind(from)
    .fetch_one(pool)
    .await
    .unwrap();

    for (n, (account, is_tax)) in
        [(1, (tax_account, true)), (2, (base_account, false))]
    {
        sqlx::query(
            r#"INSERT INTO accounting.journal_lines
                 (journal_id, line_number, account_id, account_number,
                  account_name, debit_amount, credit_amount, is_posted, is_tax_line, tags)
               VALUES ($1,$2,$3,$4,$5,$6,$7,true,$8,$9::jsonb)"#,
        )
        .bind(journal)
        .bind(n)
        .bind(account)
        .bind(if is_tax { format!("2101-{}", company.simple()) } else { format!("4101-{}", company.simple()) })
        .bind(if is_tax { "VAT OUT" } else { "SALES" })
        .bind(if is_tax { rust_decimal::Decimal::ZERO } else { rust_decimal::Decimal::new(11_000_00, 2) })
        .bind(if is_tax { rust_decimal::Decimal::new(1_100_00, 2) } else { rust_decimal::Decimal::ZERO })
        .bind(is_tax)
        .bind(stale_tags)
        .execute(pool)
        .await
        .unwrap();
    }
    (tax_account, journal)
}

fn request(
    company: Uuid,
    rules: Vec<TaxTagRule>,
    allow_closed: bool,
    dry_run: bool,
    reason: &str,
) -> RepairRequest {
    let (from, to) = window(company);
    RepairRequest {
        company_id: company,
        date_from: from,
        date_to: to,
        rules,
        allow_closed_periods: allow_closed,
        dry_run,
        actor: Some(Uuid::new_v4()),
        reason: reason.into(),
    }
}

fn tax_rule(account: Option<Uuid>, tags: &[&str]) -> TaxTagRule {
    TaxTagRule {
        account_id: account,
        is_tax_line: if account.is_none() { Some(true) } else { None },
        tags: tags.iter().map(|t| t.to_string()).collect(),
    }
}

/// Happy apply: selected lines retagged in place, others untouched, audit row
/// stamped with the exact rules, counts, officer, and reason.
#[tokio::test]
async fn apply_retags_selected_lines_and_stamps_audit() {
    let _guard = DB_LOCK.lock().await;
    let pool = pool().await;
    let company = Uuid::new_v4();
    let svc = TaxTagRepairService::new(pool.clone());
    seed_period(&pool, company, "open").await;
    let (tax_account, journal) = seed_posted_journal(&pool, company, r#"["vat-out-old"]"#).await;

    let report = svc
        .recompute(request(
            company,
            vec![tax_rule(Some(tax_account), &["vat-out", "vat-11"])],
            false,
            false,
            "PPN rate re-issue 11%: reclassify output VAT reporting",
        ))
        .await
        .unwrap();

    assert!(!report.dry_run);
    assert_eq!(report.lines_examined, 1);
    assert_eq!(report.lines_retagged, 1);
    assert_eq!(report.closed_periods_overridden, Vec::<String>::new());

    // The tax line carries the new tag set; the base line keeps the stale one.
    let tax_tags: serde_json::Value = sqlx::query_scalar(
        "SELECT tags FROM accounting.journal_lines WHERE journal_id=$1 AND is_tax_line",
    )
    .bind(journal)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(tax_tags, serde_json::json!(["vat-out", "vat-11"]));
    let base_tags: serde_json::Value = sqlx::query_scalar(
        "SELECT tags FROM accounting.journal_lines WHERE journal_id=$1 AND NOT is_tax_line",
    )
    .bind(journal)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(base_tags, serde_json::json!(["vat-out-old"]));

    // The audit ledger read returns the run with everything stamped.
    let runs = svc.list_runs(company, 10).await.unwrap();
    let run = runs.iter().find(|r| r.id == report.run_id).expect("the new run listed");
    assert!(!run.dry_run);
    assert_eq!(run.lines_examined, 1);
    assert_eq!(run.lines_retagged, 1);
    assert!(run.reason.contains("re-issue"));
    assert!(run.actor.is_some());
    assert_eq!(
        run.rules,
        serde_json::json!([{
            "account_id": tax_account,
            "is_tax_line": null,
            "tags": ["vat-out", "vat-11"],
        }])
    );

    shed_period(&pool, company).await;
}

/// Dry run: counts and reports WITHOUT writing (the audit row still lands).
#[tokio::test]
async fn dry_run_counts_without_writing() {
    let _guard = DB_LOCK.lock().await;
    let pool = pool().await;
    let company = Uuid::new_v4();
    let svc = TaxTagRepairService::new(pool.clone());
    seed_period(&pool, company, "open").await;
    let (_, journal) = seed_posted_journal(&pool, company, r#"["vat-out-old"]"#).await;

    let report = svc
        .recompute(request(
            company,
            vec![tax_rule(None, &["vat-out"])],
            false,
            true,
            "preview the reclassification before applying",
        ))
        .await
        .unwrap();

    assert!(report.dry_run);
    assert_eq!(report.lines_examined, 1);
    assert_eq!(report.lines_retagged, 1);

    // Nothing was written.
    let tags: serde_json::Value = sqlx::query_scalar(
        "SELECT tags FROM accounting.journal_lines WHERE journal_id=$1 AND is_tax_line",
    )
    .bind(journal)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(tags, serde_json::json!(["vat-out-old"]));

    // The preview is still audited.
    let runs = svc.list_runs(company, 10).await.unwrap();
    let run = runs.iter().find(|r| r.id == report.run_id).expect("the new run listed");
    assert!(run.dry_run);

    shed_period(&pool, company).await;
}

/// The guard set, one refusal each.
#[tokio::test]
async fn guard_refusals_are_typed() {
    let _guard = DB_LOCK.lock().await;
    let pool = pool().await;
    let company = Uuid::new_v4();
    let svc = TaxTagRepairService::new(pool.clone());
    seed_period(&pool, company, "open").await;

    // Empty reason.
    let err = svc
        .recompute(request(company, vec![tax_rule(None, &["t"])], false, false, "   "))
        .await
        .unwrap_err();
    assert_eq!(err.code(), "reason_required");

    // Inverted window.
    let mut inverted = request(company, vec![tax_rule(None, &["t"])], false, false, "legal fix");
    std::mem::swap(&mut inverted.date_from, &mut inverted.date_to);
    let err = svc.recompute(inverted).await.unwrap_err();
    assert_eq!(err.code(), "window_inverted");

    // Rule with no selector / no tags.
    let err = svc
        .recompute(request(
            company,
            vec![TaxTagRule { account_id: None, is_tax_line: None, tags: vec!["t".into()] }],
            false,
            false,
            "legal fix",
        ))
        .await
        .unwrap_err();
    assert_eq!(err.code(), "rule_without_selector");
    assert_eq!(err.http_status(), 422);
    let err = svc
        .recompute(request(company, vec![tax_rule(Some(Uuid::new_v4()), &[])], false, false, "legal fix"))
        .await
        .unwrap_err();
    assert_eq!(err.code(), "rule_without_tags");

    shed_period(&pool, company).await;
}

/// A window crossing a LOCKED period refuses outright — no override exists.
#[tokio::test]
async fn locked_period_refuses_without_override_path() {
    let _guard = DB_LOCK.lock().await;
    let pool = pool().await;
    let company = Uuid::new_v4();
    let svc = TaxTagRepairService::new(pool.clone());
    seed_period(&pool, company, "locked").await;
    let (_, journal) = seed_posted_journal(&pool, company, r#"["vat-out-old"]"#).await;

    // The audit ledger is a shared table on this undecorated database — judge the
    // refusal by a count delta, not by absolute emptiness.
    let runs_before: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM accounting.tax_tag_repair_runs")
            .fetch_one(&pool)
            .await
            .unwrap();

    // Even with the closed-period override armed, locked refuses.
    let err = svc
        .recompute(request(company, vec![tax_rule(None, &["vat-out"])], true, false, "legal fix"))
        .await
        .unwrap_err();
    assert_eq!(err.code(), "locked_period_in_window");
    assert_eq!(err.http_status(), 422);
    assert!(matches!(err, TaxTagRepairError::LockedPeriodInWindow(_)));

    // Nothing was retagged and nothing was audited.
    let tags: serde_json::Value = sqlx::query_scalar(
        "SELECT tags FROM accounting.journal_lines WHERE journal_id=$1 AND is_tax_line",
    )
    .bind(journal)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(tags, serde_json::json!(["vat-out-old"]));
    let runs_after: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM accounting.tax_tag_repair_runs")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(runs_after, runs_before, "the refused run must not be audited");

    shed_period(&pool, company).await;
}

/// A window crossing a CLOSED period demands the explicit override; with it
/// armed the run proceeds and records WHICH periods it crossed.
#[tokio::test]
async fn closed_period_requires_explicit_override() {
    let _guard = DB_LOCK.lock().await;
    let pool = pool().await;
    let company = Uuid::new_v4();
    let svc = TaxTagRepairService::new(pool.clone());
    seed_period(&pool, company, "closed").await;
    seed_posted_journal(&pool, company, r#"["vat-out-old"]"#).await;
    let code = period_code(company);

    // Without the override: typed refusal.
    let err = svc
        .recompute(request(company, vec![tax_rule(None, &["vat-out"])], false, false, "legal fix"))
        .await
        .unwrap_err();
    assert!(matches!(&err,
        TaxTagRepairError::ClosedPeriodRequiresOverride(p) if p.as_slice() == [code.as_str()]));
    assert_eq!(err.code(), "closed_period_requires_override");

    // With it: the repair applies and the audit row carries the override.
    let report = svc
        .recompute(request(company, vec![tax_rule(None, &["vat-out"])], true, false, "legal fix"))
        .await
        .unwrap();
    assert_eq!(report.lines_retagged, 1);
    assert_eq!(report.closed_periods_overridden, vec![code.clone()]);

    let runs = svc.list_runs(company, 10).await.unwrap();
    let run = runs.iter().find(|r| r.id == report.run_id).expect("the new run listed");
    assert_eq!(run.overridden_closed_periods, serde_json::json!([code]));

    shed_period(&pool, company).await;
}

/// Unposted lines and soft-deleted journals are never selected.
#[tokio::test]
async fn only_live_posted_lines_are_selected() {
    let _guard = DB_LOCK.lock().await;
    let pool = pool().await;
    let company = Uuid::new_v4();
    let svc = TaxTagRepairService::new(pool.clone());
    seed_period(&pool, company, "open").await;
    let (tax_account, journal) = seed_posted_journal(&pool, company, r#"["vat-out-old"]"#).await;

    // Flip the journal soft-delete flag in its metadata bag.
    sqlx::query(
        r#"UPDATE accounting.journals
              SET metadata = metadata || '{"deleted_at":"2026-02-01T00:00:00Z"}'::jsonb
            WHERE id=$1"#,
    )
    .bind(journal)
    .execute(&pool)
    .await
    .unwrap();

    let report = svc
        .recompute(request(company, vec![tax_rule(Some(tax_account), &["vat-out"])], false, false, "legal fix"))
        .await
        .unwrap();
    assert_eq!(report.lines_examined, 0);
    assert_eq!(report.lines_retagged, 0);

    shed_period(&pool, company).await;
}
