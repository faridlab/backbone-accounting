//! Non-CRUD HTTP surface for financial statements (read-only reports).
//!
//! Hand-authored (user-owned; see `metaphor.codegen.yaml`). Wraps `ReportingService`.
//!   GET /accounting/reports/trial-balance?company_id=..&as_of=YYYY-MM-DD
//!   GET /accounting/reports/balance-sheet?company_id=..&as_of=YYYY-MM-DD
//!   GET /accounting/reports/income-statement?company_id=..&period_start=..&period_end=..

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

/// Read-only financial-statement report routes.
pub fn create_reporting_routes(service: Arc<ReportingService>) -> Router {
    Router::new()
        .route("/accounting/reports/trial-balance", get(trial_balance))
        .route("/accounting/reports/balance-sheet", get(balance_sheet))
        .route("/accounting/reports/income-statement", get(income_statement))
        .with_state(service)
}
