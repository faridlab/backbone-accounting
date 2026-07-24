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
    run_approve(svc, id, body.company_id, body.approved_by).await
}

async fn reject(
    State(svc): State<Arc<JournalWorkflowService>>,
    Path(id): Path<Uuid>,
    Json(body): Json<RejectBody>,
) -> impl IntoResponse {
    run_reject(svc, id, body.company_id, body.reason, body.rejected_by).await
}

async fn void(
    State(svc): State<Arc<JournalWorkflowService>>,
    Path(id): Path<Uuid>,
    Json(body): Json<VoidBody>,
) -> impl IntoResponse {
    run_void(svc, id, body.company_id, body.voided_by, body.reason).await
}

/// Shared execution for approve (open + protected variants).
async fn run_approve(
    svc: Arc<JournalWorkflowService>, id: Uuid, company_id: Uuid, approved_by: Option<Uuid>,
) -> axum::response::Response {
    match svc.approve(id, company_id, approved_by).await {
        Ok(r) => Json(WorkflowResponse {
            success: true, journal_id: r.journal_id, status: "posted",
            post_id: Some(r.post_id), idempotent_reuse: Some(r.idempotent_reuse),
        }).into_response(),
        Err(e) => err_response(e).into_response(),
    }
}

/// Shared execution for reject.
async fn run_reject(
    svc: Arc<JournalWorkflowService>, id: Uuid, company_id: Uuid, reason: String, rejected_by: Option<Uuid>,
) -> axum::response::Response {
    match svc.reject(id, company_id, reason, rejected_by).await {
        Ok(()) => Json(WorkflowResponse {
            success: true, journal_id: id, status: "rejected",
            post_id: None, idempotent_reuse: None,
        }).into_response(),
        Err(e) => err_response(e).into_response(),
    }
}

/// Shared execution for void.
async fn run_void(
    svc: Arc<JournalWorkflowService>, id: Uuid, company_id: Uuid, voided_by: Option<Uuid>, reason: String,
) -> axum::response::Response {
    match svc.void(id, company_id, voided_by, reason).await {
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

// =============================================================================
// Authenticated variant — derives approve/reject/void actors from the principal
// =============================================================================
//
// `company_id` still comes from the body (AuthContext carries no tenant; cross-tenant isolation
// is RLS-enforced — see ADR-0011). The actor fields (`approved_by`/`rejected_by`/`voided_by`) are
// taken from the verified `AuthContext`, making the audit trail non-repudiable.

#[cfg(feature = "auth")]
use axum::Extension;

#[cfg(feature = "auth")]
use backbone_auth::{middleware::AuthContext, AuthMiddleware};

#[cfg(feature = "auth")]
fn principal(auth: &AuthContext) -> Option<Uuid> {
    Uuid::parse_str(&auth.user_id).ok()
}

#[cfg(feature = "auth")]
async fn approve_protected(
    State(svc): State<Arc<JournalWorkflowService>>,
    Extension(auth): Extension<AuthContext>,
    Path(id): Path<Uuid>,
    Json(body): Json<ApproveBody>,
) -> impl IntoResponse {
    run_approve(svc, id, body.company_id, principal(&auth)).await
}

#[cfg(feature = "auth")]
async fn reject_protected(
    State(svc): State<Arc<JournalWorkflowService>>,
    Extension(auth): Extension<AuthContext>,
    Path(id): Path<Uuid>,
    Json(body): Json<RejectBody>,
) -> impl IntoResponse {
    run_reject(svc, id, body.company_id, body.reason, principal(&auth)).await
}

#[cfg(feature = "auth")]
async fn void_protected(
    State(svc): State<Arc<JournalWorkflowService>>,
    Extension(auth): Extension<AuthContext>,
    Path(id): Path<Uuid>,
    Json(body): Json<VoidBody>,
) -> impl IntoResponse {
    run_void(svc, id, body.company_id, principal(&auth), body.reason).await
}

#[cfg(feature = "auth")]
/// Authenticated workflow routes: actors derived from the principal. Requires the `auth` feature.
pub fn create_protected_journal_workflow_routes<A: AuthMiddleware + Send + Sync + 'static>(
    service: Arc<JournalWorkflowService>,
    auth: Arc<A>,
) -> Router {
    use axum::{middleware, response::IntoResponse};

    let auth_layer = auth.clone();
    Router::new()
        .route("/journals/:id/submit", post(submit))
        .route("/journals/:id/approve", post(approve_protected))
        .route("/journals/:id/reject", post(reject_protected))
        .route("/journals/:id/void", post(void_protected))
        .with_state(service)
        .layer(middleware::from_fn(move |mut req: axum::extract::Request, next: axum::middleware::Next| {
            let auth = auth_layer.clone();
            async move {
                let token = req
                    .headers()
                    .get(axum::http::header::AUTHORIZATION)
                    .and_then(|h| h.to_str().ok())
                    .and_then(|raw| raw.strip_prefix("Bearer ").or_else(|| raw.strip_prefix("bearer ")))
                    .unwrap_or("");
                match auth.authenticate(token).await {
                    Ok(ctx) => {
                        req.extensions_mut().insert(ctx);
                        next.run(req).await
                    }
                    Err(_) => (
                        axum::http::StatusCode::UNAUTHORIZED,
                        axum::Json(serde_json::json!({
                            "success": false, "error": "unauthorized", "message": "Authentication required"
                        })),
                    ).into_response(),
                }
            }
        }))
}
