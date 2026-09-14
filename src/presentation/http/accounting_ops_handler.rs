//! Non-CRUD HTTP surface for accounting operations: bank reconciliation + period close.
//!
//! Hand-authored (user-owned; see `metaphor.codegen.yaml`).
//!   POST /accounting/reconcile                         (body = ReconcileRequest)
//!   POST /accounting/periods/{period_id}/close         (body = { retained_earnings_account_id })
//!
//! Tenancy (ADR-0029): the wire bodies carry no tenant field. The composing service's tenancy
//! decorator scopes every read and write from the request's own scope, so a tenant sent in the
//! body could only ever agree with it or contradict it — and a field that cannot change the
//! answer is a field that invites someone to think it can.

use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::post,
    Json, Router,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::application::service::bank_reconciliation_service::{
    BankReconciliationService, ReconcileRequest,
};
use crate::application::service::period_close_service::PeriodCloseService;

// ── Reconciliation ────────────────────────────────────────────────────────────
async fn reconcile(
    State(svc): State<Arc<BankReconciliationService>>,
    Json(req): Json<ReconcileRequest>,
) -> impl IntoResponse {
    match svc.reconcile(req).await {
        Ok(r) => (StatusCode::OK, Json(serde_json::to_value(r).unwrap())),
        Err(e) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(serde_json::json!({ "success": false, "error": e.to_string() })),
        ),
    }
}

pub fn create_bank_reconciliation_routes(service: Arc<BankReconciliationService>) -> Router {
    Router::new()
        .route("/accounting/reconcile", post(reconcile))
        .with_state(service)
}

// ── Period close ──────────────────────────────────────────────────────────────
#[derive(Debug, Deserialize)]
pub struct ClosePeriodBody {
    pub retained_earnings_account_id: Uuid,
}

async fn close_period(
    State(svc): State<Arc<PeriodCloseService>>,
    Path(period_id): Path<Uuid>,
    Json(body): Json<ClosePeriodBody>,
) -> impl IntoResponse {
    match svc
        .close_period(period_id, body.retained_earnings_account_id)
        .await
    {
        Ok(r) => (StatusCode::OK, Json(serde_json::to_value(r).unwrap())),
        Err(e) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(serde_json::json!({ "success": false, "error": e.to_string() })),
        ),
    }
}

pub fn create_period_close_routes(service: Arc<PeriodCloseService>) -> Router {
    Router::new()
        .route("/accounting/periods/{period_id}/close", post(close_period))
        .with_state(service)
}
