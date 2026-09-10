//! Chart install engine cases — the install/refuse/idempotency contract, against a
//! real Postgres. Requires DATABASE_URL (defaults to the local dev Postgres on :5433).
//!
//! Tenancy: the module ships NONE (ADR-0029) — request shapes keep the legacy company
//! twin but no table carries a tenant column, and an undecorated database has no fence.
//! Each test therefore isolates itself with a unique account-number tag (the overlap
//! gate and the verification queries key on numbers, not tenants), and the probes run
//! under one lock and wipe the accounting tables first: the postings gate reads the
//! whole table on an undecorated database (the decorator's fence scopes it per unit in
//! production), so rows left behind by any other suite would refuse every install.

use backbone_accounting::application::service::chart_install_service::{
    ChartInstallError, ChartInstallService,
};
use backbone_accounting::domain::chart_dataset::{validate_dataset, ChartAccountDef, ChartDataset};
use backbone_accounting::domain::entity::{AccountSubtype, AccountType, NormalBalance};
use backbone_accounting::infrastructure::persistence::chart_install_repository::SqlxChartInstallRepository;
use sqlx::{PgPool, Row};
use std::sync::Arc;
use uuid::Uuid;

/// Serializes the database-touching probes in this file (see the header note).
static DB_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

async fn pool() -> PgPool {
    let url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgresql://postgres:postgres@localhost:5433/backbone_accounting".to_string()
    });
    PgPool::connect(&url).await.expect("connect DB")
}

