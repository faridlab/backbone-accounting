//! Non-CRUD HTTP surface for the GL-posting contract: `POST /accounting/posts`.
//!
//! Hand-authored (user-owned; see `metaphor.codegen.yaml`). Wraps `PostingService` — the
//! inbound port a producer (billing/selling/payments) calls to record a balanced entry.

use std::sync::Arc;

use axum::{extract::State, http::StatusCode, response::IntoResponse, routing::post, Json, Router};
use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::application::service::posting_service::{PostingLine, PostingRequest, PostingService};

fn default_idr() -> String {
    "IDR".to_string()
}
fn default_original() -> String {
    "original".to_string()
}

#[derive(Debug, Deserialize)]
pub struct PostingLineDto {
    pub account_id: Uuid,
    #[serde(default)]
    pub debit: Decimal,
    #[serde(default)]
    pub credit: Decimal,
    pub party_type: Option<String>,
    pub party_id: Option<Uuid>,
    pub cost_center_id: Option<Uuid>,
    pub project_id: Option<Uuid>,
    pub department_id: Option<Uuid>,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PostingRequestDto {
    pub company_id: Uuid,
    pub branch_id: Option<Uuid>,
    pub source_type: String,
    pub source_id: Uuid,
    pub source_reference: Option<String>,
    pub posting_date: NaiveDate,
    #[serde(default = "default_idr")]
    pub currency: String,
    #[serde(default = "default_original")]
    pub posting_type: String,
    pub reverses_post_id: Option<Uuid>,
    pub description: Option<String>,
    pub posted_by: Option<Uuid>,
    #[serde(default)]
    pub idempotency_key: Option<String>,
    #[serde(default)]
    pub lines: Vec<PostingLineDto>,
}

#[derive(Debug, Serialize)]
pub struct PostingResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub post_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub journal_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub posting_status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idempotent_reuse: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
}

async fn post_handler(
    State(service): State<Arc<PostingService>>,
    Json(dto): Json<PostingRequestDto>,
) -> impl IntoResponse {
    let posted_by = dto.posted_by;
    run_post(service, posting_request_from_dto(dto), posted_by).await
}

/// Map the wire DTO to the domain `PostingRequest`.
fn posting_request_from_dto(dto: PostingRequestDto) -> PostingRequest {
    PostingRequest {
        company_id: dto.company_id,
        branch_id: dto.branch_id,
        source_type: dto.source_type,
        source_id: dto.source_id,
        source_reference: dto.source_reference,
        posting_date: dto.posting_date,
        currency: dto.currency,
        posting_type: dto.posting_type,
        reverses_post_id: dto.reverses_post_id,
        description: dto.description,
        lines: dto
            .lines
            .into_iter()
            .map(|l| PostingLine {
                account_id: l.account_id,
                debit: l.debit,
                credit: l.credit,
                party_type: l.party_type,
                party_id: l.party_id,
                cost_center_id: l.cost_center_id,
                project_id: l.project_id,
                department_id: l.department_id,
                description: l.description,
            })
            .collect(),
        idempotency_key: dto.idempotency_key,
    }
}

/// Shared post execution + response mapping for the open and protected handlers.
async fn run_post(
    service: Arc<PostingService>,
    req: PostingRequest,
    posted_by: Option<Uuid>,
) -> axum::response::Response {
    match service.post(req, posted_by).await {
        Ok(r) => (
            StatusCode::OK,
            Json(PostingResponse {
                success: true,
                post_id: Some(r.post_id),
                journal_id: Some(r.journal_id),
                posting_status: Some(r.posting_status),
                idempotent_reuse: Some(r.idempotent_reuse),
                error_code: None,
            }),
        )
            .into_response(),
        Err(e) => (
            StatusCode::from_u16(e.http_status()).unwrap_or(StatusCode::UNPROCESSABLE_ENTITY),
            Json(PostingResponse {
                success: false,
                post_id: None,
                journal_id: None,
                posting_status: None,
                idempotent_reuse: None,
                error_code: Some(e.code().to_string()),
            }),
        )
            .into_response(),
    }
}

/// Route: `POST /accounting/posts` — the inbound GL-posting endpoint.
pub fn create_posting_routes(service: Arc<PostingService>) -> Router {
    Router::new()
        .route("/accounting/posts", post(post_handler))
        .with_state(service)
}

// =============================================================================
// Authenticated variant — derives `posted_by` from the verified principal
// =============================================================================
//
// Use this in hosts that mount an auth middleware: `posted_by` is taken from the `AuthContext`
// (non-repudiable audit trail) rather than trusted from the request body. `company_id` still comes
// from the body (AuthContext carries no tenant); cross-tenant isolation is RLS-enforced — see
// ADR-0011 for the host role/`app.company_id` contract.

#[cfg(feature = "auth")]
use axum::Extension;

#[cfg(feature = "auth")]
use backbone_auth::{middleware::AuthContext, AuthMiddleware};

#[cfg(feature = "auth")]
async fn post_handler_protected(
    State(service): State<Arc<PostingService>>,
    Extension(auth): Extension<AuthContext>,
    Json(dto): Json<PostingRequestDto>,
) -> impl IntoResponse {
    let req = posting_request_from_dto(dto);
    // The auditor is the authenticated principal, not a body field.
    let posted_by = uuid::Uuid::parse_str(&auth.user_id).ok();
    run_post(service, req, posted_by).await
}

#[cfg(feature = "auth")]
/// Authenticated route: `POST /accounting/posts` with `posted_by` derived from the principal.
/// Requires the `auth` feature; wrap with a host that supplies a bearer token.
pub fn create_protected_posting_routes<A: AuthMiddleware + Send + Sync + 'static>(
    service: Arc<PostingService>,
    auth: Arc<A>,
) -> Router {
    use axum::{middleware, Extension, response::IntoResponse};

    let auth_layer = auth.clone();
    Router::new()
        .route("/accounting/posts", post(post_handler_protected))
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
                            "success": false,
                            "error": "unauthorized",
                            "message": "Authentication required"
                        })),
                    )
                        .into_response(),
                }
            }
        }))
}
