//! Reconciliation-graph contract types (domain layer).
//!
//! These are the domain shapes shared by the `ReconcileGraphRepository` port
//! (`domain/repositories/reconcile_graph_repository.rs`), the pure validation rules
//! (`domain/services/reconcile_rules.rs`), and the application `ReconcileWriteService`.
//! They mirror the wire contract in `backbone-gl-posting` (the `ReconcileSink` producers
//! settle and clear through) — the host maps wire → domain at the seam.
//!
//! Model: `PartialReconcile` edges (debit↔credit over journal lines, partial amounts in
//! COMPANY currency) + `FullReconcile` groups. Residual is COMPUTED (base line amount minus
//! the partial amounts applied on either side), never stored; the matching number is a READ
//! (stored `full_reconcile_id` for complete groups, a label derived from the component's
//! minimum partial id otherwise). Unlinking is side-effecting — see the write service.

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use uuid::Uuid;

/// Locates one side of an edge by producer identity (mirrors the wire `ReconcileLine`).
/// Resolves to exactly one posted journal line on the shared reconcilable account.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineLocator {
    /// Producer discriminator stamped on the journal line — e.g. "order" (sales invoice),
    /// "expense" (purchase invoice), "payment", "settlement" (bank clearance), "manual".
    pub source_type: String,
    /// The producer document id the journal was posted from.
    pub source_id: Uuid,
    /// The reconcilable account both lines of the pair sit on.
    pub account_id: Uuid,
    /// `true` resolves to the reversal journal of the source (`journals.is_reversing`).
    pub reversing: bool,
}

impl LineLocator {
    pub fn new(source_type: &str, source_id: Uuid, account_id: Uuid) -> Self {
        Self { source_type: source_type.to_string(), source_id, account_id, reversing: false }
    }
    pub fn reversing(mut self) -> Self {
        self.reversing = true;
        self
    }
}

/// What created an edge — stamped on the partial for traceability.
pub const ORIGIN_SETTLEMENT: &str = "settlement";
pub const ORIGIN_CLEARING: &str = "clearing";
pub const ORIGIN_MANUAL: &str = "manual";

/// A loaded journal line with everything the reconciliation rules and write path need.
/// `FOR UPDATE`-locked by the repository when loaded for writing.
#[derive(Debug, Clone)]
pub struct ReconcileLineSnapshot {
    pub id: Uuid,
    pub journal_id: Uuid,
    pub company_id: Uuid,
    pub account_id: Uuid,
    /// `accounts.account_subtype` (joined) — drives the settlement-dimension guard.
    pub account_subtype: String,
    pub party_type: Option<String>,
    pub party_id: Option<Uuid>,
    /// Document-currency amounts (one of the two is zero on a clean line).
    pub debit_amount: Decimal,
    pub credit_amount: Decimal,
    pub currency: String,
    /// Document → company currency rate at posting time.
    pub exchange_rate: Decimal,
    pub transaction_date: NaiveDate,
    pub is_posted: bool,
    pub journal_status: String,
    pub journal_is_reversing: bool,
    pub source_type: Option<String>,
    pub source_id: Option<Uuid>,
    pub is_reconciled: bool,
    pub full_reconcile_id: Option<Uuid>,
    /// Company-currency face amount (`base_debit_amount + base_credit_amount`); equals the
    /// document-currency face while the books are single-currency.
    pub base_amount: Decimal,
}

/// A partial edge to insert.
#[derive(Debug, Clone)]
pub struct NewPartial {
    pub company_id: Uuid,
    pub debit_move_id: Uuid,
    pub credit_move_id: Uuid,
    /// Company currency, strictly positive (the write service clamps before inserting).
    pub amount: Decimal,
    pub currency: String,
    pub max_date: NaiveDate,
    pub origin: String,
    pub source_type: Option<String>,
    pub source_id: Option<Uuid>,
    pub metadata: serde_json::Value,
}

/// A loaded partial edge.
#[derive(Debug, Clone)]
pub struct PartialRow {
    pub id: Uuid,
    pub company_id: Uuid,
    pub debit_move_id: Uuid,
    pub credit_move_id: Uuid,
    pub amount: Decimal,
    pub max_date: NaiveDate,
    pub origin: String,
    pub full_reconcile_id: Option<Uuid>,
    pub exchange_move_id: Option<Uuid>,
    pub source_type: Option<String>,
    pub source_id: Option<Uuid>,
}

