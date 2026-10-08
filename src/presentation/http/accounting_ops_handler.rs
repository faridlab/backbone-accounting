//! Non-CRUD HTTP surface for accounting operations: bank reconciliation + the
//! fiscal period lifecycle.
//!
//! Hand-authored (user-owned; see `metaphor.codegen.yaml`).
//!   POST /accounting/reconcile                       (body = ReconcileRequest)
//!   POST /accounting/periods/:period_id/close        (body = { retained_earnings_account_id })
//!   POST /accounting/periods/:period_id/lock         (body = { reason })
//!   POST /accounting/periods/:period_id/reopen       (body = { reason })
//!
//! A period's status moves only through these verbs; generic writes refuse
//! it. The person acting is the authenticated principal (`OrgContext`), never
//! a field of the body, so the trail on the period names who really acted. A
//! request without one is refused before the handler runs. Which role may
//! call which verb is the composing service's gate.
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
use crate::application::service::period_close_service::{PeriodCloseError, PeriodCloseService};
use backbone_auth::org::OrgContext;

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

// ── Fiscal period lifecycle ───────────────────────────────────────────────────
#[derive(Debug, Deserialize)]
pub struct ClosePeriodBody {
    pub retained_earnings_account_id: Uuid,
}

/// The body of a lock or reopen: why. A blank reason is refused.
#[derive(Debug, Deserialize)]
pub struct PeriodReasonBody {
    #[serde(default)]
    pub reason: String,
}

fn actor(org: &OrgContext) -> Option<Uuid> {
    Uuid::parse_str(&org.user_id).ok()
}

/// 404 for a period that is not there, 409 for a move the period's current
/// state does not allow, 422 for a request that is incomplete or a closing
/// entry the posting rules refuse, 500 otherwise.
fn period_error(e: PeriodCloseError) -> (StatusCode, Json<serde_json::Value>) {
    let status = match &e {
        PeriodCloseError::PeriodNotFound(_) => StatusCode::NOT_FOUND,
        PeriodCloseError::AlreadyClosed
        | PeriodCloseError::CloseInProgress
        | PeriodCloseError::NotClosed { .. }
        | PeriodCloseError::Locked => StatusCode::CONFLICT,
        PeriodCloseError::ReasonRequired | PeriodCloseError::Posting(_) => StatusCode::UNPROCESSABLE_ENTITY,
        PeriodCloseError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
    };
    (status, Json(serde_json::json!({ "success": false, "error": e.to_string() })))
}

async fn close_period(
    State(svc): State<Arc<PeriodCloseService>>,
    org: OrgContext,
    Path(period_id): Path<Uuid>,
    Json(body): Json<ClosePeriodBody>,
) -> impl IntoResponse {
    match svc
        .close_period_as(period_id, body.retained_earnings_account_id, actor(&org))
        .await
    {
        Ok(r) => (StatusCode::OK, Json(serde_json::to_value(r).unwrap())),
        Err(e) => period_error(e),
    }
}

async fn lock_period(
    State(svc): State<Arc<PeriodCloseService>>,
    org: OrgContext,
    Path(period_id): Path<Uuid>,
    Json(body): Json<PeriodReasonBody>,
) -> impl IntoResponse {
    match svc.lock_period(period_id, actor(&org), &body.reason).await {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({ "success": true, "period_id": period_id, "status": "locked" }))),
        Err(e) => period_error(e),
    }
}

async fn reopen_period(
    State(svc): State<Arc<PeriodCloseService>>,
    org: OrgContext,
    Path(period_id): Path<Uuid>,
    Json(body): Json<PeriodReasonBody>,
) -> impl IntoResponse {
    match svc.reopen_period(period_id, actor(&org), &body.reason).await {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({ "success": true, "period_id": period_id, "status": "open" }))),
        Err(e) => period_error(e),
    }
}

/// The routine close. Lock and reopen live in
/// [`create_period_finality_routes`], so the two can sit behind different
/// roles.
pub fn create_period_close_routes(service: Arc<PeriodCloseService>) -> Router {
    Router::new()
        .route("/accounting/periods/:period_id/close", post(close_period))
        .with_state(service)
}

/// The lock and reopen verbs: the ones that make a period final or undo a
/// close. A separate router so the composing service can put them behind a
/// stricter role than the routine close.
pub fn create_period_finality_routes(service: Arc<PeriodCloseService>) -> Router {
    Router::new()
        .route("/accounting/periods/:period_id/lock", post(lock_period))
        .route("/accounting/periods/:period_id/reopen", post(reopen_period))
        .with_state(service)
}
