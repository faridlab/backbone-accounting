//! EMV(QRCPS)/QRIS display HTTP surface — hand-authored (user-owned; see
//! `metaphor.codegen.yaml`).
//!
//! Two routes over the `EmvQrService`:
//! - `PUT  /accounting/emv-qr/config`   — write/replace the company's QR
//!   display configuration (validated through the domain builder first)
//! - `GET  /accounting/emv-qr/payload`  — render the merchant-presented payload
//!   for one invoice display (amount / currency / reference as query params —
//!   invoice fields live in the billing module and arrive here already read)
//!
//! The module default-deny posture applies: hosts mount these behind their own
//! auth + role layers; the tenant-consistency check below refuses a body/query
//! company that disagrees with an ambient company scope.

use std::sync::Arc;

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, put},
    Json, Router,
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::application::service::emv_qr_service::{
    EmvPayloadAck, EmvQrConfigAck, EmvQrConfigInput, EmvQrService, EmvQrServiceError,
};

#[derive(Debug, Deserialize)]
pub struct EmvQrConfigBody {
    pub company_id: Uuid,
    #[serde(default)]
    pub bank_account_id: Option<Uuid>,
    pub merchant_name: String,
    pub merchant_city: String,
    #[serde(default = "default_country")]
    pub country: String,
    pub mcc: String,
    #[serde(default = "default_gui")]
    pub gui: String,
    pub merchant_identifier: String,
    #[serde(default = "default_currency")]
    pub currency: String,
    #[serde(default = "default_initiation")]
    pub initiation: String,
}

fn default_country() -> String {
    "ID".into()
}
fn default_gui() -> String {
    "ID.CO.QRIS.WWW".into()
}
fn default_currency() -> String {
    "IDR".into()
}
fn default_initiation() -> String {
    "11".into()
}

#[derive(Debug, Deserialize)]
pub struct EmvQrPayloadQuery {
    pub company_id: Uuid,
    #[serde(default)]
    pub bank_account_id: Option<Uuid>,
    /// Invoice residual amount; omit for a static QR without tag 54.
    pub amount: Option<Decimal>,
    /// Currency override (ISO-4217 alpha); defaults to the configured one.
    pub currency: Option<String>,
    /// Invoice number/reference (tag 62-01).
    pub reference: Option<String>,
}

#[derive(Debug, Serialize)]
struct ErrorBody {
    error: String,
    message: String,
}

fn error_response(e: &EmvQrServiceError) -> axum::response::Response {
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

// ── Tenant consistency ────────────────────────────────────────────────────────
// Same contract as the reconciliation verbs: when a host has mounted an ambient
// company scope, the request's company must agree with it.

fn tenant_mismatch(req_company: Uuid) -> bool {
    match backbone_orm::current_company() {
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

async fn upsert_config(
    State(service): State<Arc<EmvQrService>>,
    Json(body): Json<EmvQrConfigBody>,
) -> impl IntoResponse {
    if tenant_mismatch(body.company_id) {
        return forbidden_tenant();
    }
    let input = EmvQrConfigInput {
        company_id: body.company_id,
        bank_account_id: body.bank_account_id,
        merchant_name: body.merchant_name,
        merchant_city: body.merchant_city,
        country: body.country,
        mcc: body.mcc,
        gui: body.gui,
        merchant_identifier: body.merchant_identifier,
        currency: body.currency,
        initiation: body.initiation,
    };
    match service.upsert_config(input).await {
        Ok(ack) => (
            StatusCode::OK,
            Json(EmvQrConfigAck {
                config_id: ack.config_id,
                company_id: ack.company_id,
                bank_account_id: ack.bank_account_id,
            }),
        )
            .into_response(),
        Err(e) => error_response(&e),
    }
}

async fn payload(
    State(service): State<Arc<EmvQrService>>,
    Query(q): Query<EmvQrPayloadQuery>,
) -> impl IntoResponse {
    if tenant_mismatch(q.company_id) {
        return forbidden_tenant();
    }
    match service
        .invoice_payload(
            q.company_id,
            q.bank_account_id,
            q.amount,
            q.currency,
            q.reference,
        )
        .await
    {
        Ok(ack) => (StatusCode::OK, Json(EmvPayloadAck {
            payload: ack.payload,
            currency: ack.currency,
            initiation_method: ack.initiation_method,
        }))
            .into_response(),
        Err(e) => error_response(&e),
    }
}

/// The EMV QR display routes. Hosts with auth middleware wrap these handlers
/// with their own layers; the config verb additionally belongs behind an
/// accounting-officer role gate at the host.
pub fn create_emv_qr_routes(service: Arc<EmvQrService>) -> Router {
    Router::new()
        .route("/accounting/emv-qr/config", put(upsert_config))
        .route("/accounting/emv-qr/payload", get(payload))
        .with_state(service)
}