/// Account facts the guards need.
#[derive(Debug, Clone)]
pub struct AccountReconcileFlags {
    pub is_reconcilable: bool,
    pub subtype: String,
}

/// Outcome of locator resolution: exactly one line, or a typed failure.
#[derive(Debug)]
pub enum LocatorResolution {
    One(ReconcileLineSnapshot),
    NotFound,
    Ambiguous(usize),
}

/// The matching-group READ: the connected component of a line, with the derived label.
#[derive(Debug, Clone)]
pub struct MatchingGroup {
    /// "F-<uuid8>" for a stored full-reconcile group, "P-<uuid8>" derived from the
    /// component's minimum partial id while the chain is still partial, "-" for a bare line.
    pub label: String,
    pub full_reconcile_id: Option<Uuid>,
    pub line_ids: Vec<Uuid>,
    pub partial_ids: Vec<Uuid>,
    /// Per-line company-currency residual.
    pub residuals: Vec<(Uuid, Decimal)>,
}

impl MatchingGroup {
    pub fn residual_of(&self, line_id: Uuid) -> Option<Decimal> {
        self.residuals.iter().find(|(id, _)| *id == line_id).map(|(_, r)| *r)
    }
}

/// One open AR/AP line for the party-aging read.
#[derive(Debug, Clone)]
pub struct PartyResidual {
    pub line_id: Uuid,
    pub journal_id: Uuid,
    pub journal_number: String,
    pub transaction_date: NaiveDate,
    pub source_reference: Option<String>,
    pub residual: Decimal,
    pub currency: String,
    pub is_reconciled: bool,
}

/// Typed reconciliation failure. `code()` is the stable error string asserted by the golden
/// cases; carries no `sqlx` type (the domain stays persistence-agnostic).
#[derive(Debug)]
pub enum ReconcileError {
    LineNotFound,
    AmbiguousLocator(usize),
    SameCompanyRequired,
    SameAccountRequired,
    CurrencyMismatch(String, String),
    AccountNotReconcilable,
    DirectionMismatch,
    LineNotPosted,
    PartyMismatch,
    ExchangeAccountUnconfigured,
    PeriodClosed,
    Conflict(String),
    Internal(String),
}

impl ReconcileError {
    pub fn code(&self) -> &'static str {
        match self {
            ReconcileError::LineNotFound => "line_not_found",
            ReconcileError::AmbiguousLocator(_) => "ambiguous_locator",
            ReconcileError::SameCompanyRequired => "same_company_required",
            ReconcileError::SameAccountRequired => "same_account_required",
            ReconcileError::CurrencyMismatch(..) => "currency_mismatch",
            ReconcileError::AccountNotReconcilable => "account_not_reconcilable",
            ReconcileError::DirectionMismatch => "direction_mismatch",
            ReconcileError::LineNotPosted => "line_not_posted",
            ReconcileError::PartyMismatch => "party_mismatch",
            ReconcileError::ExchangeAccountUnconfigured => "exchange_account_unconfigured",
            ReconcileError::PeriodClosed => "period_closed",
            ReconcileError::Conflict(_) => "conflict",
            ReconcileError::Internal(_) => "internal_error",
        }
    }

    /// HTTP status: missing line → 404, ambiguity/conflict → 409, guards → 422, internal → 500.
    pub fn http_status(&self) -> u16 {
        match self {
            ReconcileError::LineNotFound => 404,
            ReconcileError::AmbiguousLocator(_) | ReconcileError::Conflict(_) => 409,
            ReconcileError::Internal(_) => 500,
            _ => 422,
        }
    }
}

impl std::fmt::Display for ReconcileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.code())
    }
}
impl std::error::Error for ReconcileError {}

/// Result of a reconcile verb. `partial_id` is `None` when the amount clamped to zero
/// (on-account no-op — no edge exists). `full_reconcile_id` is set when this edge
/// completed a group.
#[derive(Debug, Clone)]
pub struct EdgeOutcome {
    pub partial_id: Option<Uuid>,
    pub applied: Decimal,
    pub full_reconcile_id: Option<Uuid>,
}

/// Request shape for the pair verbs (application layer).
#[derive(Debug, Clone)]
pub struct PairRequest {
    pub company_id: Uuid,
    pub debit: LineLocator,
    pub credit: LineLocator,
    pub amount: Decimal,
    pub origin: String,
    pub actor: Option<Uuid>,
}

/// The timestamp shape used for group stamping.
pub type ReconciledAt = DateTime<Utc>;
