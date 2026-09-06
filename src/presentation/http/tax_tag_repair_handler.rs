//! Tax-tag legal-change repair HTTP surface — hand-authored (user-owned; see
//! `metaphor.codegen.yaml`).
//!
//! Two routes over the `TaxTagRepairService`:
//! - `POST /accounting/tax-tag-repairs` — recompute tax-tag assignments on
//!   posted journal lines for a date window (dry-run or apply)
//! - `GET  /accounting/tax-tag-repairs` — the audit trail (most recent first)
//!
//! This is an OFFICER tool: hosts must mount it behind an accounting-officer
//! role gate (the same shape as the chart-install gate) and their ambient
//! company scope. It is never invoked by a scheduler — the module exposes no
//! path that would, and the guard set (reason mandatory, locked periods
//! refused, closed periods overridden explicitly, every run audit-stamped)
//! assumes a human making a legal-change correction.

use std::sync::Arc;

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::application::service::tax_tag_repair_service::{
    RepairReport, RepairRequest, RepairRunRow, TaxTagRepairError, TaxTagRepairService,
};

#[derive(Debug, Deserialize)]
pub struct TaxTagRuleBody {
    #[serde(default)]
    pub account_id: Option<Uuid>,
    #[serde(default)]
    pub is_tax_line: Option<bool>,
    pub tags: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct RepairBody {
    pub company_id: Uuid,
    pub date_from: NaiveDate,
    pub date_to: NaiveDate,
    pub rules: Vec<TaxTagRuleBody>,
    #[serde(default)]
    pub allow_closed_periods: bool,
    #[serde(default)]
    pub dry_run: bool,
    #[serde(default)]
    pub actor: Option<Uuid>,
    /// Mandatory legal-change justification.
    pub reason: String,
}

#[derive(Debug, Deserialize)]
pub struct ListRunsQuery {
    pub company_id: Uuid,
    #[serde(default = "default_limit")]
    pub limit: i64,
}

fn default_limit() -> i64 {
    25
}

#[derive(Debug, Serialize)]
struct ErrorBody {
    error: String,
    message: String,
}

fn error_response(e: &TaxTagRepairError) -> axum::response::Response {
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

async fn run_repair(
    State(service): State<Arc<TaxTagRepairService>>,
    Json(body): Json<RepairBody>,
) -> impl IntoResponse {
    if tenant_mismatch(body.company_id) {
        return forbidden_tenant();
    }
    let req = RepairRequest {
        company_id: body.company_id,
        date_from: body.date_from,
        date_to: body.date_to,
        rules: body
            .rules
            .into_iter()
            .map(|r| crate::application::service::tax_tag_repair_service::TaxTagRule {
                account_id: r.account_id,
                is_tax_line: r.is_tax_line,
                tags: r.tags,
            })
            .collect(),
        allow_closed_periods: body.allow_closed_periods,
        dry_run: body.dry_run,
        actor: body.actor,
        reason: body.reason,
    };
    match service.recompute(req).await {
        Ok(report) => (StatusCode::OK, Json(RepairReport {
            run_id: report.run_id,
            dry_run: report.dry_run,
            lines_examined: report.lines_examined,
            lines_retagged: report.lines_retagged,
            per_rule: report.per_rule,
            closed_periods_overridden: report.closed_periods_overridden,
        }))
            .into_response(),
        Err(e) => error_response(&e),
    }
}

async fn list_runs(
    State(service): State<Arc<TaxTagRepairService>>,
    Query(q): Query<ListRunsQuery>,
) -> impl IntoResponse {
    if tenant_mismatch(q.company_id) {
        return forbidden_tenant();
    }
    match service.list_runs(q.company_id, q.limit).await {
        Ok(rows) => {
            let body: Vec<RepairRunRow> = rows;
            (StatusCode::OK, Json(body)).into_response()
        }
        Err(e) => error_response(&e),
    }
}

/// The tax-tag repair routes — the POST is the officer verb, the GET the audit
/// read. Both belong behind the host's accounting-officer role gate.
pub fn create_tax_tag_repair_routes(service: Arc<TaxTagRepairService>) -> Router {
    Router::new()
        .route("/accounting/tax-tag-repairs", post(run_repair))
        .route("/accounting/tax-tag-repairs", get(list_runs))
        .with_state(service)
}
