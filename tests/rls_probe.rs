//! RLS host-contract probe (ADR-0011).
//!
//! Proves the tenant fence works WHEN the host honors its contract: connect as a non-superuser,
//! non-BYPASSRLS role and set `app.company_id` per request. Under those conditions a write whose
//! `company_id` does NOT match the session tenant is rejected (WITH CHECK violation), and a
//! matching write succeeds. If this test ever fails, either RLS was disabled or the role can
//! bypass it — i.e. the contract documented in ADR-0011 is broken.
//!
//! Requires DATABASE_URL (defaults to local dev Postgres on :5433) backed by a superuser-capable
//! role so it can mint/teardown the restricted role.

use std::sync::Arc;

use sqlx::{PgPool, Row};
use uuid::Uuid;

use backbone_accounting::application::service::reconcile_write_service::ReconcileWriteService;
use backbone_accounting::domain::repositories::reporting_repository::ReportingRepository;
use backbone_accounting::infrastructure::persistence::{
    SqlxPostingRepository, SqlxReconcileGraphRepository,
};

const ROLE: &str = "bbacc_rls_probe";
const PWD: &str = "probe";

/// Role/catalog DDL serializes — two tests minting roles concurrently hit
/// "tuple concurrently updated" in the system catalogs.
static ROLE_DDL_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

async fn admin() -> PgPool {
    let url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgresql://postgres:postgres@localhost:5433/backbone_accounting".to_string()
    });
    PgPool::connect(&url).await.expect("connect admin")
}

/// Shed the role's grants, then drop it. Leftover grants (from a run whose teardown never
/// reached the drop, or whose drop was swallowed) make plain DROP ROLE fail with 2BP01 —
/// DROP OWNED BY first keeps both bootstrap and teardown idempotent across runs.
async fn drop_role(admin: &PgPool) {
    let _ = sqlx::query(&format!("DROP OWNED BY {ROLE}"))
        .execute(admin)
        .await;
    let _ = sqlx::query(&format!("DROP ROLE IF EXISTS {ROLE}"))
        .execute(admin)
        .await;
}

async fn bootstrap_role(admin: &PgPool) {
    // Restricted role: NOSUPERUSER, NOBYPASSRLS — the posture ADR-0011 demands of the host.
    drop_role(admin).await;
    for stmt in [
        format!("CREATE ROLE {ROLE} LOGIN PASSWORD '{PWD}' NOSUPERUSER NOBYPASSRLS"),
        format!("GRANT USAGE ON SCHEMA accounting TO {ROLE}"),
        format!("GRANT USAGE ON SCHEMA public TO {ROLE}"),
        format!("GRANT INSERT ON accounting.accounts TO {ROLE}"),
    ] {
        sqlx::query(&stmt).execute(admin).await.unwrap();
    }
}

async fn restricted() -> PgPool {
    let url = format!("postgresql://{ROLE}:{PWD}@localhost:5433/backbone_accounting");
    PgPool::connect(&url)
        .await
        .expect("connect restricted role")
}

async fn teardown_role(admin: &PgPool) {
    drop_role(admin).await;
}

/// Insert a minimal accounts row for `company`. Returns the account id (or errors under RLS).
async fn try_insert(
    pool: &PgPool,
    app_company: Uuid,
    row_company: Uuid,
) -> Result<Uuid, sqlx::Error> {
    let mut tx = pool.begin().await?;
    sqlx::query("SELECT set_config('app.company_id', $1, true)")
        .bind(app_company.to_string())
        .execute(&mut *tx)
        .await?;
    let id = Uuid::new_v4();
    let res = sqlx::query(
        r#"INSERT INTO accounting.accounts
            (id, company_id, account_number, account_code, name, account_type, account_subtype,
             normal_balance, is_detail, is_header, status)
           VALUES ($1,$2,$3,$3,$4,'asset'::account_type,'cash'::account_subtype,
                   'debit'::normal_balance, TRUE, FALSE, 'active'::account_status)"#,
    )
    .bind(id)
    .bind(row_company)
    .bind("RLS")
    .bind("RLS probe")
    .execute(&mut *tx)
    .await;
    match res {
        Ok(_) => {
            tx.commit().await?;
            Ok(id)
        }
        Err(e) => {
            let _ = tx.rollback().await;
            Err(e)
        }
    }
}

