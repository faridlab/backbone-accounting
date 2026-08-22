//! Non-CRUD HTTP surface for financial reports (read-only, computed on the fly).
//!
//! Hand-authored (user-owned; see `metaphor.codegen.yaml`). Wraps `ReportingService`.
//!   GET /accounting/reports/trial-balance?company_id=..&as_of=YYYY-MM-DD
//!   GET /accounting/reports/balance-sheet?company_id=..&as_of=YYYY-MM-DD
//!   GET /accounting/reports/income-statement?company_id=..&period_start=..&period_end=..
//!   GET /accounting/reports/general-ledger?company_id=..&to_date=..&from_date=..&account_id=..&limit=..&offset=..
//!   GET /accounting/reports/partner-ledger?company_id=..&party_type=customer|supplier&party_id=..&as_of=..
//!   GET /accounting/reports/aged-receivables?company_id=..&as_of=..
//!   GET /accounting/reports/aged-payables?company_id=..&as_of=..

use std::sync::Arc;

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use chrono::NaiveDate;
use serde::Deserialize;
use uuid::Uuid;

use crate::application::service::reporting_service::ReportingService;

#[derive(Debug, Deserialize)]
pub struct AsOfQuery {
    pub company_id: Uuid,
    pub as_of: NaiveDate,
}

#[derive(Debug, Deserialize)]
pub struct PeriodQuery {
    pub company_id: Uuid,
    pub period_start: NaiveDate,
    pub period_end: NaiveDate,
}

#[derive(Debug, Deserialize)]
pub struct GeneralLedgerQuery {
    pub company_id: Uuid,
    pub to_date: NaiveDate,
    pub from_date: Option<NaiveDate>,
    pub account_id: Option<Uuid>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct PartnerLedgerQuery {
    pub company_id: Uuid,
    pub party_type: String,
    pub party_id: Uuid,
    pub as_of: NaiveDate,
}

fn err(e: anyhow::Error) -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(serde_json::json!({ "success": false, "error": e.to_string() })),
    )
}

async fn trial_balance(
    State(svc): State<Arc<ReportingService>>,
    Query(q): Query<AsOfQuery>,
) -> impl IntoResponse {
    match svc.trial_balance(q.company_id, q.as_of).await {
        Ok(r) => (StatusCode::OK, Json(serde_json::to_value(r).unwrap())),
        Err(e) => err(e),
    }
}

async fn balance_sheet(
    State(svc): State<Arc<ReportingService>>,
    Query(q): Query<AsOfQuery>,
) -> impl IntoResponse {
    match svc.balance_sheet(q.company_id, q.as_of).await {
        Ok(r) => (StatusCode::OK, Json(serde_json::to_value(r).unwrap())),
        Err(e) => err(e),
    }
}

async fn income_statement(
    State(svc): State<Arc<ReportingService>>,
    Query(q): Query<PeriodQuery>,
) -> impl IntoResponse {
    match svc
        .income_statement(q.company_id, q.period_start, q.period_end)
        .await
    {
        Ok(r) => (StatusCode::OK, Json(serde_json::to_value(r).unwrap())),
        Err(e) => err(e),
    }
}

async fn general_ledger(
    State(svc): State<Arc<ReportingService>>,
    Query(q): Query<GeneralLedgerQuery>,
) -> impl IntoResponse {
    match svc
        .general_ledger(
            q.company_id,
            q.account_id,
            q.from_date,
            q.to_date,
            q.limit,
            q.offset.unwrap_or(0),
        )
        .await
    {
        Ok(r) => (StatusCode::OK, Json(serde_json::to_value(r).unwrap())),
        Err(e) => err(e),
    }
}

async fn partner_ledger(
    State(svc): State<Arc<ReportingService>>,
    Query(q): Query<PartnerLedgerQuery>,
) -> impl IntoResponse {
    // Validate up front: an unknown party_type would otherwise surface as a 500
    // carrying the raw database cast error (the swallowed-enum-detail failure class).
    if !matches!(q.party_type.as_str(), "customer" | "supplier") {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "success": false,
                "error": format!(
                    "party_type must be 'customer' or 'supplier', got {:?}",
                    q.party_type
                ),
            })),
        );
    }
    match svc
        .partner_ledger(q.company_id, &q.party_type, q.party_id, q.as_of)
        .await
    {
        Ok(r) => (StatusCode::OK, Json(serde_json::to_value(r).unwrap())),
        Err(e) => err(e),
    }
}

async fn aged_receivables(
    State(svc): State<Arc<ReportingService>>,
    Query(q): Query<AsOfQuery>,
) -> impl IntoResponse {
    match svc.aged_receivables(q.company_id, q.as_of).await {
        Ok(r) => (StatusCode::OK, Json(serde_json::to_value(r).unwrap())),
        Err(e) => err(e),
    }
}

async fn aged_payables(
    State(svc): State<Arc<ReportingService>>,
    Query(q): Query<AsOfQuery>,
) -> impl IntoResponse {
    match svc.aged_payables(q.company_id, q.as_of).await {
        Ok(r) => (StatusCode::OK, Json(serde_json::to_value(r).unwrap())),
        Err(e) => err(e),
    }
}

/// Read-only financial-report routes.
pub fn create_reporting_routes(service: Arc<ReportingService>) -> Router {
    Router::new()
        .route("/accounting/reports/trial-balance", get(trial_balance))
        .route("/accounting/reports/balance-sheet", get(balance_sheet))
        .route(
            "/accounting/reports/income-statement",
            get(income_statement),
        )
        .route("/accounting/reports/general-ledger", get(general_ledger))
        .route("/accounting/reports/partner-ledger", get(partner_ledger))
        .route(
            "/accounting/reports/aged-receivables",
            get(aged_receivables),
        )
        .route("/accounting/reports/aged-payables", get(aged_payables))
        .with_state(service)
}
