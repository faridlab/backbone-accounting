//! Chart install engine cases — the install/refuse/idempotency contract, against a
//! real Postgres. Requires DATABASE_URL (defaults to the local dev Postgres on :5433).
//! Each test seeds its own company_id, so tests are isolated and parallel-safe.

use backbone_accounting::application::service::chart_install_service::{
    ChartInstallError, ChartInstallService,
};
use backbone_accounting::domain::chart_dataset::{
    validate_dataset, ChartAccountDef, ChartDataset,
};
use backbone_accounting::domain::entity::{AccountSubtype, AccountType, NormalBalance};
use backbone_accounting::infrastructure::persistence::chart_install_repository::SqlxChartInstallRepository;
use sqlx::{PgPool, Row};
use std::sync::Arc;
use uuid::Uuid;

async fn pool() -> PgPool {
    let url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgresql://postgres:postgres@localhost:5433/backbone_accounting".to_string()
    });
    PgPool::connect(&url).await.expect("connect DB")
}

fn def(
    number: &str,
    name: &str,
    typ: AccountType,
    sub: AccountSubtype,
    bal: NormalBalance,
    parent: Option<&str>,
    reconcilable: bool,
    sort: i32,
) -> ChartAccountDef {
    ChartAccountDef {
        number: number.to_string(),
        code: number.replace('.', ""),
        name: name.to_string(),
        account_type: typ,
        account_subtype: sub,
        normal_balance: bal,
        parent_code: parent.map(str::to_string),
        is_reconcilable: reconcilable,
        currency: "IDR".to_string(),
        sort_order: sort,
    }
}

/// Small SAK-shaped tree: two roots, one leaf under each plus a tax leaf under 2000.
fn chart() -> ChartDataset {
    ChartDataset {
        code: "TEST_CHART".into(),
        version: "1.0".into(),
        name: "Test chart".into(),
        accounts: vec![
            def("1000", "Aset", AccountType::Asset, AccountSubtype::CurrentAsset, NormalBalance::Debit, None, false, 1),
            def("1100", "Kas", AccountType::Asset, AccountSubtype::Cash, NormalBalance::Debit, Some("1000"), true, 2),
            def("1110", "Bank", AccountType::Asset, AccountSubtype::Bank, NormalBalance::Debit, Some("1000"), true, 3),
            def("1200", "Piutang Usaha", AccountType::Asset, AccountSubtype::AccountsReceivable, NormalBalance::Debit, Some("1000"), true, 4),
            def("2000", "Liabilitas", AccountType::Liability, AccountSubtype::CurrentLiability, NormalBalance::Credit, None, false, 5),
            def("2100", "Utang Usaha", AccountType::Liability, AccountSubtype::AccountsPayable, NormalBalance::Credit, Some("2000"), true, 6),
            def("2110", "PPN Keluaran", AccountType::Liability, AccountSubtype::Tax, NormalBalance::Credit, Some("2000"), false, 7),
        ],
    }
}

fn service(pool: &PgPool) -> ChartInstallService {
    ChartInstallService::new(
        Arc::new(SqlxChartInstallRepository::new()),
        pool.clone(),
        vec![Arc::new(chart())],
    )
}

