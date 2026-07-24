//! GL-posting contract types (domain layer).
//!
//! These are the domain shapes shared by the `PostingRepository` port
//! (`domain/repositories/posting_repository.rs`), the pure validation rules
//! (`domain/services/posting_rules.rs`), and the application `PostingService`.
//! Living in the domain layer means neither the port nor the rules depend on the
//! application or infrastructure layers — and `PostingError` carries no `sqlx`
//! type, so the domain stays persistence-agnostic.

use chrono::{DateTime, Utc, NaiveDate};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// One debit/credit line of a posting request. Exactly one of `debit`/`credit` is > 0.
#[derive(Debug, Clone)]
pub struct PostingLine {
    pub account_id: Uuid,
    pub debit: Decimal,
    pub credit: Decimal,
    pub party_type: Option<String>, // "customer" | "supplier" | "employee"
    pub party_id: Option<Uuid>,
    pub cost_center_id: Option<Uuid>,
    pub project_id: Option<Uuid>,
    pub department_id: Option<Uuid>,
    pub description: Option<String>,
}

/// A request to record a balanced set of lines in the GL (the inbound contract shape).
#[derive(Debug, Clone)]
pub struct PostingRequest {
    pub company_id: Uuid,
    pub branch_id: Option<Uuid>,
    pub source_type: String, // posting_source_type: order|payment|settlement|refund|expense|inventory|manual
    pub source_id: Uuid,
    pub source_reference: Option<String>,
    pub posting_date: NaiveDate,
    pub currency: String,
    pub posting_type: String, // "original" | "reversal"
    pub reverses_post_id: Option<Uuid>,
    pub description: Option<String>,
    pub lines: Vec<PostingLine>,
    /// The REAL dedup key when set: two posts with the same `(company_id, idempotency_key)` collapse
    /// to one, and the producer may reuse `source_id` across its several posts. When `None`, dedup
    /// falls back to the tuple `(company_id, source_type, source_id, posting_type)`.
    pub idempotency_key: Option<String>,
}

impl PostingRequest {
    /// Convenience constructor for an original posting.
    pub fn original(company_id: Uuid, source_type: &str, source_id: Uuid, posting_date: NaiveDate) -> Self {
        Self {
            company_id,
            branch_id: None,
            source_type: source_type.to_string(),
            source_id,
            source_reference: None,
            posting_date,
            currency: "IDR".to_string(),
            posting_type: "original".to_string(),
            reverses_post_id: None,
            description: None,
            lines: Vec::new(),
            idempotency_key: None,
        }
    }

    /// Set the idempotency key (the real per-post dedup identity).
    pub fn with_idempotency_key(mut self, key: impl Into<String>) -> Self {
        self.idempotency_key = Some(key.into());
        self
    }
}

/// Outcome of a successful post.
#[derive(Debug, Clone)]
pub struct PostingResult {
    pub post_id: Uuid,
    pub journal_id: Uuid,
    pub posting_status: String,
    /// True when an existing posted entry was returned instead of writing a new one.
    pub idempotent_reuse: bool,
}

/// Typed posting failure. `code()` is the stable error string asserted by the golden cases.
/// Deliberately carries NO `sqlx::Error` — the domain layer is persistence-agnostic; the
/// infrastructure adapter maps storage errors to `PostingError::Internal`.
#[derive(Debug)]
pub enum PostingError {
    TooFewLines,
    Unbalanced,
    /// A line's debit/credit are not valid: negative, both non-zero, or both zero.
    /// A clean line has exactly one side strictly positive and both non-negative (R1a).
    InvalidLineAmount,
    NonPostableAccount(String),
    AccountNotFound(Uuid),
    PartyRequired(String),
    PartyNotAllowed(String),
    PeriodClosed,
    Conflict(String),
    Internal(String),
}

impl PostingError {
    pub fn code(&self) -> &'static str {
        match self {
            PostingError::TooFewLines => "too_few_lines",
            PostingError::Unbalanced => "unbalanced",
            PostingError::InvalidLineAmount => "invalid_line_amount",
            PostingError::NonPostableAccount(_) => "non_postable_account",
            PostingError::AccountNotFound(_) => "account_not_found",
            PostingError::PartyRequired(_) => "party_required",
            PostingError::PartyNotAllowed(_) => "party_not_allowed",
            PostingError::PeriodClosed => "period_closed",
            PostingError::Conflict(_) => "conflict",
            PostingError::Internal(_) => "internal_error",
        }
    }

    /// HTTP status: validation → 422, missing account → 404, internal → 500.
    pub fn http_status(&self) -> u16 {
        match self {
            PostingError::AccountNotFound(_) => 404,
            PostingError::Internal(_) => 500,
            _ => 422,
        }
    }
}

impl std::fmt::Display for PostingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.code())
    }
}
impl std::error::Error for PostingError {}

/// Sink for GL-posting domain events (the event-bus seam). Fire-and-forget. A real adapter
/// (e.g. backbone-messaging) implements this; tests use a recording sink; the default logs.
pub trait PostingEventSink: Send + Sync {
    fn publish(&self, event: PostingEvent);
}

/// Published when the GL-posting service records a balanced entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountingPostPosted {
    pub post_id: Uuid,
    pub journal_id: Uuid,
    pub company_id: Uuid,
    pub source_type: String,
    pub source_id: Uuid,
    pub total_debit: Decimal,
    pub total_credit: Decimal,
    pub occurred_at: DateTime<Utc>,
}

/// Published when a posting is rejected (validation failure). Carries the stable error code.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountingPostFailed {
    pub company_id: Uuid,
    pub source_type: String,
    pub source_id: Uuid,
    pub error_code: String,
    pub error_message: String,
    pub occurred_at: DateTime<Utc>,
}

/// GL-posting domain events (discriminated union) for the module event bus.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum PostingEvent {
    AccountingPostPosted(AccountingPostPosted),
    AccountingPostFailed(AccountingPostFailed),
}

/// Map a posting source_type + posting_type to (journal_type, journal_source) enum labels.
pub fn map_source(source_type: &str, posting_type: &str) -> (&'static str, &'static str) {
    if posting_type == "reversal" {
        return ("reversing", "adjustment");
    }
    match source_type {
        "order" => ("sales", "order"),
        "payment" => ("cash_receipt", "payment"),
        "settlement" => ("general", "settlement"),
        "refund" => ("general", "adjustment"),
        "expense" => ("purchase", "adjustment"),
        "inventory" => ("general", "system"),
        _ => ("general", "manual"),
    }
}