async fn count_for(admin: &PgPool, company: Uuid) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM accounting.accounts WHERE company_id=$1")
        .bind(company)
        .fetch_one(admin)
        .await
        .unwrap()
}

#[tokio::test]
async fn rls_rejects_mismatched_tenant_write() {
    let _ddl = ROLE_DDL_LOCK.lock().await;
    let admin = admin().await;
    bootstrap_role(&admin).await;
    let restricted = restricted().await;

    let tenant_a = Uuid::new_v4();
    let tenant_b = Uuid::new_v4();

    // Mismatched: session = A, row = B → must be rejected by the RLS WITH CHECK predicate.
    let err = try_insert(&restricted, tenant_a, tenant_b).await;
    assert!(
        err.is_err(),
        "RLS must reject a write to a non-session tenant"
    );
    assert_eq!(
        count_for(&admin, tenant_b).await,
        0,
        "no row should have landed for tenant B"
    );

    // Matching: session = A, row = A → succeeds.
    let id = try_insert(&restricted, tenant_a, tenant_a)
        .await
        .expect("matching write succeeds");
    let got: Uuid = sqlx::query("SELECT company_id FROM accounting.accounts WHERE id=$1")
        .bind(id)
        .fetch_one(&admin)
        .await
        .unwrap()
        .get::<Uuid, _>("company_id");
    assert_eq!(got, tenant_a);

    // Sanity: a non-superuser role with RLS honors the contract — A's write landed, B's did not.
    assert_eq!(count_for(&admin, tenant_a).await, 1);

    teardown_role(&admin).await;
}

