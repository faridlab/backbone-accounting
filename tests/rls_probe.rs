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

use sqlx::{PgPool, Row};
use uuid::Uuid;

const ROLE: &str = "bbacc_rls_probe";
const PWD: &str = "probe";

async fn admin() -> PgPool {
    let url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgresql://postgres:postgres@localhost:5433/backbone_accounting".to_string()
    });
    PgPool::connect(&url).await.expect("connect admin")
}

async fn bootstrap_role(admin: &PgPool) {
    // Restricted role: NOSUPERUSER, NOBYPASSRLS — the posture ADR-0011 demands of the host.
    for stmt in [
        format!("DROP ROLE IF EXISTS {ROLE}"),
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
    PgPool::connect(&url).await.expect("connect restricted role")
}

async fn teardown_role(admin: &PgPool) {
    let _ = sqlx::query(&format!("DROP ROLE IF EXISTS {ROLE}")).execute(admin).await;
}

/// Insert a minimal accounts row for `company`. Returns the account id (or errors under RLS).
async fn try_insert(pool: &PgPool, app_company: Uuid, row_company: Uuid) -> Result<Uuid, sqlx::Error> {
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
    let admin = admin().await;
    bootstrap_role(&admin).await;
    let restricted = restricted().await;

    let tenant_a = Uuid::new_v4();
    let tenant_b = Uuid::new_v4();

    // Mismatched: session = A, row = B → must be rejected by the RLS WITH CHECK predicate.
    let err = try_insert(&restricted, tenant_a, tenant_b).await;
    assert!(err.is_err(), "RLS must reject a write to a non-session tenant");
    assert_eq!(count_for(&admin, tenant_b).await, 0, "no row should have landed for tenant B");

    // Matching: session = A, row = A → succeeds.
    let id = try_insert(&restricted, tenant_a, tenant_a).await.expect("matching write succeeds");
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
