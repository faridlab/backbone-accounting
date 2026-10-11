// This directory's extension (hand-written; ADR-0031). The generated mod.rs (lib.rs for
// src/) pulls it in with include!, so its lines resolve against this directory.

pub mod accounting_ops_handler;
pub mod chart_routes;
pub mod check_printing_handler;
pub mod emv_qr_handler;
pub mod guarded_routes;
pub mod hierarchy_handler;
pub mod journal_workflow_handler;
pub mod posting_handler;
pub mod reconcile_handler;
pub mod reporting_handler;
pub mod tax_tag_repair_handler;
pub use accounting_ops_handler::{
    create_bank_reconciliation_routes, create_period_close_routes, create_period_finality_routes,
    ClosePeriodBody, PeriodReasonBody,
};
pub use chart_routes::create_chart_routes;
pub use guarded_routes::create_guarded_accounting_routes;
pub use hierarchy_handler::create_hierarchy_routes;
pub use journal_workflow_handler::create_journal_workflow_routes;
pub use posting_handler::{
    create_posting_routes, PostingLineDto, PostingRequestDto, PostingResponse,
};
pub use reconcile_handler::create_reconcile_verb_routes;
pub use reporting_handler::create_reporting_routes;
