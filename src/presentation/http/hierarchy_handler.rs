//! Non-CRUD HTTP surface for entity hierarchy reads.
//!
//! Hand-authored (user-owned; see `metaphor.codegen.yaml`). Wraps `HierarchyService`:
//!   GET /accounts/:id/hierarchy?company_id=..
//!   GET /cost-centers/:id/hierarchy?company_id=..
//!   GET /fiscal-periods/:id/hierarchy?company_id=..
//!
//! Each returns the ancestor chain (root → self) so a client can show where the entity sits in its
//! tree without walking parent links itself.

use std::sync::Arc;

use axum::{extract::Path, extract::Query, extract::State, response::IntoResponse, routing::get, Json, Router};
use serde::Deserialize;
use uuid::Uuid;

use crate::application::service::hierarchy_service::HierarchyService;
use crate::domain::repositories::hierarchy_repository::HierarchyTable;

#[derive(Debug, Deserialize)]
pub struct CompanyQuery {
    pub company_id: Uuid,
}

fn err(e: anyhow::Error) -> (axum::http::StatusCode, Json<serde_json::Value>) {
    (
        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        Json(serde_json::json!({ "success": false, "error": e.to_string() })),
    )
}

async fn ancestors(
    State(svc): State<Arc<HierarchyService>>,
    table: HierarchyTable,
    Path(id): Path<Uuid>,
    Query(q): Query<CompanyQuery>,
) -> impl IntoResponse {
    match svc.ancestors(table, q.company_id, id).await {
        Ok(chain) => Json(serde_json::json!({ "success": true, "hierarchy": chain })).into_response(),
        Err(e) => err(e).into_response(),
    }
}

/// Route composer for the three hierarchy endpoints.
pub fn create_hierarchy_routes(service: Arc<HierarchyService>) -> Router {
    Router::new()
        .route(
            "/accounts/:id/hierarchy",
            get(|st, id, q| ancestors(st, HierarchyTable::Account, id, q)),
        )
        .route(
            "/cost-centers/:id/hierarchy",
            get(|st, id, q| ancestors(st, HierarchyTable::CostCenter, id, q)),
        )
        .route(
            "/fiscal-periods/:id/hierarchy",
            get(|st, id, q| ancestors(st, HierarchyTable::FiscalPeriod, id, q)),
        )
        .with_state(service)
}
