//! HTTP surface for registered charts — a read-only listing.
//!
//! The install VERB is deliberately NOT here: installing orchestrates across modules
//! (chart + tax templates + repartition routing) and is mounted by the composing
//! service, which owns the authority gate for it. The module only exposes what it
//! knows on its own: which datasets are registered.

use crate::application::service::chart_install_service::ChartInstallService;
use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use std::sync::Arc;

/// `GET /accounting/charts` — the registered chart datasets (code, version, size).
pub async fn list_charts(
    State(service): State<Arc<ChartInstallService>>,
) -> Json<Vec<crate::application::service::chart_install_service::ChartInfo>> {
    Json(service.list_charts())
}

/// Chart routes: read-only. Merge into the guarded composition.
pub fn create_chart_routes(service: Arc<ChartInstallService>) -> Router {
    Router::new()
        .route("/accounting/charts", get(list_charts))
        .with_state(service)
}
