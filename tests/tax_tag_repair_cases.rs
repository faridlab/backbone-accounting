//! Tax-tag legal-change repair cases — the guard set and the recompute —
//! against a real Postgres. Requires DATABASE_URL (defaults to the local
//! scratch Postgres on :5433). Each test seeds its own company_id, so tests
//! are isolated and parallel-safe.

use chrono::{Duration, NaiveDate};
use sqlx::PgPool;
use uuid::Uuid;

use backbone_accounting::application::service::tax_tag_repair_service::{
    RepairRequest, TaxTagRepairError, TaxTagRepairService, TaxTagRule,
};

async fn pool() -> PgPool {
    let url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgresql://postgres:postgres@localhost:5433/backbone_accounting".to_string()
    });
    PgPool::connect(&url).await.expect("connect DB")
}

fn window() -> (NaiveDate, NaiveDate) {
    let from = NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
    (from, from + Duration::days(31))
}

/// Seed one fiscal period with the given status covering the window.
async fn seed_period(pool: &PgPool, company: Uuid, status: &str, code: &str) {
    let (from, to) = window();
    sqlx::query(
        r#"INSERT INTO accounting.fiscal_periods
             (company_id, period_code, name, start_date, end_date, fiscal_year, status)
           VALUES ($1,$2,$3,$4,$5,$6,$7::period_status)"#,
    )
    .bind(company)
    .bind(code)
    .bind(format!("FY26/{code}"))
    .bind(from)
    .bind(to)
    .bind(2026)
    .bind(status)
    .execute(pool)
    .await
    .unwrap();
}

/// Seed one POSTED journal at the window start with one tax line and one base
/// line, both carrying the stale tag set. Returns the tax line's account id.
async fn seed_posted_journal(
    pool: &PgPool,
    company: Uuid,
    stale_tags: &str,
) -> Uuid {
    // journal_lines.account_id carries a real FK into accounts — seed the two
    // chart rows the lines post on.
    let tax_account: Uuid = sqlx::query_scalar(
        r#"INSERT INTO accounting.accounts
             (company_id, account_number, account_code, name,
              account_type, account_subtype, normal_balance)
           VALUES ($1,$2,$2,$3,'liability'::account_type,'tax'::account_subtype,'credit'::normal_balance)
           RETURNING id"#,
    )
    .bind(company)
    .bind(format!("2101-{}", company.simple()))
    .bind("VAT OUT")
    .fetch_one(pool)
    .await
    .unwrap();
    let base_account: Uuid = sqlx::query_scalar(
        r#"INSERT INTO accounting.accounts
             (company_id, account_number, account_code, name,
              account_type, account_subtype, normal_balance)
           VALUES ($1,$2,$2,$3,'revenue'::account_type,'operating_revenue'::account_subtype,'credit'::normal_balance)
           RETURNING id"#,
    )
    .bind(company)
    .bind(format!("4101-{}", company.simple()))
    .bind("SALES")
    .fetch_one(pool)
    .await
    .unwrap();

    let (from, _) = window();
    let journal: Uuid = sqlx::query_scalar(
        r#"INSERT INTO accounting.journals
             (company_id, journal_number, description, transaction_date, status)
           VALUES ($1,$2,$3,$4,'posted')
           RETURNING id"#,
    )
    .bind(company)
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
                 (journal_id, company_id, line_number, account_id, account_number,
                  account_name, debit_amount, credit_amount, is_posted, is_tax_line, tags)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,true,$9,$10::jsonb)"#,
        )
        .bind(journal)
        .bind(company)
        .bind(n)
        .bind(account)
        .bind(if is_tax { "2101" } else { "4101" })
        .bind(if is_tax { "VAT OUT" } else { "SALES" })
        .bind(if is_tax { rust_decimal::Decimal::ZERO } else { rust_decimal::Decimal::new(11_000_00, 2) })
        .bind(if is_tax { rust_decimal::Decimal::new(1_100_00, 2) } else { rust_decimal::Decimal::ZERO })
        .bind(is_tax)
        .bind(stale_tags)
        .execute(pool)
        .await
        .unwrap();
    }
    tax_account
}

fn request(
    company: Uuid,
    rules: Vec<TaxTagRule>,
    allow_closed: bool,
    dry_run: bool,
    reason: &str,
) -> RepairRequest {
    let (from, to) = window();
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
    let pool = pool().await;
    let company = Uuid::new_v4();
    let svc = TaxTagRepairService::new(pool.clone());
    seed_period(&pool, company, "open", "2026-01").await;
    let tax_account = seed_posted_journal(&pool, company, r#"["vat-out-old"]"#).await;

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
        "SELECT tags FROM accounting.journal_lines WHERE company_id=$1 AND is_tax_line",
    )
    .bind(company)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(tax_tags, serde_json::json!(["vat-out", "vat-11"]));
    let base_tags: serde_json::Value = sqlx::query_scalar(
        "SELECT tags FROM accounting.journal_lines WHERE company_id=$1 AND NOT is_tax_line",
    )
    .bind(company)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(base_tags, serde_json::json!(["vat-out-old"]));

    // The audit ledger read returns the run with everything stamped.
    let runs = svc.list_runs(company, 10).await.unwrap();
    assert_eq!(runs.len(), 1);
    let run = &runs[0];
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
}