async fn count_accounts(pool: &PgPool, company_id: Uuid) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM accounting.accounts WHERE company_id = $1")
        .bind(company_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn one(pool: &PgPool, company_id: Uuid, number: &str) -> sqlx::postgres::PgRow {
    sqlx::query(
        r#"SELECT id, account_number, parent_id, level, path, is_header, is_detail,
                  chart_code, chart_version, name, metadata->>'deleted_at' AS deleted_at
             FROM accounting.accounts
            WHERE company_id = $1 AND account_number = $2"#,
    )
    .bind(company_id)
    .bind(number)
    .fetch_one(pool)
    .await
    .unwrap()
}

#[tokio::test]
async fn install_on_fresh_company_creates_full_tree() {
    let pool = pool().await;
    let company = Uuid::new_v4();
    let report = service(&pool).install(company, "TEST_CHART").await.unwrap();

    assert_eq!(report.accounts_installed, 7);
    assert_eq!(report.accounts_updated, 0);
    assert_eq!(report.account_ids.len(), 7);

    let root = one(&pool, company, "1000").await;
    assert_eq!(root.get::<i32, _>("level"), 0);
    assert_eq!(root.get::<Option<&str>, _>("path"), Some("1000"));
    assert!(root.get::<bool, _>("is_header"));
    assert!(!root.get::<bool, _>("is_detail"));
    assert_eq!(root.get::<Option<&str>, _>("chart_code"), Some("TEST_CHART"));
    assert_eq!(root.get::<Option<&str>, _>("chart_version"), Some("1.0"));

    let leaf = one(&pool, company, "1100").await;
    assert_eq!(leaf.get::<i32, _>("level"), 1);
    assert_eq!(leaf.get::<Option<&str>, _>("path"), Some("1000/1100"));
    assert!(!leaf.get::<bool, _>("is_header"));
    assert!(leaf.get::<bool, _>("is_detail"));
    // deterministic id map matches the stored row
    assert_eq!(leaf.get::<Uuid, _>("id"), report.account_ids["1100"]);
    // parent linkage follows the deterministic ids
    assert_eq!(
        leaf.get::<Option<Uuid>, _>("parent_id"),
        Some(report.account_ids["1000"])
    );
}

#[tokio::test]
async fn reinstall_updates_not_duplicates() {
    let pool = pool().await;
    let company = Uuid::new_v4();
    let svc = service(&pool);
    svc.install(company, "TEST_CHART").await.unwrap();
    let second = svc.install(company, "TEST_CHART").await.unwrap();

    assert_eq!(second.accounts_installed, 0);
    assert_eq!(second.accounts_updated, 7);
    assert_eq!(count_accounts(&pool, company).await, 7);
}

#[tokio::test]
async fn manager_rename_survives_reinstall_and_reparent_reverts() {
    let pool = pool().await;
    let company = Uuid::new_v4();
    let svc = service(&pool);
    let first = svc.install(company, "TEST_CHART").await.unwrap();

    // Manager edits post-install: a rename (user-owned) and a re-parent (engine-owned).
    sqlx::query("UPDATE accounting.accounts SET name = 'Kas Kecil' WHERE company_id = $1 AND account_number = '1100'")
        .bind(company)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE accounting.accounts SET parent_id = $2, path = '2000/1100' WHERE company_id = $1 AND account_number = '1100'")
        .bind(company)
        .bind(first.account_ids["2000"])
        .execute(&pool)
        .await
        .unwrap();

    svc.install(company, "TEST_CHART").await.unwrap();

    let row = one(&pool, company, "1100").await;
    // rename kept
    assert_eq!(row.get::<&str, _>("name"), "Kas Kecil");
    // structure reverted to the dataset's truth
    assert_eq!(row.get::<Option<Uuid>, _>("parent_id"), Some(first.account_ids["1000"]));
    assert_eq!(row.get::<Option<&str>, _>("path"), Some("1000/1100"));
}

#[tokio::test]
async fn reinstall_resurrects_soft_deleted() {
    let pool = pool().await;
    let company = Uuid::new_v4();
    let svc = service(&pool);
    svc.install(company, "TEST_CHART").await.unwrap();

    sqlx::query(
        "UPDATE accounting.accounts \
         SET metadata = jsonb_set(metadata, '{deleted_at}', to_jsonb(NOW())) \
         WHERE company_id = $1 AND account_number = '1100'",
    )
    .bind(company)
    .execute(&pool)
    .await
    .unwrap();

    let third = svc.install(company, "TEST_CHART").await.unwrap();
    assert_eq!(third.accounts_resurrected, 1);
    assert_eq!(third.accounts_updated, 6);

    let row = one(&pool, company, "1100").await;
    assert_eq!(row.get::<Option<&str>, _>("deleted_at"), None::<&str>);
}

#[tokio::test]
async fn refuses_when_journal_lines_exist() {
    let pool = pool().await;
    let company = Uuid::new_v4();
    let svc = service(&pool);
    let report = svc.install(company, "TEST_CHART").await.unwrap();

    // One posted line is enough to lock the books.
    let journal = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO accounting.journals (id, company_id, journal_number, transaction_date, description) \
         VALUES ($1, $2, $3, CURRENT_DATE, 'test')",
    )
    .bind(journal)
    .bind(company)
    .bind(format!("JV-{}", &journal.to_string()[..8]))
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO accounting.journal_lines (journal_id, company_id, line_number, account_id, account_number, account_name) \
         VALUES ($1, $2, 1, $3, '1100', 'Kas')",
    )
    .bind(journal)
    .bind(company)
    .bind(report.account_ids["1100"])
    .execute(&pool)
    .await
    .unwrap();

    let err = svc.install(company, "TEST_CHART").await.unwrap_err();
    match err {
        ChartInstallError::ChartHasPostings(code, c) => {
            assert_eq!(code, "TEST_CHART");
            assert_eq!(c, company);
        }
        other => panic!("expected ChartHasPostings, got: {other}"),
    }
}

#[tokio::test]
async fn refuses_overlap_with_manual_account() {
    let pool = pool().await;
    let company = Uuid::new_v4();
    // A manually created account squats on a number the chart uses.
    sqlx::query(
        "INSERT INTO accounting.accounts (company_id, account_number, account_code, name, account_type, account_subtype, normal_balance) \
         VALUES ($1, '1100', 'KAS', 'Kas manual', 'asset', 'cash', 'debit')",
    )
    .bind(company)
    .execute(&pool)
    .await
    .unwrap();

    let err = service(&pool).install(company, "TEST_CHART").await.unwrap_err();
    match err {
        ChartInstallError::AccountNumberConflict(chart, refs) => {
            assert_eq!(chart, "TEST_CHART");
            assert_eq!(refs, vec![("1100".to_string(), "KAS".to_string())]);
        }
        other => panic!("expected AccountNumberConflict, got: {other}"),
    }
    // nothing was written
    assert_eq!(count_accounts(&pool, company).await, 1);
}