/// Reset the accounting tables to an empty single-tenant database before each probe.
/// The install gate reads the postings table globally on an undecorated database, so
/// rows left behind by any other suite (or an earlier test in this file) would refuse
/// every install. Circular FK pairs (journal_lines.ledger_id <-> ledgers.journal_line_id,
/// and every self-referential link) are severed first; the deletes then run
/// children-first.
async fn wipe(pool: &PgPool) {
    for sql in [
        "UPDATE accounting.journal_lines SET ledger_id=NULL, reconciliation_id=NULL, full_reconcile_id=NULL, related_line_id=NULL",
        "UPDATE accounting.ledgers SET reverses_id=NULL, reversed_by_id=NULL, reconciliation_id=NULL",
        "UPDATE accounting.journals SET reverses_id=NULL, reversed_by_id=NULL",
        "UPDATE accounting.accounting_posts SET reverses_post_id=NULL, reversed_by_post_id=NULL",
        "UPDATE accounting.accounts SET parent_id=NULL, source_id=NULL",
        "UPDATE accounting.fiscal_periods SET parent_id=NULL",
        "UPDATE accounting.reconciliations SET previous_reconciliation_id=NULL",
        "UPDATE accounting.reconciliation_items SET matched_with_id=NULL",
        "DELETE FROM accounting.reconciliation_items",
        "DELETE FROM accounting.partial_reconciles",
        "DELETE FROM accounting.reconciliations",
        "DELETE FROM accounting.ledgers",
        "DELETE FROM accounting.journal_lines",
        "DELETE FROM accounting.full_reconciles",
        "DELETE FROM accounting.accounting_posts",
        "DELETE FROM accounting.journals",
        "DELETE FROM accounting.accounts",
        "DELETE FROM accounting.fiscal_periods",
    ] {
        sqlx::query(sql).execute(pool).await.expect("wipe");
    }
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
/// `tag` prefixes every account number so concurrent probes (which share this
/// undecorated database, where no tenancy fence separates them) never collide on the
/// overlap gate or the number-keyed verification queries.
fn chart(tag: &str) -> ChartDataset {
    let n = |number: &str| format!("{tag}{number}");
    ChartDataset {
        code: format!("TEST_CHART_{tag}"),
        version: "1.0".into(),
        name: "Test chart".into(),
        accounts: vec![
            def(
                &n("1000"),
                "Aset",
                AccountType::Asset,
                AccountSubtype::CurrentAsset,
                NormalBalance::Debit,
                None,
                false,
                1,
            ),
            def(
                &n("1100"),
                "Kas",
                AccountType::Asset,
                AccountSubtype::Cash,
                NormalBalance::Debit,
                Some(&n("1000")),
                true,
                2,
            ),
            def(
                &n("1110"),
                "Bank",
                AccountType::Asset,
                AccountSubtype::Bank,
                NormalBalance::Debit,
                Some(&n("1000")),
                true,
                3,
            ),
            def(
                &n("1200"),
                "Piutang Usaha",
                AccountType::Asset,
                AccountSubtype::AccountsReceivable,
                NormalBalance::Debit,
                Some(&n("1000")),
                true,
                4,
            ),
            def(
                &n("2000"),
                "Liabilitas",
                AccountType::Liability,
                AccountSubtype::CurrentLiability,
                NormalBalance::Credit,
                None,
                false,
                5,
            ),
            def(
                &n("2100"),
                "Utang Usaha",
                AccountType::Liability,
                AccountSubtype::AccountsPayable,
                NormalBalance::Credit,
                Some(&n("2000")),
                true,
                6,
            ),
            def(
                &n("2110"),
                "PPN Keluaran",
                AccountType::Liability,
                AccountSubtype::Tax,
                NormalBalance::Credit,
                Some(&n("2000")),
                false,
                7,
            ),
        ],
    }
}

fn service(pool: &PgPool, tag: &str) -> ChartInstallService {
    ChartInstallService::new(
        Arc::new(SqlxChartInstallRepository::new()),
        pool.clone(),
        vec![Arc::new(chart(tag))],
    )
}

async fn count_accounts(pool: &PgPool, chart_code: &str) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM accounting.accounts WHERE chart_code = $1")
        .bind(chart_code)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn one(pool: &PgPool, chart_code: &str, number: &str) -> sqlx::postgres::PgRow {
    sqlx::query(
        r#"SELECT id, account_number, parent_id, level, path, is_header, is_detail,
                  chart_code, chart_version, name, metadata->>'deleted_at' AS deleted_at
             FROM accounting.accounts
            WHERE chart_code = $1 AND account_number = $2"#,
    )
    .bind(chart_code)
    .bind(number)
    .fetch_one(pool)
    .await
    .unwrap()
}

#[tokio::test]
async fn install_on_fresh_company_creates_full_tree() {
    let _guard = DB_LOCK.lock().await;
    let pool = pool().await;
    wipe(&pool).await;
    let company = Uuid::new_v4();
    let tag = &company.simple().to_string()[..8];
    let code = format!("TEST_CHART_{tag}");
    let report = service(&pool, tag).install(company, &code).await.unwrap();

    assert_eq!(report.accounts_installed, 7);
    assert_eq!(report.accounts_updated, 0);
    assert_eq!(report.account_ids.len(), 7);

    let root = one(&pool, &code, &format!("{tag}1000")).await;
    assert_eq!(root.get::<i32, _>("level"), 0);
    assert_eq!(root.get::<Option<&str>, _>("path"), Some(format!("{tag}1000").as_str()));
    assert!(root.get::<bool, _>("is_header"));
    assert!(!root.get::<bool, _>("is_detail"));
    assert_eq!(
        root.get::<Option<&str>, _>("chart_code"),
        Some(code.as_str())
    );
    assert_eq!(root.get::<Option<&str>, _>("chart_version"), Some("1.0"));

    let leaf = one(&pool, &code, &format!("{tag}1100")).await;
    assert_eq!(leaf.get::<i32, _>("level"), 1);
    assert_eq!(
        leaf.get::<Option<&str>, _>("path"),
        Some(format!("{tag}1000/{tag}1100").as_str())
    );
    assert!(!leaf.get::<bool, _>("is_header"));
    assert!(leaf.get::<bool, _>("is_detail"));
    // deterministic id map matches the stored row
    assert_eq!(leaf.get::<Uuid, _>("id"), report.account_ids[&format!("{tag}1100")]);
    // parent linkage follows the deterministic ids
    assert_eq!(
        leaf.get::<Option<Uuid>, _>("parent_id"),
        Some(report.account_ids[&format!("{tag}1000")])
    );
}

#[tokio::test]
async fn reinstall_updates_not_duplicates() {
    let _guard = DB_LOCK.lock().await;
    let pool = pool().await;
    wipe(&pool).await;
    let company = Uuid::new_v4();
    let tag = &company.simple().to_string()[..8];
    let code = format!("TEST_CHART_{tag}");
    let svc = service(&pool, tag);
    svc.install(company, &code).await.unwrap();
    let second = svc.install(company, &code).await.unwrap();

    assert_eq!(second.accounts_installed, 0);
    assert_eq!(second.accounts_updated, 7);
    assert_eq!(count_accounts(&pool, &code).await, 7);
}

#[tokio::test]
async fn manager_rename_survives_reinstall_and_reparent_reverts() {
    let _guard = DB_LOCK.lock().await;
    let pool = pool().await;
    wipe(&pool).await;
    let company = Uuid::new_v4();
    let tag = &company.simple().to_string()[..8];
    let code = format!("TEST_CHART_{tag}");
    let kas = format!("{tag}1100");
    let svc = service(&pool, tag);
    let first = svc.install(company, &code).await.unwrap();

    // Manager edits post-install: a rename (user-owned) and a re-parent (engine-owned).
    sqlx::query("UPDATE accounting.accounts SET name = 'Kas Kecil' WHERE chart_code = $1 AND account_number = $2")
        .bind(&code)
        .bind(&kas)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE accounting.accounts SET parent_id = $2, path = $3 WHERE chart_code = $1 AND account_number = $4")
        .bind(&code)
        .bind(first.account_ids[&format!("{tag}2000")])
        .bind(format!("{tag}2000/{kas}"))
        .bind(&kas)
        .execute(&pool)
        .await
        .unwrap();

    svc.install(company, &code).await.unwrap();

    let row = one(&pool, &code, &kas).await;
    // rename kept
    assert_eq!(row.get::<&str, _>("name"), "Kas Kecil");
    // structure reverted to the dataset's truth
    assert_eq!(
        row.get::<Option<Uuid>, _>("parent_id"),
        Some(first.account_ids[&format!("{tag}1000")])
    );
    assert_eq!(
        row.get::<Option<&str>, _>("path"),
        Some(format!("{tag}1000/{kas}").as_str())
    );
}

#[tokio::test]
async fn reinstall_resurrects_soft_deleted() {
    let _guard = DB_LOCK.lock().await;
    let pool = pool().await;
    wipe(&pool).await;
    let company = Uuid::new_v4();
    let tag = &company.simple().to_string()[..8];
    let code = format!("TEST_CHART_{tag}");
    let svc = service(&pool, tag);
    svc.install(company, &code).await.unwrap();

    sqlx::query(
        "UPDATE accounting.accounts \
         SET metadata = jsonb_set(metadata, '{deleted_at}', to_jsonb(NOW())) \
         WHERE chart_code = $1 AND account_number = $2",
    )
    .bind(&code)
    .bind(format!("{tag}1100"))
    .execute(&pool)
    .await
    .unwrap();

    let third = svc.install(company, &code).await.unwrap();
    assert_eq!(third.accounts_resurrected, 1);
    assert_eq!(third.accounts_updated, 6);

    let row = one(&pool, &code, &format!("{tag}1100")).await;
    assert_eq!(row.get::<Option<&str>, _>("deleted_at"), None::<&str>);
}

#[tokio::test]
async fn refuses_when_journal_lines_exist() {
    let _guard = DB_LOCK.lock().await;
    let pool = pool().await;
    wipe(&pool).await;
    let company = Uuid::new_v4();
    let tag = &company.simple().to_string()[..8];
    let code = format!("TEST_CHART_{tag}");
    let svc = service(&pool, tag);
    let report = svc.install(company, &code).await.unwrap();

    // One posted line is enough to lock the books (the postings gate reads the
    // whole table; the decorator's fence scopes it per unit in production).
    let journal = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO accounting.journals (id, journal_number, transaction_date, description) \
         VALUES ($1, $2, CURRENT_DATE, 'test')",
    )
    .bind(journal)
    .bind(format!("JV-{}", &journal.to_string()[..8]))
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO accounting.journal_lines (journal_id, line_number, account_id, account_number, account_name) \
         VALUES ($1, 1, $2, $3, 'Kas')",
    )
    .bind(journal)
    .bind(report.account_ids[&format!("{tag}1100")])
    .bind(format!("{tag}1100"))
    .execute(&pool)
    .await
    .unwrap();

    let err = svc.install(company, &code).await.unwrap_err();
    match err {
        ChartInstallError::ChartHasPostings(code_name, c) => {
            assert_eq!(code_name, code);
            assert_eq!(c, company);
        }
        other => panic!("expected ChartHasPostings, got: {other}"),
    }

    // Shed the lines again: the postings gate is table-global on an undecorated
    // database, so leftover rows would refuse every later install probe.
    sqlx::query("DELETE FROM accounting.journal_lines WHERE journal_id = $1")
        .bind(journal)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM accounting.journals WHERE id = $1")
        .bind(journal)
        .execute(&pool)
        .await
        .unwrap();
}