/// The reconciliation-graph tables carry the same fence: a partial edge written under a
/// session tenant other than the row's company must bounce, a matching one lands. The
/// journal lines the edge references are seeded by the admin (the probe exercises the
/// graph tables' fence, not the lines').
#[tokio::test]
async fn rls_fences_reconciliation_graph_tables() {
    let _ddl = ROLE_DDL_LOCK.lock().await;
    let graph_role = "bbacc_rls_graph_probe";
    let admin = admin().await;
    let _ = sqlx::query(&format!("DROP OWNED BY {graph_role}"))
        .execute(&admin)
        .await;
    let _ = sqlx::query(&format!("DROP ROLE IF EXISTS {graph_role}"))
        .execute(&admin)
        .await;
    for stmt in [
        format!("CREATE ROLE {graph_role} LOGIN PASSWORD '{PWD}' NOSUPERUSER NOBYPASSRLS"),
        format!("GRANT USAGE ON SCHEMA accounting TO {graph_role}"),
        format!("GRANT INSERT ON accounting.partial_reconciles TO {graph_role}"),
        format!("GRANT INSERT ON accounting.full_reconciles TO {graph_role}"),
    ] {
        sqlx::query(&stmt).execute(&admin).await.unwrap();
    }
    let restricted = PgPool::connect(&format!(
        "postgresql://{graph_role}:{PWD}@localhost:5433/backbone_accounting"
    ))
    .await
    .expect("connect graph probe role");

    // Two tenant-scoped line pairs, seeded as the owner.
    let seed_lines = |company: Uuid| {
        let admin = admin.clone();
        async move {
            let account = Uuid::new_v4();
            sqlx::query(
                r#"INSERT INTO accounting.accounts
                     (id, company_id, account_number, account_code, name, account_type,
                      account_subtype, normal_balance, status)
                   VALUES ($1,$2,$3,$3,'rls probe','asset'::account_type,
                           'accounts_receivable'::account_subtype,'debit'::normal_balance,
                           'active'::account_status)"#,
            )
            .bind(account)
            .bind(company)
            .bind(format!("RLS-{account}"))
            .execute(&admin)
            .await
            .unwrap();
            let journal = Uuid::new_v4();
            sqlx::query(
                r#"INSERT INTO accounting.journals
                     (id, company_id, journal_number, journal_type, source, transaction_date,
                      description, currency, status)
                   VALUES ($1,$2,$3,'general'::journal_type,'manual'::journal_source,'2026-06-15',
                           'rls probe','IDR','posted'::journal_status)"#,
            )
            .bind(journal)
            .bind(company)
            .bind(format!("RLS-{journal}"))
            .execute(&admin)
            .await
            .unwrap();
            let mut ids = Vec::new();
            for n in 1..=2 {
                let id = Uuid::new_v4();
                sqlx::query(
                    r#"INSERT INTO accounting.journal_lines
                         (id, journal_id, company_id, line_number, account_id, account_number,
                          account_name, debit_amount, credit_amount, base_debit_amount,
                          base_credit_amount, is_posted)
                       VALUES ($1,$2,$3,$4,$5,'RLS','rls probe',100,0,100,0,TRUE)"#,
                )
                .bind(id)
                .bind(journal)
                .bind(company)
                .bind(n)
                .bind(account)
                .execute(&admin)
                .await
                .unwrap();
                ids.push(id);
            }
            ids
        }
    };
    let tenant_a = Uuid::new_v4();
    let tenant_b = Uuid::new_v4();
    let lines_a = seed_lines(tenant_a).await;

    let try_edge = |pool: PgPool, app_company: Uuid, row_company: Uuid, lines: Vec<Uuid>| async move {
        let mut tx = pool.begin().await.unwrap();
        sqlx::query("SELECT set_config('app.company_id', $1, true)")
            .bind(app_company.to_string())
            .execute(&mut *tx)
            .await
            .unwrap();
        let res = sqlx::query(
            r#"INSERT INTO accounting.partial_reconciles
                 (company_id, debit_move_id, credit_move_id, amount, currency, max_date,
                  origin, updated_at)
               VALUES ($1,$2,$3,10,'IDR','2026-06-15','manual'::reconcile_origin,NOW())"#,
        )
        .bind(row_company)
        .bind(lines[0])
        .bind(lines[1])
        .execute(&mut *tx)
        .await;
        match res {
            Ok(_) => {
                tx.commit().await.unwrap();
                Ok(())
            }
            Err(e) => {
                let _ = tx.rollback().await;
                Err(e)
            }
        }
    };

    // Session = B, row = A → the fence must reject the cross-tenant edge.
    let err = try_edge(restricted.clone(), tenant_b, tenant_a, lines_a.clone()).await;
    assert!(
        err.is_err(),
        "RLS must reject a graph edge into another tenant"
    );

    // Session = A, row = A → lands.
    try_edge(restricted.clone(), tenant_a, tenant_a, lines_a.clone())
        .await
        .expect("matching graph edge succeeds");

    let n: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM accounting.partial_reconciles WHERE company_id=$1 \
         AND debit_move_id=$2 AND credit_move_id=$3",
    )
    .bind(tenant_a)
    .bind(lines_a[0])
    .bind(lines_a[1])
    .fetch_one(&admin)
    .await
    .unwrap();
    assert_eq!(n, 1, "exactly the matching edge landed");

    let _ = tenant_b; // seeded only if a future probe needs B-side lines
    let _ = sqlx::query(&format!("DROP OWNED BY {graph_role}"))
        .execute(&admin)
        .await;
    let _ = sqlx::query(&format!("DROP ROLE IF EXISTS {graph_role}"))
        .execute(&admin)
        .await;
}

