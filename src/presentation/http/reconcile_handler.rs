//! Reconciliation verb HTTP surface — hand-authored (NOT generated).
//!
//! Three routes over the `ReconcileWriteService` pool verbs:
//! - `POST /accounting/reconcile` — create (or grow) a debit↔credit edge
//! - `POST /accounting/unreconcile/:partial_id` — the side-effecting unlink (reverses
//!   generated moves, repairs flags/groups)
//! - `GET  /accounting/reconciliation-groups/:line_id` — the matching-group read
//!
//! The graph tables themselves expose NO CRUD routes anywhere (`enabled: false` +
//! `guarded_routes` mounts none) — this file IS the only write surface. Hosts mount the
//! `auth`-protected variants; the accountant role gate on unlink is a HOST concern
//! (module-level default-deny is the composition's job — see the ADR).
//!
//! This file is user-owned (see `metaphor.codegen.yaml`) and survives regeneration.

use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::application::service::reconcile_write_service::ReconcileWriteService;
use crate::domain::reconcile_graph::{
    EdgeOutcome, LineLocator, PairRequest, ReconcileError, ORIGIN_MANUAL,
};

// =============================================================================
// DTOs
// =============================================================================

#[derive(Debug, Deserialize)]
pub struct ReconcileLineDto {
    /// Producer identity of the journal line, e.g. "payment" / "order" / "expense".
    pub source_type: String,
    /// Producer identity value (payment id, invoice ref, …).
    pub source_id: Uuid,
    /// The reconcilable control account the line sits on.
    pub account_id: Uuid,
    /// True when the line belongs to a reversal journal.
    #[serde(default)]
    pub reversing: bool,
}

#[derive(Debug, Deserialize)]
pub struct ReconcilePairRequestDto {
    pub company_id: Uuid,
    pub debit: ReconcileLineDto,
    pub credit: ReconcileLineDto,
    /// Company-currency amount requested; clamped to the smaller residual.
    pub amount: Decimal,
    /// settlement | clearing | manual (defaults to manual).
    #[serde(default = "default_origin")]
    pub origin: String,
}

fn default_origin() -> String {
    ORIGIN_MANUAL.to_string()
}

#[derive(Debug, Serialize)]
pub struct ReconcileEdgeAckDto {
    /// `null` when the request clamped to zero — no edge exists (on-account remainder).
    pub partial_id: Option<Uuid>,
    pub applied: Decimal,
    pub full_reconcile_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct UnreconcileBody {
    pub company_id: Uuid,
}

#[derive(Debug, Deserialize)]
pub struct CompanyQuery {
    pub company_id: Uuid,
}

#[derive(Debug, Serialize)]
pub struct MatchingGroupDto {
    pub label: String,
    pub full_reconcile_id: Option<Uuid>,
    pub line_ids: Vec<Uuid>,
    pub partial_ids: Vec<Uuid>,
    pub residuals: Vec<ResidualDto>,
}

#[derive(Debug, Serialize)]
pub struct ResidualDto {
    pub line_id: Uuid,
    pub residual: Decimal,
}

#[derive(Debug, Serialize)]
pub struct ReconcileErrorDto {
    pub code: String,
    pub message: String,
}

// =============================================================================
// Mapping
// =============================================================================

fn locator_from_dto(dto: ReconcileLineDto) -> LineLocator {
    let l = LineLocator::new(&dto.source_type, dto.source_id, dto.account_id);
    if dto.reversing {
        l.reversing()
    } else {
        l
    }
}

fn ack_from_outcome(o: EdgeOutcome) -> ReconcileEdgeAckDto {
    ReconcileEdgeAckDto {
        partial_id: o.partial_id,
        applied: o.applied,
        full_reconcile_id: o.full_reconcile_id,
    }
}

fn error_response(e: &ReconcileError) -> axum::response::Response {
    let status =
        StatusCode::from_u16(e.http_status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    (
        status,
        Json(ReconcileErrorDto {
            code: e.code().to_string(),
            message: e.to_string(),
        }),
    )
        .into_response()
}

// =============================================================================
// Handlers
// =============================================================================

async fn reconcile_handler(
    State(service): State<Arc<ReconcileWriteService>>,
    Json(dto): Json<ReconcilePairRequestDto>,
) -> impl IntoResponse {
    let req = PairRequest {
        company_id: dto.company_id,
        debit: locator_from_dto(dto.debit),
        credit: locator_from_dto(dto.credit),
        amount: dto.amount,
        origin: dto.origin,
        actor: None,
    };
    match service.reconcile_pair(&req).await {
        Ok(o) => (StatusCode::OK, Json(ack_from_outcome(o))).into_response(),
        Err(e) => error_response(&e),
    }
}

async fn unreconcile_handler(
    State(service): State<Arc<ReconcileWriteService>>,
    Path(partial_id): Path<Uuid>,
    Json(body): Json<UnreconcileBody>,
) -> impl IntoResponse {
    match service.unreconcile(body.company_id, partial_id, None).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => error_response(&e),
    }
}

async fn matching_group_handler(
    State(service): State<Arc<ReconcileWriteService>>,
    Path(line_id): Path<Uuid>,
    Query(q): Query<CompanyQuery>,
) -> impl IntoResponse {
    match service.matching_group(q.company_id, line_id).await {
        Ok(g) => (
            StatusCode::OK,
            Json(MatchingGroupDto {
                label: g.label,
                full_reconcile_id: g.full_reconcile_id,
                line_ids: g.line_ids,
                partial_ids: g.partial_ids,
                residuals: g
                    .residuals
                    .into_iter()
                    .map(|(line_id, residual)| ResidualDto { line_id, residual })
                    .collect(),
            }),
        )
            .into_response(),
        Err(e) => error_response(&e),
    }
}

// =============================================================================
// Routes
// =============================================================================

/// The reconciliation verb routes (unauthenticated variant — for tests and trusted
/// internal hosts; hosts with an auth middleware wrap these handlers with their own
/// layers and principal-derived `actor`).
pub fn create_reconcile_verb_routes(service: Arc<ReconcileWriteService>) -> Router {
    Router::new()
        .route("/accounting/reconcile", post(reconcile_handler))
        .route("/accounting/unreconcile/:partial_id", post(unreconcile_handler))
        .route("/accounting/reconciliation-groups/:line_id", get(matching_group_handler))
        .with_state(service)
}