/// Dry run: counts and reports WITHOUT writing (the audit row still lands).
#[tokio::test]
async fn dry_run_counts_without_writing() {
    let pool = pool().await;
    let company = Uuid::new_v4();
    let svc = TaxTagRepairService::new(pool.clone());
    seed_period(&pool, company, "open", "2026-01").await;
    seed_posted_journal(&pool, company, r#"["vat-out-old"]"#).await;

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
        "SELECT tags FROM accounting.journal_lines WHERE company_id=$1 AND is_tax_line",
    )
    .bind(company)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(tags, serde_json::json!(["vat-out-old"]));

    // The preview is still audited.
    let runs = svc.list_runs(company, 10).await.unwrap();
    assert_eq!(runs.len(), 1);
    assert!(runs[0].dry_run);
}

/// The guard set, one refusal each.
#[tokio::test]
async fn guard_refusals_are_typed() {
    let pool = pool().await;
    let company = Uuid::new_v4();
    let svc = TaxTagRepairService::new(pool.clone());
    seed_period(&pool, company, "open", "2026-01").await;

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
}

/// A window crossing a LOCKED period refuses outright — no override exists.
#[tokio::test]
async fn locked_period_refuses_without_override_path() {
    let pool = pool().await;
    let company = Uuid::new_v4();
    let svc = TaxTagRepairService::new(pool.clone());
    seed_period(&pool, company, "locked", "2026-01").await;
    seed_posted_journal(&pool, company, r#"["vat-out-old"]"#).await;

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
        "SELECT tags FROM accounting.journal_lines WHERE company_id=$1 AND is_tax_line",
    )
    .bind(company)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(tags, serde_json::json!(["vat-out-old"]));
    assert!(svc.list_runs(company, 10).await.unwrap().is_empty());
}

/// A window crossing a CLOSED period demands the explicit override; with it
/// armed the run proceeds and records WHICH periods it crossed.
#[tokio::test]
async fn closed_period_requires_explicit_override() {
    let pool = pool().await;
    let company = Uuid::new_v4();
    let svc = TaxTagRepairService::new(pool.clone());
    seed_period(&pool, company, "closed", "2026-01").await;
    seed_posted_journal(&pool, company, r#"["vat-out-old"]"#).await;

    // Without the override: typed refusal.
    let err = svc
        .recompute(request(company, vec![tax_rule(None, &["vat-out"])], false, false, "legal fix"))
        .await
        .unwrap_err();
    assert!(matches!(&err,
        TaxTagRepairError::ClosedPeriodRequiresOverride(p) if p.as_slice() == ["2026-01"]));
    assert_eq!(err.code(), "closed_period_requires_override");

    // With it: the repair applies and the audit row carries the override.
    let report = svc
        .recompute(request(company, vec![tax_rule(None, &["vat-out"])], true, false, "legal fix"))
        .await
        .unwrap();
    assert_eq!(report.lines_retagged, 1);
    assert_eq!(report.closed_periods_overridden, vec!["2026-01".to_string()]);

    let runs = svc.list_runs(company, 10).await.unwrap();
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].overridden_closed_periods, serde_json::json!(["2026-01"]));
}

/// Unposted lines and soft-deleted journals are never selected.
#[tokio::test]
async fn only_live_posted_lines_are_selected() {
    let pool = pool().await;
    let company = Uuid::new_v4();
    let svc = TaxTagRepairService::new(pool.clone());
    seed_period(&pool, company, "open", "2026-01").await;
    let tax_account = seed_posted_journal(&pool, company, r#"["vat-out-old"]"#).await;

    // Flip the journal soft-delete flag in its metadata bag.
    sqlx::query(
        r#"UPDATE accounting.journals
              SET metadata = metadata || '{"deleted_at":"2026-02-01T00:00:00Z"}'::jsonb
            WHERE company_id=$1"#,
    )
    .bind(company)
    .execute(&pool)
    .await
    .unwrap();

    let report = svc
        .recompute(request(company, vec![tax_rule(Some(tax_account), &["vat-out"])], false, false, "legal fix"))
        .await
        .unwrap();
    assert_eq!(report.lines_examined, 0);
    assert_eq!(report.lines_retagged, 0);
}