/// The pool-verb matching-group read must survive the fence on a RESTRICTED pool. The bind is
/// `set_config(..., is_local=true)` — transaction-scoped — so a read wrapper that binds on a bare
/// pooled connection (no transaction) loses the setting before its first statement and the fence
/// silently empties the result. That shape returns 200 with an empty group on any fenced
/// app-role deployment while looking perfectly green on owner/superuser DSNs. This probe builds
/// the service over a restricted pool with real graph rows and asserts the read finds them.
#[tokio::test]
async fn matching_group_read_survives_the_fence_on_a_restricted_pool() {
    let _ddl = ROLE_DDL_LOCK.lock().await;
    let role = "bbacc_rls_read_probe";
    let admin = admin().await;
    let _ = sqlx::query(&format!("DROP OWNED BY {role}"))
        .execute(&admin)
        .await;
    let _ = sqlx::query(&format!("DROP ROLE IF EXISTS {role}"))
        .execute(&admin)
        .await;
    for stmt in [
        format!("CREATE ROLE {role} LOGIN PASSWORD '{PWD}' NOSUPERUSER NOBYPASSRLS"),
        format!("GRANT USAGE ON SCHEMA accounting TO {role}"),
        format!(
            "GRANT SELECT ON accounting.journals, accounting.journal_lines, \
             accounting.partial_reconciles, accounting.full_reconciles, accounting.accounts TO {role}"
        ),
    ] {
        sqlx::query(&stmt).execute(&admin).await.unwrap();
    }
    let restricted = PgPool::connect(&format!(
        "postgresql://{role}:{PWD}@localhost:5433/backbone_accounting"
    ))
    .await
    .expect("connect read-probe role");

    // Seed one tenant's graph as the owner: an account, a journal, a debit and a credit
    // line, and a partial edge between them.
    let company = Uuid::new_v4();
    let account = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO accounting.accounts
             (id, company_id, account_number, account_code, name, account_type,
              account_subtype, normal_balance, status)
           VALUES ($1,$2,$3,$3,'read probe','asset'::account_type,
                   'accounts_receivable'::account_subtype,'debit'::normal_balance,
                   'active'::account_status)"#,
    )
    .bind(account)
    .bind(company)
    .bind(format!("RLR-{account}"))
    .execute(&admin)
    .await
    .unwrap();
    let journal = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO accounting.journals
             (id, company_id, journal_number, journal_type, source, transaction_date,
              description, currency, status)
           VALUES ($1,$2,$3,'general'::journal_type,'manual'::journal_source,'2026-06-15',
                   'read probe','IDR','posted'::journal_status)"#,
    )
    .bind(journal)
    .bind(company)
    .bind(format!("RLR-{journal}"))
    .execute(&admin)
    .await
    .unwrap();
    let mut lines = Vec::new();
    for n in 1..=2 {
        let id = Uuid::new_v4();
        sqlx::query(
            r#"INSERT INTO accounting.journal_lines
                 (id, journal_id, company_id, line_number, account_id, account_number,
                  account_name, debit_amount, credit_amount, base_debit_amount,
                  base_credit_amount, is_posted)
               VALUES ($1,$2,$3,$4,$5,'RLR','read probe',100,0,100,0,TRUE)"#,
        )
        .bind(id)
        .bind(journal)
        .bind(company)
        .bind(n)
        .bind(account)
        .execute(&admin)
        .await
        .unwrap();
        lines.push(id);
    }
    let partial = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO accounting.partial_reconciles
             (id, company_id, debit_move_id, credit_move_id, amount, currency, max_date,
              origin, updated_at)
           VALUES ($1,$2,$3,$4,40,'IDR','2026-06-15','manual'::reconcile_origin,NOW())"#,
    )
    .bind(partial)
    .bind(company)
    .bind(lines[0])
    .bind(lines[1])
    .execute(&admin)
    .await
    .unwrap();

    // The service over the RESTRICTED pool — the pool verbs bind the company themselves,
    // exactly like an HTTP read on a fenced deployment.
    let svc = ReconcileWriteService::new(
        Arc::new(SqlxReconcileGraphRepository::new()),
        Arc::new(SqlxPostingRepository::new(restricted.clone())),
        restricted.clone(),
        None,
    );
    let group = svc
        .matching_group(company, lines[0])
        .await
        .expect("matching-group read succeeds");
    assert!(
        group.label.starts_with("P-"),
        "partial-only component must carry a P- label, got {:?}",
        group.label
    );
    assert_eq!(
        group.line_ids.len(),
        2,
        "the component must span both lines, got {:?}",
        group.line_ids
    );
    assert_eq!(group.partial_ids, vec![partial], "the edge must be found");
    assert!(
        group.residuals.iter().all(|(_, r)| *r > 0.into()),
        "residuals must be readable through the fence"
    );

    let _ = sqlx::query(&format!("DROP OWNED BY {role}"))
        .execute(&admin)
        .await;
    let _ = sqlx::query(&format!("DROP ROLE IF EXISTS {role}"))
        .execute(&admin)
        .await;
}