#[tokio::test]
async fn refuses_overlap_with_manual_account() {
    let _guard = DB_LOCK.lock().await;
    let pool = pool().await;
    wipe(&pool).await;
    let company = Uuid::new_v4();
    let tag = &company.simple().to_string()[..8];
    let code = format!("TEST_CHART_{tag}");
    let squat_number = format!("{tag}1100");
    // A manually created account squats on a number the chart uses.
    sqlx::query(
        "INSERT INTO accounting.accounts (account_number, account_code, name, account_type, account_subtype, normal_balance) \
         VALUES ($1, 'KAS', 'Kas manual', 'asset', 'cash', 'debit')",
    )
    .bind(&squat_number)
    .execute(&pool)
    .await
    .unwrap();

    let err = service(&pool, tag)
        .install(company, &code)
        .await
        .unwrap_err();
    match err {
        ChartInstallError::AccountNumberConflict(chart_name, refs) => {
            assert_eq!(chart_name, code);
            assert_eq!(refs, vec![(squat_number.clone(), "KAS".to_string())]);
        }
        other => panic!("expected AccountNumberConflict, got: {other}"),
    }
    // nothing was written
    assert_eq!(count_accounts(&pool, &code).await, 0);

    // Shed the manual squatter so later probes don't inherit the conflict.
    sqlx::query("DELETE FROM accounting.accounts WHERE account_number = $1")
        .bind(&squat_number)
        .execute(&pool)
        .await
        .unwrap();
}

