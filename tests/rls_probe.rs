//! Tenancy posture probe (ADR-0029).
//!
//! The module ships NO tenancy: no tenant column, no tenant predicate in any statement,
//! and no RLS policy of its own. What it ships instead is the half-fence the composing
//! service's tenancy decorator completes: every table carries ENABLE + FORCE ROW LEVEL
//! SECURITY with zero policies. This probe pins that posture from below, exactly the
//! family pattern:
//!
//! - the flags are armed and the policy set is empty (schema pin);
//! - a plain non-superuser, NOBYPASSRLS role is default-DENIED — zero rows, writes
//!   refused — no matter what legacy variable is set (no policy reads `app.company_id`
//!   anymore; the decorator's org-scoped policies will, once composed);
//! - the owner/superuser pool sees its own seeded rows plainly, proving the denial is
//!   the missing policy and not an empty database;
//! - the module-side half of the contract still works: the AMBIENT org scope — what a
//!   composing service binds per request — drives reads as the owner, and the same
//!   ambient binding rides a RESTRICTED pool per transaction (the relay shape), where
//!   the read completes and returns exactly what the (absent) policies admit: nothing,
//!   until the decorator composes.
//!
//! Requires DATABASE_URL (defaults to local dev Postgres on :5433) backed by a
//! superuser-capable role so it can mint/teardown the probe role.

use sqlx::{PgPool, Row};
use uuid::Uuid;

use backbone_accounting::domain::repositories::reporting_repository::ReportingRepository;
use backbone_accounting::infrastructure::persistence::SqlxReportingRepository;

const ROLE: &str = "bbacc_tenancy_probe";
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

async fn bootstrap_role(admin: &PgPool, grants: &[&str]) {
    drop_role(admin).await;
    for stmt in [
        format!("CREATE ROLE {ROLE} LOGIN PASSWORD '{PWD}' NOSUPERUSER NOBYPASSRLS"),
        format!("GRANT USAGE ON SCHEMA accounting TO {ROLE}"),
    ]
    .into_iter()
    .chain(grants.iter().map(|t| format!("GRANT SELECT ON accounting.{t} TO {ROLE}")))
    {
        sqlx::query(&stmt).execute(admin).await.unwrap();
    }
}

async fn teardown_role(admin: &PgPool) {
    drop_role(admin).await;
}

// ── The schema pin: armed flags, empty policy set ─────────────────────────────

/// Every accounting base table carries ENABLE + FORCE ROW LEVEL SECURITY and the
/// module ships ZERO policies — the decorator's half-fence. If a strip or regen ever
/// drops the flags, an undecorated deployment would silently become readable by any
/// role the host grants; if a policy ever reappears module-side, the decorator's
/// org-scoped policies would fight it.
#[tokio::test]
async fn tables_carry_rls_flags_and_the_module_ships_no_policy() {
    let admin = admin().await;
    let armed: Vec<String> = sqlx::query(
        "SELECT c.relname FROM pg_class c \
         JOIN pg_namespace n ON n.oid = c.relnamespace \
         WHERE n.nspname = 'accounting' AND c.relkind = 'r' \
           AND c.relrowsecurity AND c.relforcerowsecurity \
         ORDER BY c.relname",
    )
    .fetch_all(&admin)
    .await
    .unwrap()
    .iter()
    .map(|r| r.get::<String, _>("relname"))
    .collect();
    for table in [
        "accounting_posts",
        "accounts",
        "bank_check_sequences",
        "cost_centers",
        "emv_qr_configs",
        "financial_statements",
        "fiscal_periods",
        "full_reconciles",
        "journal_lines",
        "journals",
        "ledgers",
        "partial_reconciles",
        "printed_checks",
        "reconciliation_items",
        "reconciliations",
        "tax_tag_repair_runs",
    ] {
        assert!(
            armed.iter().any(|t| t == table),
            "{table} must carry ENABLE + FORCE ROW LEVEL SECURITY"
        );
    }

    let policies: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pg_policy WHERE polrelid::regnamespace::text = 'accounting'",
    )
    .fetch_one(&admin)
    .await
    .unwrap();
    assert_eq!(
        policies, 0,
        "the module ships no RLS policy — isolation belongs to the composing service's decorator"
    );
}

// ── Default-deny until composed: the plain probe role ─────────────────────────

