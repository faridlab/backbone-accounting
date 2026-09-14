//! Non-CRUD HTTP surface for entity hierarchy reads.
//!
//! Hand-authored (user-owned; see `metaphor.codegen.yaml`). Wraps `HierarchyService`:
//!   GET /accounts/:id/hierarchy
//!   GET /cost-centers/:id/hierarchy
//!   GET /fiscal-periods/:id/hierarchy
//!
//! Each returns the ancestor chain (root → self) so a client can show where the entity sits in its
//! tree without walking parent links itself.
//!
//! Tenancy (ADR-0029): the wire carries no tenant parameter. The composing service's tenancy
//! decorator scopes the read from the request's own scope, so a company in the query string
//! could only agree with it or contradict it.

use std::sync::Arc;

use axum::{
    extract::Path, extract::Query, extract::State, response::IntoResponse, routing::get, Json,
    Router,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::application::service::hierarchy_service::HierarchyService;
use crate::domain::repositories::hierarchy_repository::HierarchyTable;

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
) -> impl IntoResponse {
    match svc.ancestors(table, id).await {
        Ok(chain) => {
            Json(serde_json::json!({ "success": true, "hierarchy": chain })).into_response()
        }
        Err(e) => err(e).into_response(),
    }
}

/// Route composer for the three hierarchy endpoints.
pub fn create_hierarchy_routes(service: Arc<HierarchyService>) -> Router {
    Router::new()
        .route(
            "/accounts/:id/hierarchy",
            get(|st, id| ancestors(st, HierarchyTable::Account, id)),
        )
        .route(
            "/cost-centers/:id/hierarchy",
            get(|st, id| ancestors(st, HierarchyTable::CostCenter, id)),
        )
        .route(
            "/fiscal-periods/:id/hierarchy",
            get(|st, id| ancestors(st, HierarchyTable::FiscalPeriod, id)),
        )
        .with_state(service)
}
