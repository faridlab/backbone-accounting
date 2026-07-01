//! Guarded route composition — the RECOMMENDED way to mount the accounting module.
//!
//! Hand-authored (user-owned; see `metaphor.codegen.yaml`). Closes the CRUD-bypass the council
//! flagged: the generated `routes()` exposes full mutable CRUD (POST/PATCH/upsert/bulk) on every
//! entity, including the posted GL records — which lets a caller write an unbalanced posting or
//! PATCH a posted ledger row, bypassing double-entry validation entirely.
//!
//! Here the **posted GL entities (Journal, JournalLine, Ledger, AccountingPost) are READ-ONLY**
//! over HTTP; the only sanctioned writer is `PostingService` (`POST /accounting/posts`). Master /
//! configuration entities keep full CRUD.

use axum::Router;

use crate::AccountingModule;

use super::{
    create_account_routes, create_accounting_post_read_routes, create_cost_center_routes,
    create_financial_statement_routes, create_fiscal_period_routes, create_journal_line_read_routes,
    create_journal_read_routes, create_ledger_read_routes, create_reconciliation_item_routes,
    create_reconciliation_routes,
};

/// Mount the accounting module with the posted-GL write paths locked to `PostingService`.
/// Prefer this over `AccountingModule::routes()` for any real deployment.
pub fn create_guarded_accounting_routes(m: &AccountingModule) -> Router {
    Router::new()
        // Master / configuration data — full CRUD is appropriate here.
        .merge(create_account_routes(m.account_service.clone()))
        .merge(create_cost_center_routes(m.cost_center_service.clone()))
        .merge(create_fiscal_period_routes(m.fiscal_period_service.clone()))
        .merge(create_financial_statement_routes(m.financial_statement_service.clone()))
        .merge(create_reconciliation_routes(m.reconciliation_service.clone()))
        .merge(create_reconciliation_item_routes(m.reconciliation_item_service.clone()))
        // Posted GL — READ ONLY. Writes flow only through the GL-posting contract.
        .merge(create_journal_read_routes(m.journal_service.clone()))
        .merge(create_journal_line_read_routes(m.journal_line_service.clone()))
        .merge(create_ledger_read_routes(m.ledger_service.clone()))
        .merge(create_accounting_post_read_routes(m.accounting_post_service.clone()))
}
