//! Check printing HTTP surface — hand-authored (user-owned; see
//! `metaphor.codegen.yaml`).
//!
//! Four routes over the `CheckPrintingService`:
//! - `PUT    /accounting/checks/sequences`        — register/replace a
//!   bank-journal sequence (auto/manual mode, starting number)
//! - `POST   /accounting/checks/allocate`         — allocate the next number(s)
//!   from an AUTO sequence (atomic; distinct under concurrency)
//! - `POST   /accounting/checks`                  — record one printed check
//!   (auto: allocates inside the verb transaction; manual: validates + unique)
//! - `POST   /accounting/checks/:id/void`         — void a printed check (the
//!   number stays consumed)
//!
//! Hosts mount these behind their own auth layers; the registry verbs belong
//! behind an accounting-officer role gate at the host. The tenant-consistency
//! check refuses a body company that disagrees with the ambient scope.
//!
//! Tenancy (ADR-0029): the `company_id` fields in the wire bodies are the legacy
//! twin — kept so unstripped callers compile and run unchanged; the module keys
//! no statement on them and the composing service's tenancy decorator scopes the
//! underlying reads/writes.

use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{post, put},
    Json, Router,
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::application::service::check_printing_service::{
    AllocationAck, CheckPrintingError, CheckPrintingService, PrintedCheckAck,
    RecordCheck, RegisterSequence, SequenceAck,
};

#[derive(Debug, Deserialize)]
pub struct RegisterSequenceBody {
    pub company_id: Uuid,
    pub bank_account_id: Uuid,
    /// "auto" | "manual".
    pub numbering_mode: String,
    #[serde(default = "default_next")]
    pub next_number: i64,
}

fn default_next() -> i64 {
    1
}

#[derive(Debug, Deserialize)]
pub struct AllocateBody {
    pub company_id: Uuid,
    pub bank_account_id: Uuid,
    #[serde(default = "default_count")]
    pub count: i64,
}

fn default_count() -> i64 {
    1
}

#[derive(Debug, Deserialize)]
pub struct RecordCheckBody {
    pub company_id: Uuid,
    pub bank_account_id: Uuid,
    pub payment_id: Uuid,
    #[serde(default)]
    pub payment_number: Option<String>,
    pub amount: Decimal,
    #[serde(default)]
    pub payee_name: Option<String>,
    /// Manual numbering only; must be absent for auto sequences.
    #[serde(default)]
    pub check_number: Option<String>,
    #[serde(default)]
    pub actor: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct VoidBody {
    pub company_id: Uuid,
    #[serde(default)]
    pub actor: Option<Uuid>,
}

#[derive(Debug, Serialize)]
struct ErrorBody {
    error: String,
    message: String,
}

fn error_response(e: &CheckPrintingError) -> axum::response::Response {
    let status = StatusCode::from_u16(e.http_status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    (
        status,
        Json(ErrorBody {
            error: e.code().to_string(),
            message: e.to_string(),
        }),
    )
        .into_response()
}

fn tenant_mismatch(req_company: Uuid) -> bool {
    // Scope-aware (ADR-0029): prefer the ambient org scope's legacy company id;
    // fall back to the legacy company lane for unstripped hosts. Neither bound —
    // e.g. an undecorated deployment — means nothing to compare against, so no
    // refusal.
    let authenticated = backbone_orm::org_scope::current_org_scope()
        .and_then(|s| s.legacy_company_id())
        .or_else(|| backbone_orm::current_company());
    match authenticated {
        Some(authenticated) => authenticated != req_company,
        None => false,
    }
}

fn forbidden_tenant() -> axum::response::Response {
    (
        StatusCode::FORBIDDEN,
        Json(ErrorBody {
            error: "company_mismatch".into(),
            message: "the request's company_id does not match the authenticated company".into(),
        }),
    )
        .into_response()
}

async fn register_sequence(
    State(service): State<Arc<CheckPrintingService>>,
    Json(body): Json<RegisterSequenceBody>,
) -> impl IntoResponse {
    if tenant_mismatch(body.company_id) {
        return forbidden_tenant();
    }
    let req = RegisterSequence {
        company_id: body.company_id,
        bank_account_id: body.bank_account_id,
        numbering_mode: body.numbering_mode,
        next_number: body.next_number,
    };
    match service.register_sequence(req).await {
        Ok(ack) => (StatusCode::OK, Json(SequenceAck {
            sequence_id: ack.sequence_id,
            company_id: ack.company_id,
            bank_account_id: ack.bank_account_id,
            numbering_mode: ack.numbering_mode,
            next_number: ack.next_number,
        }))
            .into_response(),
        Err(e) => error_response(&e),
    }
}

async fn allocate(
    State(service): State<Arc<CheckPrintingService>>,
    Json(body): Json<AllocateBody>,
) -> impl IntoResponse {
    if tenant_mismatch(body.company_id) {
        return forbidden_tenant();
    }
    match service
        .allocate_check_numbers(body.company_id, body.bank_account_id, body.count)
        .await
    {
        Ok(ack) => (StatusCode::OK, Json(AllocationAck {
            first: ack.first,
            last: ack.last,
            numbers: ack.numbers,
        }))
            .into_response(),
        Err(e) => error_response(&e),
    }
}

async fn record_check(
    State(service): State<Arc<CheckPrintingService>>,
    Json(body): Json<RecordCheckBody>,
) -> impl IntoResponse {
    if tenant_mismatch(body.company_id) {
        return forbidden_tenant();
    }
    let req = RecordCheck {
        company_id: body.company_id,
        bank_account_id: body.bank_account_id,
        payment_id: body.payment_id,
        payment_number: body.payment_number,
        amount: body.amount,
        payee_name: body.payee_name,
        check_number: body.check_number,
        actor: body.actor,
    };
    match service.record_printed_check(req).await {
        Ok(ack) => (StatusCode::CREATED, Json(PrintedCheckAck {
            printed_check_id: ack.printed_check_id,
            company_id: ack.company_id,
            bank_account_id: ack.bank_account_id,
            payment_id: ack.payment_id,
            check_number: ack.check_number,
            status: ack.status,
        }))
            .into_response(),
        Err(e) => error_response(&e),
    }
}

async fn void_check(
    State(service): State<Arc<CheckPrintingService>>,
    Path(printed_check_id): Path<Uuid>,
    Json(body): Json<VoidBody>,
) -> impl IntoResponse {
    if tenant_mismatch(body.company_id) {
        return forbidden_tenant();
    }
    match service
        .void_printed_check(body.company_id, printed_check_id, body.actor)
        .await
    {
        Ok(ack) => (StatusCode::OK, Json(PrintedCheckAck {
            printed_check_id: ack.printed_check_id,
            company_id: ack.company_id,
            bank_account_id: ack.bank_account_id,
            payment_id: ack.payment_id,
            check_number: ack.check_number,
            status: ack.status,
        }))
            .into_response(),
        Err(e) => error_response(&e),
    }
}

/// The check printing routes (sequence registration + allocation + registry +
/// void).
pub fn create_check_printing_routes(service: Arc<CheckPrintingService>) -> Router {
    Router::new()
        .route("/accounting/checks/sequences", put(register_sequence))
        .route("/accounting/checks/allocate", post(allocate))
        .route("/accounting/checks", post(record_check))
        .route("/accounting/checks/:id/void", post(void_check))
        .with_state(service)
}