/// A plain non-superuser, NOBYPASSRLS role with a bare SELECT grant sees NOTHING and
/// cannot write — with or without the legacy company variable set. No policy admits it
/// (there are none), and none reads `app.company_id` anymore. The owner pool still sees
/// its seeded row: the denial is the missing policy, not an empty database.
#[tokio::test]
async fn plain_role_is_default_denied_until_the_decorator_composes() {
    let _ddl = ROLE_DDL_LOCK.lock().await;
    let admin = admin().await;
    bootstrap_role(&admin, &["accounts"]).await;

    // The owner seeds a row as the superuser (whom RLS can never bind).
    let account = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO accounting.accounts
             (id, account_number, account_code, name, account_type, account_subtype,
              normal_balance, is_detail, is_header, status)
           VALUES ($1,$2,$2,'tenancy probe','asset'::account_type,'bank'::account_subtype,
                   'debit'::normal_balance,TRUE,FALSE,'active'::account_status)"#,
    )
    .bind(account)
    .bind(format!("TEN-{account}"))
    .execute(&admin)
    .await
    .unwrap();

    let restricted = PgPool::connect(&format!(
        "postgresql://{ROLE}:{PWD}@localhost:5433/backbone_accounting"
    ))
    .await
    .expect("connect probe role");

    // Bare read: zero rows — default-deny with no policy admitting the role.
    let n: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM accounting.accounts WHERE id=$1")
            .bind(account)
            .fetch_one(&restricted)
            .await
            .unwrap();
    assert_eq!(n, 0, "a role no policy admits sees zero rows");

    // The legacy company variable resurrects nothing: no policy reads it anymore
    // (the decorator's org-scoped policies will, once composed).
    let mut tx = restricted.begin().await.unwrap();
    sqlx::query("SELECT set_config('app.company_id', $1, true)")
        .bind(Uuid::new_v4().to_string())
        .execute(&mut *tx)
        .await
        .unwrap();
    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM accounting.accounts WHERE id=$1")
        .bind(account)
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    assert_eq!(n, 0, "the legacy variable must not bypass the absent policy set");
    tx.rollback().await.unwrap();

    // A write is refused outright (no WITH CHECK policy admits the new row).
    let err = sqlx::query(
        r#"INSERT INTO accounting.accounts
             (id, account_number, account_code, name, account_type, account_subtype,
              normal_balance, is_detail, is_header, status)
           VALUES ($1,$2,$2,'tenancy probe','asset'::account_type,'bank'::account_subtype,
                   'debit'::normal_balance,TRUE,FALSE,'active'::account_status)"#,
    )
    .bind(Uuid::new_v4())
    .bind(format!("TEN-{ROLE}"))
    .execute(&restricted)
    .await;
    assert!(err.is_err(), "a write with no admitting policy must be refused");

    // The owner pool still sees its row.
    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM accounting.accounts WHERE id=$1")
        .bind(account)
        .fetch_one(&admin)
        .await
        .unwrap();
    assert_eq!(n, 1, "the owner pool must still see the seeded row");

    teardown_role(&admin).await;
}

/// The reconciliation-graph tables carry the same posture: a plain probe role granted
/// SELECT on the edge table sees none of the owner's edges, and the legacy variable
/// resurrects nothing there either.
#[tokio::test]
async fn graph_tables_are_default_denied_for_a_plain_role_too() {
    let _ddl = ROLE_DDL_LOCK.lock().await;
    let admin = admin().await;
    bootstrap_role(&admin, &["partial_reconciles", "full_reconciles"]).await;

    // Owner-seeded graph: two posted lines and one edge between them.
    let account = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO accounting.accounts
             (id, account_number, account_code, name, account_type, account_subtype,
              normal_balance, status)
           VALUES ($1,$2,$2,'tenancy probe','asset'::account_type,
                   'accounts_receivable'::account_subtype,'debit'::normal_balance,
                   'active'::account_status)"#,
    )
    .bind(account)
    .bind(format!("TEN-{account}"))
    .execute(&admin)
    .await
    .unwrap();
    let journal = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO accounting.journals
             (id, journal_number, journal_type, source, transaction_date,
              description, currency, status)
           VALUES ($1,$2,'general'::journal_type,'manual'::journal_source,'2026-06-15',
                   'tenancy probe','IDR','posted'::journal_status)"#,
    )
    .bind(journal)
    .bind(format!("TEN-{journal}"))
    .execute(&admin)
    .await
    .unwrap();
    let mut lines = Vec::new();
    for n in 1..=2 {
        let id = Uuid::new_v4();
        sqlx::query(
            r#"INSERT INTO accounting.journal_lines
                 (id, journal_id, line_number, account_id, account_number, account_name,
                  debit_amount, credit_amount, base_debit_amount, base_credit_amount, is_posted)
               VALUES ($1,$2,$3,$4,'TEN','tenancy probe',100,0,100,0,TRUE)"#,
        )
        .bind(id)
        .bind(journal)
        .bind(n)
        .bind(account)
        .execute(&admin)
        .await
        .unwrap();
        lines.push(id);
    }
    let edge = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO accounting.partial_reconciles
             (id, debit_move_id, credit_move_id, amount, currency, max_date, origin, updated_at)
           VALUES ($1,$2,$3,40,'IDR','2026-06-15','manual'::reconcile_origin,NOW())"#,
    )
    .bind(edge)
    .bind(lines[0])
    .bind(lines[1])
    .execute(&admin)
    .await
    .unwrap();

    let restricted = PgPool::connect(&format!(
        "postgresql://{ROLE}:{PWD}@localhost:5433/backbone_accounting"
    ))
    .await
    .expect("connect probe role");

    let n: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM accounting.partial_reconciles WHERE id=$1",
    )
    .bind(edge)
    .fetch_one(&restricted)
    .await
    .unwrap();
    assert_eq!(n, 0, "the probe role sees no edges — no policy admits it");

    // The legacy variable resurrects nothing on the graph tables either.
    let mut tx = restricted.begin().await.unwrap();
    sqlx::query("SELECT set_config('app.company_id', $1, true)")
        .bind(Uuid::new_v4().to_string())
        .execute(&mut *tx)
        .await
        .unwrap();
    let n: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM accounting.partial_reconciles WHERE id=$1",
    )
    .bind(edge)
    .fetch_one(&mut *tx)
    .await
    .unwrap();
    assert_eq!(n, 0, "the legacy variable must not bypass the absent policy set");
    tx.rollback().await.unwrap();

    // The owner still sees its edge.
    let n: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM accounting.partial_reconciles WHERE id=$1",
    )
    .bind(edge)
    .fetch_one(&admin)
    .await
    .unwrap();
    assert_eq!(n, 1, "the owner pool must still see the seeded edge");
}