#[tokio::test]
async fn deterministic_ids_stable_across_runs_and_scoped_per_company() {
    let _guard = DB_LOCK.lock().await;
    let pool = pool().await;
    wipe(&pool).await;
    let company = Uuid::new_v4();
    let tag = &company.simple().to_string()[..8];
    let code = format!("TEST_CHART_{tag}");
    let svc = service(&pool, tag);

    let first = svc.install(company, &code).await.unwrap();
    let second = svc.install(company, &code).await.unwrap();
    assert_eq!(first.account_ids, second.account_ids);

    // The id derivation mixes the caller's legacy company twin into the hash, so
    // another company's install of the same chart derives DIFFERENT ids — provable
    // purely, since on an undecorated database a second same-numbered install would
    // (correctly) conflict on the overlap gate instead of coexisting.
    let other_company = Uuid::new_v4();
    let kas = format!("{tag}1100");
    let other_kas_id = Uuid::new_v5(
        &Uuid::NAMESPACE_URL,
        format!("account:{other_company}:{code}:{kas}").as_bytes(),
    );
    assert_ne!(first.account_ids[&kas], other_kas_id);
}

#[tokio::test]
async fn unknown_chart_is_named() {
    let _guard = DB_LOCK.lock().await;
    let pool = pool().await;
    wipe(&pool).await;
    let err = service(&pool, "zz")
        .install(Uuid::new_v4(), "NOPE")
        .await
        .unwrap_err();
    match err {
        ChartInstallError::UnknownChart(code, registered) => {
            assert_eq!(code, "NOPE");
            assert_eq!(registered, vec!["TEST_CHART_zz".to_string()]);
        }
        other => panic!("expected UnknownChart, got: {other}"),
    }
}