#[tokio::test]
async fn deterministic_ids_stable_across_runs_and_scoped_per_company() {
    let pool = pool().await;
    let company = Uuid::new_v4();
    let svc = service(&pool);

    let first = svc.install(company, "TEST_CHART").await.unwrap();
    let second = svc.install(company, "TEST_CHART").await.unwrap();
    assert_eq!(first.account_ids, second.account_ids);

    // Another company's install of the same chart produces different ids.
    let other_company = Uuid::new_v4();
    let other = svc.install(other_company, "TEST_CHART").await.unwrap();
    assert_ne!(first.account_ids["1100"], other.account_ids["1100"]);
}

#[tokio::test]
async fn unknown_chart_is_named() {
    let pool = pool().await;
    let err = service(&pool)
        .install(Uuid::new_v4(), "NOPE")
        .await
        .unwrap_err();
    match err {
        ChartInstallError::UnknownChart(code, registered) => {
            assert_eq!(code, "NOPE");
            assert_eq!(registered, vec!["TEST_CHART".to_string()]);
        }
        other => panic!("expected UnknownChart, got: {other}"),
    }
}

#[test]
fn dataset_validation_contract() {
    // The pure validation rules also hold for the fixture used above.
    assert!(validate_dataset(&chart()).is_ok());
}

#[test]
fn validation_rejects_code_number_mismatch() {
    let mut ds = chart();
    ds.accounts[1].code = "9999".into(); // number stays 1100
    assert!(matches!(
        validate_dataset(&ds),
        Err(backbone_accounting::domain::chart_dataset::DatasetError::CodeNumberMismatch(_, _, _))
    ));
}

#[test]
fn validation_rejects_empty_name_bad_currency_duplicate_sort() {
    let mut ds = chart();
    ds.accounts[1].name = "   ".into();
    assert!(matches!(
        validate_dataset(&ds),
        Err(backbone_accounting::domain::chart_dataset::DatasetError::EmptyName(_))
    ));

    let mut ds = chart();
    ds.accounts[1].currency = "idr".into();
    assert!(matches!(
        validate_dataset(&ds),
        Err(backbone_accounting::domain::chart_dataset::DatasetError::InvalidCurrency(_, _))
    ));

    let mut ds = chart();
    ds.accounts[2].sort_order = ds.accounts[1].sort_order;
    assert!(matches!(
        validate_dataset(&ds),
        Err(backbone_accounting::domain::chart_dataset::DatasetError::DuplicateSortOrder(_))
    ));
}

/// The version contract: a renumbered account is a NEW identity — the install
/// succeeds, but the old row lingers as a live, chart-stamped posting target
/// (documented posture; cleanup is a manual archival step until a deprecation
/// sweep exists). Dropping an account behaves the same way.
#[tokio::test]
async fn renumber_installs_new_identity_and_dropped_codes_linger() {
    let pool = pool().await;
    let company = Uuid::new_v4();
    let svc = service(&pool);
    let first = svc.install(company, "TEST_CHART").await.unwrap();

    // v2: renumber 1100 Kas -> 1150 (new identity), drop 2110 PPN Keluaran entirely.
    let mut v2 = chart();
    v2.version = "2.0".into();
    let kas = v2.accounts.iter_mut().find(|a| a.number == "1100").unwrap();
    kas.number = "1150".into();
    kas.code = "1150".into();
    v2.accounts.retain(|a| a.number != "2110");

    let svc2 = ChartInstallService::new(
        Arc::new(SqlxChartInstallRepository::new()),
        pool.clone(),
        vec![Arc::new(v2)],
    );
    let second = svc2.install(company, "TEST_CHART").await.unwrap();

    // v2 installs one new row (1150); the five surviving codes update in place.
    assert_eq!(second.accounts_installed, 1);
    assert_eq!(second.accounts_updated, 5);

    // The renumbered-away row and the dropped row are STILL fully live —
    // active, chart-stamped, and posting targets. This is the pinned posture.
    let old_kas = one(&pool, company, "1100").await;
    assert_eq!(old_kas.get::<Option<&str>, _>("deleted_at"), None::<&str>);
    assert_eq!(old_kas.get::<Option<&str>, _>("chart_code"), Some("TEST_CHART"));
    assert_eq!(old_kas.get::<Uuid, _>("id"), first.account_ids["1100"]);
    let dropped = one(&pool, company, "2110").await;
    assert_eq!(dropped.get::<Option<&str>, _>("deleted_at"), None::<&str>);
    assert_eq!(count_accounts(&pool, company).await, 8); // 7 v1 rows + the new 1150
}
