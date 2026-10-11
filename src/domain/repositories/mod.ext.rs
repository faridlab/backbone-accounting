// This directory's extension (hand-written; ADR-0031). The generated mod.rs (lib.rs for
// src/) pulls it in with include!, so its lines resolve against this directory.

// Hand-authored ports for the GL-posting contract (not schema-derived).
pub mod bank_reconciliation_repository;
pub mod budget_control;
pub mod chart_install_repository;
pub mod deferred_tax;
pub mod hierarchy_repository;
pub mod journal_workflow_repository;
pub mod period_close_repository;
pub mod posting_repository;
pub mod reconcile_graph_repository;
pub mod reporting_repository;
pub use bank_reconciliation_repository::{
    BankReconciliationRepository, BookEntryRow, MatchedPair, ReconciliationCommit, UnmatchedBook,
    UnmatchedStatement,
};
pub use chart_install_repository::{
    ChartAccountRow, ChartInstallRepository, OverlappingAccount, UpsertOutcome,
};
pub use budget_control::{BudgetBreach, BudgetControlPort, BudgetEnforcement};
pub use deferred_tax::{DeferredTaxLine, DeferredTaxLookup};
pub use hierarchy_repository::{HierarchyNode, HierarchyRepository, HierarchyTable};
pub use journal_workflow_repository::{JournalAuthors, JournalStatusRow, JournalWorkflowRepository};
pub use period_close_repository::{PeriodCloseRepository, PeriodRow, PlBalanceRow};
pub use posting_repository::{
    FailedPost, LedgerEntryInput, ManualJournalCommit, ManualJournalForPost, PostableAccount,
    PostingCommit, PostingRepository, PostingWrite, ReversalSource,
};
pub use reconcile_graph_repository::{JournalReversalMeta, ReconcileGraphRepository};
pub use reporting_repository::{AccountSumRow, ReportingRepository};