#[test]
fn dataset_validation_contract() {
    // The pure validation rules also hold for the fixture used above.
    assert!(validate_dataset(&chart("vt")).is_ok());
}

#[test]
fn validation_rejects_code_number_mismatch() {
    let mut ds = chart("vt");
    ds.accounts[1].code = "9999".into(); // number stays tagged 1100
    assert!(matches!(
        validate_dataset(&ds),
        Err(backbone_accounting::domain::chart_dataset::DatasetError::CodeNumberMismatch(_, _, _))
    ));
}

#[test]
fn validation_rejects_empty_name_bad_currency_duplicate_sort() {
    let mut ds = chart("vt");
    ds.accounts[1].name = "   ".into();
    assert!(matches!(
        validate_dataset(&ds),
        Err(backbone_accounting::domain::chart_dataset::DatasetError::EmptyName(_))
    ));

    let mut ds = chart("vt");
    ds.accounts[1].currency = "idr".into();
    assert!(matches!(
        validate_dataset(&ds),
        Err(backbone_accounting::domain::chart_dataset::DatasetError::InvalidCurrency(_, _))
    ));

    let mut ds = chart("vt");
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
    let _guard = DB_LOCK.lock().await;
    let pool = pool().await;
    wipe(&pool).await;
    let company = Uuid::new_v4();
    let tag = &company.simple().to_string()[..8];
    let code = format!("TEST_CHART_{tag}");
    let svc = service(&pool, tag);
    let first = svc.install(company, &code).await.unwrap();

    // v2: renumber tagged-1100 Kas -> 1150 (new identity), drop 2110 PPN Keluaran entirely.
    let mut v2 = chart(tag);
    v2.version = "2.0".into();
    let old_kas_number = format!("{tag}1100");
    let new_kas_number = format!("{tag}1150");
    let dropped_number = format!("{tag}2110");
    let kas = v2.accounts.iter_mut().find(|a| a.number == old_kas_number).unwrap();
    kas.number = new_kas_number.clone();
    kas.code = new_kas_number.clone();
    v2.accounts.retain(|a| a.number != dropped_number);

    let svc2 = ChartInstallService::new(
        Arc::new(SqlxChartInstallRepository::new()),
        pool.clone(),
        vec![Arc::new(v2)],
    );
    let second = svc2.install(company, &code).await.unwrap();

    // v2 installs one new row (1150); the five surviving codes update in place.
    assert_eq!(second.accounts_installed, 1);
    assert_eq!(second.accounts_updated, 5);

    // The renumbered-away row and the dropped row are STILL fully live —
    // active, chart-stamped, and posting targets. This is the pinned posture.
    let old_kas = one(&pool, &code, &old_kas_number).await;
    assert_eq!(old_kas.get::<Option<&str>, _>("deleted_at"), None::<&str>);
    assert_eq!(
        old_kas.get::<Option<&str>, _>("chart_code"),
        Some(code.as_str())
    );
    assert_eq!(old_kas.get::<Uuid, _>("id"), first.account_ids[&old_kas_number]);
    let dropped = one(&pool, &code, &dropped_number).await;
    assert_eq!(dropped.get::<Option<&str>, _>("deleted_at"), None::<&str>);
    assert_eq!(count_accounts(&pool, &code).await, 8); // 7 v1 rows + the new 1150
}
