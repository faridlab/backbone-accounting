//! Non-CRUD HTTP surface for the manual-journal approval workflow.
//!
//! Hand-authored (user-owned; see `metaphor.codegen.yaml`). Wraps `JournalWorkflowService`:
//!   POST /journals/:id/submit?company_id=..                        (no body)
//!   POST /journals/:id/approve   body { company_id, approved_by }
//!   POST /journals/:id/reject    body { company_id, reason, rejected_by? }
//!   POST /journals/:id/void      body { company_id, voided_by?, reason }
//!
//! `approve` posts the journal to the ledger; `void` posts a reversal. Both flow through the
//! audited `PostingService` core (FOR UPDATE per-account lock, idempotency, immutable ledger).

use std::sync::Arc;

use axum::{extract::Path, extract::Query, extract::State, response::IntoResponse, routing::post, Json, Router};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::application::service::journal_workflow_service::{JournalWorkflowError, JournalWorkflowService};

#[derive(Debug, Deserialize)]
pub struct CompanyQuery {
    pub company_id: Uuid,
}

#[derive(Debug, Deserialize)]
pub struct ApproveBody {
    pub company_id: Uuid,
    pub approved_by: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct RejectBody {
    pub company_id: Uuid,
    pub reason: String,
    pub rejected_by: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct VoidBody {
    pub company_id: Uuid,
    pub voided_by: Option<Uuid>,
    pub reason: String,
}

#[derive(Debug, Serialize)]
struct WorkflowResponse {
    success: bool,
    journal_id: Uuid,
    status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    post_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    idempotent_reuse: Option<bool>,
}

fn err_response(e: JournalWorkflowError) -> (axum::http::StatusCode, Json<serde_json::Value>) {
    let status = axum::http::StatusCode::from_u16(e.http_status())
        .unwrap_or(axum::http::StatusCode::INTERNAL_SERVER_ERROR);
    (
        status,
        Json(serde_json::json!({
            "success": false,
            "error": e.code(),
            "message": e.to_string(),
        })),
    )
}

async fn submit(
    State(svc): State<Arc<JournalWorkflowService>>,
    Path(id): Path<Uuid>,
    Query(q): Query<CompanyQuery>,
) -> impl IntoResponse {
    match svc.submit(id, q.company_id).await {
        Ok(()) => Json(WorkflowResponse {
            success: true, journal_id: id, status: "pending_approval",
            post_id: None, idempotent_reuse: None,
        }).into_response(),
        Err(e) => err_response(e).into_response(),
    }
}

async fn approve(
    State(svc): State<Arc<JournalWorkflowService>>,
    Path(id): Path<Uuid>,
    Json(body): Json<ApproveBody>,
) -> impl IntoResponse {
    match svc.approve(id, body.company_id, body.approved_by).await {
        Ok(r) => Json(WorkflowResponse {
            success: true, journal_id: r.journal_id, status: "posted",
            post_id: Some(r.post_id), idempotent_reuse: Some(r.idempotent_reuse),
        }).into_response(),
        Err(e) => err_response(e).into_response(),
    }
}

async fn reject(
    State(svc): State<Arc<JournalWorkflowService>>,
    Path(id): Path<Uuid>,
    Json(body): Json<RejectBody>,
) -> impl IntoResponse {
    match svc.reject(id, body.company_id, body.reason, body.rejected_by).await {
        Ok(()) => Json(WorkflowResponse {
            success: true, journal_id: id, status: "rejected",
            post_id: None, idempotent_reuse: None,
        }).into_response(),
        Err(e) => err_response(e).into_response(),
    }
}

async fn void(
    State(svc): State<Arc<JournalWorkflowService>>,
    Path(id): Path<Uuid>,
    Json(body): Json<VoidBody>,
) -> impl IntoResponse {
    match svc.void(id, body.company_id, body.voided_by, body.reason).await {
        Ok(r) => Json(WorkflowResponse {
            success: true, journal_id: id, status: "voided",
            post_id: Some(r.post_id), idempotent_reuse: Some(r.idempotent_reuse),
        }).into_response(),
        Err(e) => err_response(e).into_response(),
    }
}

/// Route composer for the manual-journal workflow endpoints.
pub fn create_journal_workflow_routes(service: Arc<JournalWorkflowService>) -> Router {
    Router::new()
        .route("/journals/:id/submit", post(submit))
        .route("/journals/:id/approve", post(approve))
        .route("/journals/:id/reject", post(reject))
        .route("/journals/:id/void", post(void))
        .with_state(service)
}