// Reporting reads under the restricted role — the bare-pool regression probe.
//
// The reporting repository's reads must ride the company-scoped helpers: a bare pooled
// fetch under the app role loses `set_config(is_local)` and the RLS fence silently
// empties the result (HTTP 200, zero rows — green on superuser DSNs, empty under the
// app role). This probe pins the scoped shape: the caller's company reads fine, another
// tenant's explicit company_id parameter still returns nothing, and an unscoped bare
// read under the restricted role returns nothing (which is exactly why the scope is
// load-bearing).
#[tokio::test]
async fn reporting_reads_scoped_under_restricted_role() {
    let _guard = ROLE_DDL_LOCK.lock().await;
    let admin = admin().await;
    bootstrap_role(&admin).await;
    for table in [
        "accounts",
        "ledgers",
        "journals",
        "journal_lines",
        "partial_reconciles",
    ] {
        sqlx::query(&format!("GRANT SELECT ON accounting.{table} TO {ROLE}"))
            .execute(&admin)
            .await
            .unwrap();
    }

    let company_a = Uuid::new_v4();
    let company_b = Uuid::new_v4();
    for (company, number) in [(company_a, "1100"), (company_b, "2200")] {
        sqlx::query(
            r#"INSERT INTO accounting.accounts
                (id, company_id, account_number, account_code, name, account_type, account_subtype,
                 normal_balance, is_detail, is_header, status)
               VALUES ($1,$2,$3,$3,'probe','asset'::account_type,'bank'::account_subtype,
                       'debit'::normal_balance, TRUE, FALSE, 'active'::account_status)"#,
        )
        .bind(Uuid::new_v4())
        .bind(company)
        .bind(number)
        .execute(&admin)
        .await
        .unwrap();
    }

    // Company A also gets a posted AR line + ledger row so the GL, partner-ledger, and aged
    // reads — the residual-subquery legs — are exercised under the fence, not just the
    // account directory.
    let ar = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO accounting.accounts
            (id, company_id, account_number, account_code, name, account_type, account_subtype,
             normal_balance, is_detail, is_header, status)
           VALUES ($1,$2,'1200','1200','probe AR','asset'::account_type,
                   'accounts_receivable'::account_subtype,'debit'::normal_balance,
                   TRUE, FALSE, 'active'::account_status)"#,
    )
    .bind(ar)
    .bind(company_a)
    .execute(&admin)
    .await
    .unwrap();
    let party = Uuid::new_v4();
    let journal = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO accounting.journals
             (id, company_id, journal_number, journal_type, source, transaction_date,
              description, currency, status)
           VALUES ($1,$2,'RLP-1','general'::journal_type,'manual'::journal_source,'2026-06-15',
                   'reporting probe','IDR','posted'::journal_status)"#,
    )
    .bind(journal)
    .bind(company_a)
    .execute(&admin)
    .await
    .unwrap();
    let jline = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO accounting.journal_lines
             (id, journal_id, company_id, line_number, account_id, account_number,
              account_name, debit_amount, credit_amount, base_debit_amount,
              base_credit_amount, is_posted, party_type, party_id)
           VALUES ($1,$2,$3,1,$4,'1200','probe AR',100,0,100,0,TRUE,
                   'customer'::party_type,$5)"#,
    )
    .bind(jline)
    .bind(journal)
    .bind(company_a)
    .bind(ar)
    .bind(party)
    .execute(&admin)
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO accounting.ledgers
             (id, company_id, account_id, account_number, account_name, account_type,
              normal_balance, journal_id, journal_number, journal_line_id, transaction_date,
              posting_date, description, currency, debit_amount, credit_amount,
              balance_before, balance_after, balance_change, sequence_number, party_type, party_id,
              fiscal_year, fiscal_month)
           VALUES ($1,$2,$3,'1200','probe AR','asset'::account_type,'debit'::normal_balance,
                   $4,'RLP-1',$5,'2026-06-15','2026-06-15','reporting probe','IDR',
                   100,0,0,100,100,1,'customer'::party_type,$6,2026,6)"#,
    )
    .bind(Uuid::new_v4())
    .bind(company_a)
    .bind(ar)
    .bind(journal)
    .bind(jline)
    .bind(party)
    .execute(&admin)
    .await
    .unwrap();

    let restricted = restricted().await;
    let repo =
        backbone_accounting::infrastructure::persistence::reporting_repository::SqlxReportingRepository::new(
            restricted.clone(),
        );

    // Scoped to A: A's account is visible — the scoped read does not silently empty.
    let (scoped_a, scoped_b, gl, pl, aged) =
        backbone_orm::company_scope::with_company_scope(Some(company_a), async {
            let a = repo.account_directory(company_a).await.unwrap();
            let b = repo.account_directory(company_b).await.unwrap();
            let gl = repo
                .gl_lines(
                    company_a,
                    None,
                    None,
                    chrono::NaiveDate::from_ymd_opt(2026, 6, 30).unwrap(),
                    10,
                    0,
                )
                .await
                .unwrap();
            let pl = repo
                .party_ledger_lines(
                    company_a,
                    "customer",
                    party,
                    chrono::NaiveDate::from_ymd_opt(2026, 6, 30).unwrap(),
                )
                .await
                .unwrap();
            let aged = repo
                .aged_open_items(
                    company_a,
                    "accounts_receivable",
                    chrono::NaiveDate::from_ymd_opt(2026, 6, 30).unwrap(),
                )
                .await
                .unwrap();
            (a, b, gl, pl, aged)
        })
        .await;
    assert_eq!(
        scoped_a.len(),
        2,
        "scoped read must see the caller's company"
    );
    let numbers: Vec<&str> = scoped_a.iter().map(|n| n.account_number.as_str()).collect();
    assert!(numbers.contains(&"1100") && numbers.contains(&"1200"));
    assert!(
        scoped_b.is_empty(),
        "cross-tenant parameter must not leak rows"
    );
    assert_eq!(gl.len(), 1, "scoped GL read must see the ledger row");
    assert_eq!(
        pl.len(),
        1,
        "scoped partner-ledger read must see the AR line"
    );
    assert_eq!(
        pl[0].residual,
        100.into(),
        "residual subquery rides the fence"
    );
    assert_eq!(aged.len(), 1, "scoped aged read must see the open item");

    // Unscoped bare read under the restricted role: the fence empties it (no app.company_id
    // on the pooled connection) — the failure mode this probe guards against regressing into.
    let unscoped = repo.account_directory(company_a).await.unwrap();
    assert!(
        unscoped.is_empty(),
        "unscoped read under the app role must not bypass the fence"
    );
    let unscoped_gl = repo
        .gl_lines(
            company_a,
            None,
            None,
            chrono::NaiveDate::from_ymd_opt(2026, 6, 30).unwrap(),
            10,
            0,
        )
        .await
        .unwrap();
    assert!(
        unscoped_gl.is_empty(),
        "unscoped GL read under the app role must not bypass the fence"
    );

    teardown_role(&admin).await;
}