// ── The module-side half: the ambient org scope drives the reads ──────────────

/// Run `f` with an ambient org scope bound — the single-company emulation of what a
/// composing service resolves and binds per request.
async fn scoped<F, R>(pool: &PgPool, company: Uuid, f: F) -> R
where
    F: std::future::Future<Output = R>,
{
    backbone_orm::org_scope::with_org_request_scope(
        pool,
        backbone_orm::org_scope::OrgScope::for_company_unit(company),
        f,
    )
    .await
    .unwrap()
}

/// The ambient org scope is what the module's reads ride: with a scope bound (the
/// composed shape), the statement reads complete and the scope is visible to
/// module code; without one, nothing is bound. Row isolation itself is the
/// decorator's — this pins the module-side binding contract only.
#[tokio::test]
async fn ambient_org_scope_drives_module_reads() {
    let admin = admin().await;
    let company = Uuid::new_v4();
    let account = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO accounting.accounts
             (id, account_number, account_code, name, account_type, account_subtype,
              normal_balance, is_detail, is_header, status)
           VALUES ($1,$2,$2,'ambient probe','asset'::account_type,'bank'::account_subtype,
                   'debit'::normal_balance,TRUE,FALSE,'active'::account_status)"#,
    )
    .bind(account)
    .bind(format!("AMB-{account}"))
    .execute(&admin)
    .await
    .unwrap();

    let repo = SqlxReportingRepository::new(admin.clone());

    // Inside the scope: bound, visible to module code, reads complete.
    let (scope_inside, rows) = scoped(&admin, company, async {
        let scope = backbone_orm::org_scope::current_org_scope()
            .expect("the ambient scope must be bound inside");
        let rows = repo
            .account_directory(company)
            .await
            .expect("account directory read completes under the ambient scope");
        (scope.legacy_company_id(), rows)
    })
    .await;
    assert_eq!(scope_inside, Some(company));
    assert!(
        rows.iter().any(|a| a.id == account),
        "the owner's read must see its own seeded row"
    );

    // Outside: nothing is bound.
    assert!(
        backbone_orm::org_scope::current_org_scope().is_none(),
        "no ambient scope may leak past the wrapped future"
    );
}

/// The relay shape a decorated host runs: the app role (restricted, NOBYPASSRLS) with
/// the ambient scope bound per request. The binding is per-transaction — a plain pooled
/// connection cannot lose it — and the read completes, returning exactly what the
/// (still absent) policies admit: nothing, until the decorator composes.
#[tokio::test]
async fn restricted_pool_with_ambient_scope_completes_default_denied() {
    let _ddl = ROLE_DDL_LOCK.lock().await;
    let admin = admin().await;
    bootstrap_role(&admin, &["accounts"]).await;
    let restricted = PgPool::connect(&format!(
        "postgresql://{ROLE}:{PWD}@localhost:5433/backbone_accounting"
    ))
    .await
    .expect("connect probe role");

    let company = Uuid::new_v4();
    let account = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO accounting.accounts
             (id, account_number, account_code, name, account_type, account_subtype,
              normal_balance, is_detail, is_header, status)
           VALUES ($1,$2,$2,'relay probe','asset'::account_type,'bank'::account_subtype,
                   'debit'::normal_balance,TRUE,FALSE,'active'::account_status)"#,
    )
    .bind(account)
    .bind(format!("RLY-{account}"))
    .execute(&admin)
    .await
    .unwrap();

    let repo = SqlxReportingRepository::new(restricted.clone());
    let rows = scoped(&restricted, company, repo.account_directory(company))
        .await
        .expect("read completes under the ambient scope");
    assert!(
        rows.is_empty(),
        "the restricted role stays default-denied until the decorator installs policies"
    );
    assert!(
        !backbone_orm::org_scope::current_org_scope().is_some(),
        "no ambient scope may leak past the wrapped future"
    );

    teardown_role(&admin).await;
}
