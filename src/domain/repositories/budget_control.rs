//! Budget-control port for the GL-posting chokepoint.
//!
//! Accounting owns the posting chokepoint but cannot know budget coverage: a
//! budget is a plan owned by the budget module (positions keyed account x
//! cost center x fiscal period), while the posting contract sees only lines.
//! The deployed app sees both modules and implements this port — a host-side
//! ACL, not a module edge. `None` (unwired) simply means no budget check:
//! accounting keeps working for hosts without a budget module.
//!
//! Wiring follows the deferred-tax lookup pattern: the port is Option-wired
//! through the module builder, the default is absent, and hosts that compose
//! a budget module inject an adapter. One deliberate deviation from the
//! ReconcileSink/DeferredTaxLookup shape is documented on the trait: this
//! port is NOT conn-taking. Posting validation holds no connection (the
//! `PostingService` is pool-free; persistence sits behind the repository
//! port) and the control is a snapshot read, not a write that must commit
//! atomically with the caller's transaction. The check-then-post race is
//! accepted snapshot semantics — two concurrent in-budget postings may
//! jointly overshoot a budget, the same class as any pre-commit validation
//! read. If that ever becomes unacceptable the check must move inside the
//! posting commit transaction, which is a repository-layer change, not a
//! port-signature change.

use async_trait::async_trait;
use chrono::NaiveDate;
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::domain::gl_posting::PostingLine;

/// What the budget says to do when a position is exceeded. Local to accounting
/// so this crate never imports the budget module's enum; adapters map values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BudgetEnforcement {
    /// Record the breach (structured warning) and let the posting through.
    Warn,
    /// Refuse the posting.
    Block,
}

/// One budget position this posting would push over its plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BudgetBreach {
    pub budget_id: Uuid,
    pub budget_line_id: Uuid,
    pub account_id: Uuid,
    /// `None` = postings carrying no cost center (exact-key matching).
    pub cost_center_id: Option<Uuid>,
    pub fiscal_period_id: Uuid,
    pub planned_amount: Decimal,
    /// Net normal-direction movement already committed in the ledger for this
    /// position before this posting.
    pub achieved_amount: Decimal,
    /// This posting's own normal-direction contribution to the position.
    pub pending_amount: Decimal,
    pub enforcement: BudgetEnforcement,
}

/// Host-implemented budget control for the posting chokepoint.
///
/// Contract:
/// - Every read is scoped to `company_id`.
/// - Lines are matched by exact `(account, cost_center, period-covering
///   posting_date)` key; a `None` cost center matches only positions whose
///   cost center is also `None` (never an aggregate rollup).
/// - Only confirmed/active budgets participate; draft, closed, or cancelled
///   plans are invisible to the control.
/// - Achieved amounts are committed ledger movements through the posting
///   date, oriented by the account's normal balance; reversals reduce them.
/// - An empty result means within budget or no coverage — the posting
///   proceeds. An `Err` means the check itself is broken; the chokepoint
///   treats that as an internal failure and refuses the posting (a wired
///   budget module must not silently disable enforcement).
#[async_trait]
pub trait BudgetControlPort: Send + Sync {
    /// Confirmed-budget positions this posting would exceed, with the
    /// achieved/pending/planned amounts that drove the breach.
    async fn evaluate_posting(
        &self,
        company_id: Uuid,
        posting_date: NaiveDate,
        lines: &[PostingLine],
    ) -> anyhow::Result<Vec<BudgetBreach>>;
}
