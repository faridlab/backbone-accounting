//! Golden-case oracle for the entity-hierarchy (ancestors) endpoint.
//!
//! Seeds a root → mid → leaf chain for each of account / cost_center / fiscal_period and asserts
//! the service returns the chain root-first. Plus an HTTP-level check that the route is mounted and
//! returns the chain JSON.
//!
//! Requires DATABASE_URL (defaults to local dev Postgres on :5433).

use std::sync::Arc;

use sqlx::PgPool;
use uuid::Uuid;

use backbone_accounting::application::service::hierarchy_service::HierarchyService;
use backbone_accounting::domain::repositories::hierarchy_repository::HierarchyTable;
use backbone_accounting::infrastructure::persistence::SqlxHierarchyRepository;

async fn pool() -> PgPool {
    let url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgresql://postgres:postgres@localhost:5433/backbone_accounting".to_string()
    });
    PgPool::connect(&url).await.expect("connect DB")
}

fn svc(pool: &PgPool) -> HierarchyService {
    HierarchyService::new(Arc::new(SqlxHierarchyRepository::new(pool.clone())))
}

/// Insert a 3-level account chain; returns (company, root, mid, leaf).
async fn seed_accounts(pool: &PgPool) -> (Uuid, Uuid, Uuid, Uuid) {
    let company = Uuid::new_v4();
    let (root, mid, leaf) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    for (id, code, name, parent, level) in [
        (root, "1", "Assets", None, 0),
        (mid, "1-1", "Current Assets", Some(root), 1),
        (leaf, "1-1-1", "Cash", Some(mid), 2),
    ] {
        sqlx::query(
            r#"INSERT INTO accounting.accounts
                (id, company_id, account_number, account_code, name, account_type, account_subtype,
                 normal_balance, is_detail, is_header, parent_id, level, status)
               VALUES ($1,$2,$3,$3,$4,'asset'::account_type,'current_asset'::account_subtype,
                       'debit'::normal_balance, TRUE, FALSE, $5, $6, 'active'::account_status)"#,
        )
        .bind(id).bind(company).bind(code).bind(name).bind(parent).bind(level)
        .execute(pool).await.unwrap();
    }
    (company, root, mid, leaf)
}

async fn seed_cost_centers(pool: &PgPool) -> (Uuid, Uuid, Uuid, Uuid) {
    let company = Uuid::new_v4();
    let (root, mid, leaf) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    for (id, code, name, parent, level) in [
        (root, "CC", "All Centers", None, 0),
        (mid, "CC-1", "Operations", Some(root), 1),
        (leaf, "CC-1-1", "Production", Some(mid), 2),
    ] {
        sqlx::query(
            r#"INSERT INTO accounting.cost_centers (id, company_id, code, name, parent_id, level)
               VALUES ($1,$2,$3,$4,$5,$6)"#,
        )
        .bind(id).bind(company).bind(code).bind(name).bind(parent).bind(level)
        .execute(pool).await.unwrap();
    }
    (company, root, mid, leaf)
}

async fn seed_fiscal_periods(pool: &PgPool) -> (Uuid, Uuid, Uuid, Uuid) {
    let company = Uuid::new_v4();
    let (root, mid, leaf) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    let y = chrono::NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
    for (id, code, name, parent, level, start, end) in [
        (root, "FY2026", "FY 2026", None, 0, y, y + chrono::Duration::days(365)),
        (mid, "Q1", "Q1 2026", Some(root), 1, y, y + chrono::Duration::days(90)),
        (leaf, "M01", "Jan 2026", Some(mid), 2, y, y + chrono::Duration::days(31)),
    ] {
        sqlx::query(
            r#"INSERT INTO accounting.fiscal_periods
                (id, company_id, period_code, name, parent_id, level, start_date, end_date, fiscal_year)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,2026)"#,
        )
        .bind(id).bind(company).bind(code).bind(name).bind(parent).bind(level).bind(start).bind(end)
        .execute(pool).await.unwrap();
    }
    (company, root, mid, leaf)
}

#[tokio::test]
async fn account_ancestors_root_first() {
    let pool = pool().await;
    let (company, root, mid, leaf) = seed_accounts(&pool).await;
    let chain = svc(&pool).ancestors(HierarchyTable::Account, company, leaf).await.unwrap();
    let ids: Vec<Uuid> = chain.iter().map(|n| n.id).collect();
    assert_eq!(ids, vec![root, mid, leaf], "root → self order");
    assert_eq!(chain[2].parent_id, Some(mid));
    // Leaf's own lookup returns just itself.
    let only_root = svc(&pool).ancestors(HierarchyTable::Account, company, root).await.unwrap();
    assert_eq!(only_root.len(), 1);
    // Unknown id → empty.
    let missing = svc(&pool).ancestors(HierarchyTable::Account, company, Uuid::new_v4()).await.unwrap();
    assert!(missing.is_empty());
}

#[tokio::test]
async fn cost_center_ancestors_root_first() {
    let pool = pool().await;
    let (company, root, mid, leaf) = seed_cost_centers(&pool).await;
    let chain = svc(&pool).ancestors(HierarchyTable::CostCenter, company, leaf).await.unwrap();
    let ids: Vec<Uuid> = chain.iter().map(|n| n.id).collect();
    assert_eq!(ids, vec![root, mid, leaf]);
}

#[tokio::test]
async fn fiscal_period_ancestors_root_first() {
    let pool = pool().await;
    let (company, root, mid, leaf) = seed_fiscal_periods(&pool).await;
    let chain = svc(&pool).ancestors(HierarchyTable::FiscalPeriod, company, leaf).await.unwrap();
    let ids: Vec<Uuid> = chain.iter().map(|n| n.id).collect();
    assert_eq!(ids, vec![root, mid, leaf]);
}

#[tokio::test]
async fn hierarchy_route_returns_chain_json() {
    use backbone_accounting::presentation::http::create_hierarchy_routes;
    let pool = pool().await;
    let (company, root, mid, leaf) = seed_accounts(&pool).await;
    let svc = Arc::new(svc(&pool));
    let router = create_hierarchy_routes(svc);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, router).await.unwrap(); });
    tokio::time::sleep(std::time::Duration::from_millis(150)).await;

    let resp = reqwest::get(format!(
        "http://{addr}/accounts/{leaf}/hierarchy?company_id={company}"
    ))
    .await
    .unwrap();
    assert!(resp.status().is_success());
    let body: serde_json::Value = resp.json().await.unwrap();
    let hierarchy = body["hierarchy"].as_array().unwrap();
    let got: Vec<String> = hierarchy.iter().map(|n| n["id"].as_str().unwrap().to_string()).collect();
    let want: Vec<String> = vec![root, mid, leaf].into_iter().map(|u| u.to_string()).collect();
    assert_eq!(got, want, "route returns root → self chain");
}
