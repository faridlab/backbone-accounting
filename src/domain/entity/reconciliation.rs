use chrono::{DateTime, Utc, NaiveDate};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use rust_decimal::Decimal;

use super::ReconciliationType;
use super::ReconciliationStatus;
use super::AuditMetadata;

/// Strongly-typed ID for Reconciliation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ReconciliationId(pub Uuid);

impl ReconciliationId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for ReconciliationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for ReconciliationId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for ReconciliationId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<ReconciliationId> for Uuid {
    fn from(id: ReconciliationId) -> Self { id.0 }
}

impl AsRef<Uuid> for ReconciliationId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for ReconciliationId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Reconciliation {
    pub id: Uuid,
    pub company_id: Uuid,
    pub reconciliation_number: String,
    pub account_id: Uuid,
    pub account_number: String,
    pub account_name: String,
    pub reconciliation_type: ReconciliationType,
    pub period_start: NaiveDate,
    pub period_end: NaiveDate,
    pub statement_date: NaiveDate,
    pub opening_book_balance: Decimal,
    pub opening_statement_balance: Decimal,
    pub previous_reconciliation_id: Option<Uuid>,
    pub closing_book_balance: Decimal,
    pub closing_statement_balance: Decimal,
    pub total_matched_debits: Decimal,
    pub total_matched_credits: Decimal,
    pub matched_count: i32,
    pub total_unmatched_book_debits: Decimal,
    pub total_unmatched_book_credits: Decimal,
    pub unmatched_book_count: i32,
    pub total_unmatched_statement_debits: Decimal,
    pub total_unmatched_statement_credits: Decimal,
    pub unmatched_statement_count: i32,
    pub outstanding_deposits: Decimal,
    pub outstanding_checks: Decimal,
    pub deposits_in_transit: Decimal,
    pub bank_charges: Decimal,
    pub bank_interest: Decimal,
    pub nsf_checks: Decimal,
    pub other_adjustments: Decimal,
    pub adjusted_book_balance: Option<Decimal>,
    pub adjusted_statement_balance: Option<Decimal>,
    pub difference: Decimal,
    pub is_balanced: bool,
    pub status: ReconciliationStatus,
    pub started_at: Option<DateTime<Utc>>,
    pub started_by: Option<Uuid>,
    pub completed_at: Option<DateTime<Utc>>,
    pub completed_by: Option<Uuid>,
    pub reviewed_at: Option<DateTime<Utc>>,
    pub reviewed_by: Option<Uuid>,
    pub has_adjusting_entries: bool,
    pub adjusting_journal_ids: serde_json::Value,
    pub statement_source: Option<String>,
    pub statement_file_url: Option<String>,
    pub import_date: Option<DateTime<Utc>>,
    pub notes: Option<String>,
    pub discrepancy_notes: Option<String>,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl Reconciliation {
    /// Create a builder for Reconciliation
    pub fn builder() -> ReconciliationBuilder {
        ReconciliationBuilder::default()
    }

    /// Create a new Reconciliation with required fields
    pub fn new(company_id: Uuid, reconciliation_number: String, account_id: Uuid, account_number: String, account_name: String, reconciliation_type: ReconciliationType, period_start: NaiveDate, period_end: NaiveDate, statement_date: NaiveDate, opening_book_balance: Decimal, opening_statement_balance: Decimal, closing_book_balance: Decimal, closing_statement_balance: Decimal, total_matched_debits: Decimal, total_matched_credits: Decimal, matched_count: i32, total_unmatched_book_debits: Decimal, total_unmatched_book_credits: Decimal, unmatched_book_count: i32, total_unmatched_statement_debits: Decimal, total_unmatched_statement_credits: Decimal, unmatched_statement_count: i32, outstanding_deposits: Decimal, outstanding_checks: Decimal, deposits_in_transit: Decimal, bank_charges: Decimal, bank_interest: Decimal, nsf_checks: Decimal, other_adjustments: Decimal, difference: Decimal, is_balanced: bool, status: ReconciliationStatus, has_adjusting_entries: bool, adjusting_journal_ids: serde_json::Value) -> Self {
        Self {
            id: Uuid::new_v4(),
            company_id,
            reconciliation_number,
            account_id,
            account_number,
            account_name,
            reconciliation_type,
            period_start,
            period_end,
            statement_date,
            opening_book_balance,
            opening_statement_balance,
            previous_reconciliation_id: None,
            closing_book_balance,
            closing_statement_balance,
            total_matched_debits,
            total_matched_credits,
            matched_count,
            total_unmatched_book_debits,
            total_unmatched_book_credits,
            unmatched_book_count,
            total_unmatched_statement_debits,
            total_unmatched_statement_credits,
            unmatched_statement_count,
            outstanding_deposits,
            outstanding_checks,
            deposits_in_transit,
            bank_charges,
            bank_interest,
            nsf_checks,
            other_adjustments,
            adjusted_book_balance: None,
            adjusted_statement_balance: None,
            difference,
            is_balanced,
            status,
            started_at: None,
            started_by: None,
            completed_at: None,
            completed_by: None,
            reviewed_at: None,
            reviewed_by: None,
            has_adjusting_entries,
            adjusting_journal_ids,
            statement_source: None,
            statement_file_url: None,
            import_date: None,
            notes: None,
            discrepancy_notes: None,
            metadata: AuditMetadata::default(),
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> ReconciliationId {
        ReconciliationId(self.id)
    }

    /// Get when this entity was created
    pub fn created_at(&self) -> Option<&DateTime<Utc>> {
        self.metadata.created_at.as_ref()
    }

    /// Get when this entity was last updated
    pub fn updated_at(&self) -> Option<&DateTime<Utc>> {
        self.metadata.updated_at.as_ref()
    }

    /// Check if this entity is soft deleted
    pub fn is_deleted(&self) -> bool {
        self.metadata.deleted_at.is_some()
    }

    /// Check if this entity is active (not deleted)
    pub fn is_active(&self) -> bool {
        self.metadata.deleted_at.is_none()
    }

    /// Get when this entity was deleted
    pub fn deleted_at(&self) -> Option<&DateTime<Utc>> {
        self.metadata.deleted_at.as_ref()
    }

    /// Get who created this entity
    pub fn created_by(&self) -> Option<&Uuid> {
        self.metadata.created_by.as_ref()
    }

    /// Get who last updated this entity
    pub fn updated_by(&self) -> Option<&Uuid> {
        self.metadata.updated_by.as_ref()
    }

    /// Get who deleted this entity
    pub fn deleted_by(&self) -> Option<&Uuid> {
        self.metadata.deleted_by.as_ref()
    }

    /// Get the current status
    pub fn status(&self) -> &ReconciliationStatus {
        &self.status
    }


    // ==========================================================
    // Fluent Setters (with_* for optional fields)
    // ==========================================================

    /// Set the previous_reconciliation_id field (chainable)
    pub fn with_previous_reconciliation_id(mut self, value: Uuid) -> Self {
        self.previous_reconciliation_id = Some(value);
        self
    }

    /// Set the adjusted_book_balance field (chainable)
    pub fn with_adjusted_book_balance(mut self, value: Decimal) -> Self {
        self.adjusted_book_balance = Some(value);
        self
    }

    /// Set the adjusted_statement_balance field (chainable)
    pub fn with_adjusted_statement_balance(mut self, value: Decimal) -> Self {
        self.adjusted_statement_balance = Some(value);
        self
    }

    /// Set the started_at field (chainable)
    pub fn with_started_at(mut self, value: DateTime<Utc>) -> Self {
        self.started_at = Some(value);
        self
    }

    /// Set the started_by field (chainable)
    pub fn with_started_by(mut self, value: Uuid) -> Self {
        self.started_by = Some(value);
        self
    }

    /// Set the completed_at field (chainable)
    pub fn with_completed_at(mut self, value: DateTime<Utc>) -> Self {
        self.completed_at = Some(value);
        self
    }

    /// Set the completed_by field (chainable)
    pub fn with_completed_by(mut self, value: Uuid) -> Self {
        self.completed_by = Some(value);
        self
    }

    /// Set the reviewed_at field (chainable)
    pub fn with_reviewed_at(mut self, value: DateTime<Utc>) -> Self {
        self.reviewed_at = Some(value);
        self
    }

    /// Set the reviewed_by field (chainable)
    pub fn with_reviewed_by(mut self, value: Uuid) -> Self {
        self.reviewed_by = Some(value);
        self
    }

    /// Set the statement_source field (chainable)
    pub fn with_statement_source(mut self, value: String) -> Self {
        self.statement_source = Some(value);
        self
    }

    /// Set the statement_file_url field (chainable)
    pub fn with_statement_file_url(mut self, value: String) -> Self {
        self.statement_file_url = Some(value);
        self
    }

    /// Set the import_date field (chainable)
    pub fn with_import_date(mut self, value: DateTime<Utc>) -> Self {
        self.import_date = Some(value);
        self
    }

    /// Set the notes field (chainable)
    pub fn with_notes(mut self, value: String) -> Self {
        self.notes = Some(value);
        self
    }

    /// Set the discrepancy_notes field (chainable)
    pub fn with_discrepancy_notes(mut self, value: String) -> Self {
        self.discrepancy_notes = Some(value);
        self
    }

    // ==========================================================
    // Partial Update
    // ==========================================================

    /// Apply partial updates from a map of field name to JSON value
    pub fn apply_patch(&mut self, fields: std::collections::HashMap<String, serde_json::Value>) {
        for (key, value) in fields {
            match key.as_str() {
                "company_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.company_id = v; }
                }
                "reconciliation_number" => {
                    if let Ok(v) = serde_json::from_value(value) { self.reconciliation_number = v; }
                }
                "account_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.account_id = v; }
                }
                "account_number" => {
                    if let Ok(v) = serde_json::from_value(value) { self.account_number = v; }
                }
                "account_name" => {
                    if let Ok(v) = serde_json::from_value(value) { self.account_name = v; }
                }
                "reconciliation_type" => {
                    if let Ok(v) = serde_json::from_value(value) { self.reconciliation_type = v; }
                }
                "period_start" => {
                    if let Ok(v) = serde_json::from_value(value) { self.period_start = v; }
                }
                "period_end" => {
                    if let Ok(v) = serde_json::from_value(value) { self.period_end = v; }
                }
                "statement_date" => {
                    if let Ok(v) = serde_json::from_value(value) { self.statement_date = v; }
                }
                "opening_book_balance" => {
                    if let Ok(v) = serde_json::from_value(value) { self.opening_book_balance = v; }
                }
                "opening_statement_balance" => {
                    if let Ok(v) = serde_json::from_value(value) { self.opening_statement_balance = v; }
                }
                "previous_reconciliation_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.previous_reconciliation_id = v; }
                }
                "closing_book_balance" => {
                    if let Ok(v) = serde_json::from_value(value) { self.closing_book_balance = v; }
                }
                "closing_statement_balance" => {
                    if let Ok(v) = serde_json::from_value(value) { self.closing_statement_balance = v; }
                }
                "total_matched_debits" => {
                    if let Ok(v) = serde_json::from_value(value) { self.total_matched_debits = v; }
                }
                "total_matched_credits" => {
                    if let Ok(v) = serde_json::from_value(value) { self.total_matched_credits = v; }
                }
                "matched_count" => {
                    if let Ok(v) = serde_json::from_value(value) { self.matched_count = v; }
                }
                "total_unmatched_book_debits" => {
                    if let Ok(v) = serde_json::from_value(value) { self.total_unmatched_book_debits = v; }
                }
                "total_unmatched_book_credits" => {
                    if let Ok(v) = serde_json::from_value(value) { self.total_unmatched_book_credits = v; }
                }
                "unmatched_book_count" => {
                    if let Ok(v) = serde_json::from_value(value) { self.unmatched_book_count = v; }
                }
                "total_unmatched_statement_debits" => {
                    if let Ok(v) = serde_json::from_value(value) { self.total_unmatched_statement_debits = v; }
                }
                "total_unmatched_statement_credits" => {
                    if let Ok(v) = serde_json::from_value(value) { self.total_unmatched_statement_credits = v; }
                }
                "unmatched_statement_count" => {
                    if let Ok(v) = serde_json::from_value(value) { self.unmatched_statement_count = v; }
                }
                "outstanding_deposits" => {
                    if let Ok(v) = serde_json::from_value(value) { self.outstanding_deposits = v; }
                }
                "outstanding_checks" => {
                    if let Ok(v) = serde_json::from_value(value) { self.outstanding_checks = v; }
                }
                "deposits_in_transit" => {
                    if let Ok(v) = serde_json::from_value(value) { self.deposits_in_transit = v; }
                }
                "bank_charges" => {
                    if let Ok(v) = serde_json::from_value(value) { self.bank_charges = v; }
                }
                "bank_interest" => {
                    if let Ok(v) = serde_json::from_value(value) { self.bank_interest = v; }
                }
                "nsf_checks" => {
                    if let Ok(v) = serde_json::from_value(value) { self.nsf_checks = v; }
                }
                "other_adjustments" => {
                    if let Ok(v) = serde_json::from_value(value) { self.other_adjustments = v; }
                }
                "adjusted_book_balance" => {
                    if let Ok(v) = serde_json::from_value(value) { self.adjusted_book_balance = v; }
                }
                "adjusted_statement_balance" => {
                    if let Ok(v) = serde_json::from_value(value) { self.adjusted_statement_balance = v; }
                }
                "difference" => {
                    if let Ok(v) = serde_json::from_value(value) { self.difference = v; }
                }
                "is_balanced" => {
                    if let Ok(v) = serde_json::from_value(value) { self.is_balanced = v; }
                }
                "status" => {
                    if let Ok(v) = serde_json::from_value(value) { self.status = v; }
                }
                "started_at" => {
                    if let Ok(v) = serde_json::from_value(value) { self.started_at = v; }
                }
                "started_by" => {
                    if let Ok(v) = serde_json::from_value(value) { self.started_by = v; }
                }
                "completed_at" => {
                    if let Ok(v) = serde_json::from_value(value) { self.completed_at = v; }
                }
                "completed_by" => {
                    if let Ok(v) = serde_json::from_value(value) { self.completed_by = v; }
                }
                "reviewed_at" => {
                    if let Ok(v) = serde_json::from_value(value) { self.reviewed_at = v; }
                }
                "reviewed_by" => {
                    if let Ok(v) = serde_json::from_value(value) { self.reviewed_by = v; }
                }
                "has_adjusting_entries" => {
                    if let Ok(v) = serde_json::from_value(value) { self.has_adjusting_entries = v; }
                }
                "adjusting_journal_ids" => {
                    if let Ok(v) = serde_json::from_value(value) { self.adjusting_journal_ids = v; }
                }
                "statement_source" => {
                    if let Ok(v) = serde_json::from_value(value) { self.statement_source = v; }
                }
                "statement_file_url" => {
                    if let Ok(v) = serde_json::from_value(value) { self.statement_file_url = v; }
                }
                "import_date" => {
                    if let Ok(v) = serde_json::from_value(value) { self.import_date = v; }
                }
                "notes" => {
                    if let Ok(v) = serde_json::from_value(value) { self.notes = v; }
                }
                "discrepancy_notes" => {
                    if let Ok(v) = serde_json::from_value(value) { self.discrepancy_notes = v; }
                }
                _ => {} // ignore unknown fields
            }
        }
    }

    // <<< CUSTOM METHODS START >>>
    // <<< CUSTOM METHODS END >>>
}

impl super::Entity for Reconciliation {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "Reconciliation"
    }
}

impl backbone_core::PersistentEntity for Reconciliation {
    fn entity_id(&self) -> String {
        self.id.to_string()
    }
    fn set_entity_id(&mut self, id: String) {
        if let Ok(uuid) = uuid::Uuid::parse_str(&id) {
            self.id = uuid;
        }
    }
    fn created_at(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        self.metadata.created_at
    }
    fn set_created_at(&mut self, ts: chrono::DateTime<chrono::Utc>) {
        self.metadata.created_at = Some(ts);
    }
    fn updated_at(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        self.metadata.updated_at
    }
    fn set_updated_at(&mut self, ts: chrono::DateTime<chrono::Utc>) {
        self.metadata.updated_at = Some(ts);
    }
    fn deleted_at(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        self.metadata.deleted_at
    }
    fn set_deleted_at(&mut self, ts: Option<chrono::DateTime<chrono::Utc>>) {
        self.metadata.deleted_at = ts;
    }
}

impl backbone_orm::EntityRepoMeta for Reconciliation {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("company_id".to_string(), "uuid".to_string());
        m.insert("account_id".to_string(), "uuid".to_string());
        m.insert("previous_reconciliation_id".to_string(), "uuid".to_string());
        m.insert("reconciliation_type".to_string(), "reconciliation_type".to_string());
        m.insert("status".to_string(), "reconciliation_status".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &["reconciliation_number", "account_number", "account_name"]
    }
    fn company_field() -> Option<&'static str> {
        Some("company_id")
    }
    fn relations() -> &'static [(&'static str, &'static str, &'static str)] {
        &[("account", "accounts", "accountId"), ("previousReconciliation", "reconciliations", "previousReconciliationId")]
    }
}

/// Builder for Reconciliation entity
///
/// Provides a fluent API for constructing Reconciliation instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct ReconciliationBuilder {
    company_id: Option<Uuid>,
    reconciliation_number: Option<String>,
    account_id: Option<Uuid>,
    account_number: Option<String>,
    account_name: Option<String>,
    reconciliation_type: Option<ReconciliationType>,
    period_start: Option<NaiveDate>,
    period_end: Option<NaiveDate>,
    statement_date: Option<NaiveDate>,
    opening_book_balance: Option<Decimal>,
    opening_statement_balance: Option<Decimal>,
    previous_reconciliation_id: Option<Uuid>,
    closing_book_balance: Option<Decimal>,
    closing_statement_balance: Option<Decimal>,
    total_matched_debits: Option<Decimal>,
    total_matched_credits: Option<Decimal>,
    matched_count: Option<i32>,
    total_unmatched_book_debits: Option<Decimal>,
    total_unmatched_book_credits: Option<Decimal>,
    unmatched_book_count: Option<i32>,
    total_unmatched_statement_debits: Option<Decimal>,
    total_unmatched_statement_credits: Option<Decimal>,
    unmatched_statement_count: Option<i32>,
    outstanding_deposits: Option<Decimal>,
    outstanding_checks: Option<Decimal>,
    deposits_in_transit: Option<Decimal>,
    bank_charges: Option<Decimal>,
    bank_interest: Option<Decimal>,
    nsf_checks: Option<Decimal>,
    other_adjustments: Option<Decimal>,
    adjusted_book_balance: Option<Decimal>,
    adjusted_statement_balance: Option<Decimal>,
    difference: Option<Decimal>,
    is_balanced: Option<bool>,
    status: Option<ReconciliationStatus>,
    started_at: Option<DateTime<Utc>>,
    started_by: Option<Uuid>,
    completed_at: Option<DateTime<Utc>>,
    completed_by: Option<Uuid>,
    reviewed_at: Option<DateTime<Utc>>,
    reviewed_by: Option<Uuid>,
    has_adjusting_entries: Option<bool>,
    adjusting_journal_ids: Option<serde_json::Value>,
    statement_source: Option<String>,
    statement_file_url: Option<String>,
    import_date: Option<DateTime<Utc>>,
    notes: Option<String>,
    discrepancy_notes: Option<String>,
}

impl ReconciliationBuilder {
    /// Set the company_id field (required)
    pub fn company_id(mut self, value: Uuid) -> Self {
        self.company_id = Some(value);
        self
    }

    /// Set the reconciliation_number field (required)
    pub fn reconciliation_number(mut self, value: String) -> Self {
        self.reconciliation_number = Some(value);
        self
    }

    /// Set the account_id field (required)
    pub fn account_id(mut self, value: Uuid) -> Self {
        self.account_id = Some(value);
        self
    }

    /// Set the account_number field (required)
    pub fn account_number(mut self, value: String) -> Self {
        self.account_number = Some(value);
        self
    }

    /// Set the account_name field (required)
    pub fn account_name(mut self, value: String) -> Self {
        self.account_name = Some(value);
        self
    }

    /// Set the reconciliation_type field (default: `ReconciliationType::default()`)
    pub fn reconciliation_type(mut self, value: ReconciliationType) -> Self {
        self.reconciliation_type = Some(value);
        self
    }

    /// Set the period_start field (required)
    pub fn period_start(mut self, value: NaiveDate) -> Self {
        self.period_start = Some(value);
        self
    }

    /// Set the period_end field (required)
    pub fn period_end(mut self, value: NaiveDate) -> Self {
        self.period_end = Some(value);
        self
    }

    /// Set the statement_date field (required)
    pub fn statement_date(mut self, value: NaiveDate) -> Self {
        self.statement_date = Some(value);
        self
    }

    /// Set the opening_book_balance field (required)
    pub fn opening_book_balance(mut self, value: Decimal) -> Self {
        self.opening_book_balance = Some(value);
        self
    }

    /// Set the opening_statement_balance field (required)
    pub fn opening_statement_balance(mut self, value: Decimal) -> Self {
        self.opening_statement_balance = Some(value);
        self
    }

    /// Set the previous_reconciliation_id field (optional)
    pub fn previous_reconciliation_id(mut self, value: Uuid) -> Self {
        self.previous_reconciliation_id = Some(value);
        self
    }

    /// Set the closing_book_balance field (required)
    pub fn closing_book_balance(mut self, value: Decimal) -> Self {
        self.closing_book_balance = Some(value);
        self
    }

    /// Set the closing_statement_balance field (required)
    pub fn closing_statement_balance(mut self, value: Decimal) -> Self {
        self.closing_statement_balance = Some(value);
        self
    }

    /// Set the total_matched_debits field (default: `Decimal::from(0)`)
    pub fn total_matched_debits(mut self, value: Decimal) -> Self {
        self.total_matched_debits = Some(value);
        self
    }

    /// Set the total_matched_credits field (default: `Decimal::from(0)`)
    pub fn total_matched_credits(mut self, value: Decimal) -> Self {
        self.total_matched_credits = Some(value);
        self
    }

    /// Set the matched_count field (default: `0`)
    pub fn matched_count(mut self, value: i32) -> Self {
        self.matched_count = Some(value);
        self
    }

    /// Set the total_unmatched_book_debits field (default: `Decimal::from(0)`)
    pub fn total_unmatched_book_debits(mut self, value: Decimal) -> Self {
        self.total_unmatched_book_debits = Some(value);
        self
    }

    /// Set the total_unmatched_book_credits field (default: `Decimal::from(0)`)
    pub fn total_unmatched_book_credits(mut self, value: Decimal) -> Self {
        self.total_unmatched_book_credits = Some(value);
        self
    }

    /// Set the unmatched_book_count field (default: `0`)
    pub fn unmatched_book_count(mut self, value: i32) -> Self {
        self.unmatched_book_count = Some(value);
        self
    }

    /// Set the total_unmatched_statement_debits field (default: `Decimal::from(0)`)
    pub fn total_unmatched_statement_debits(mut self, value: Decimal) -> Self {
        self.total_unmatched_statement_debits = Some(value);
        self
    }

    /// Set the total_unmatched_statement_credits field (default: `Decimal::from(0)`)
    pub fn total_unmatched_statement_credits(mut self, value: Decimal) -> Self {
        self.total_unmatched_statement_credits = Some(value);
        self
    }

    /// Set the unmatched_statement_count field (default: `0`)
    pub fn unmatched_statement_count(mut self, value: i32) -> Self {
        self.unmatched_statement_count = Some(value);
        self
    }

    /// Set the outstanding_deposits field (default: `Decimal::from(0)`)
    pub fn outstanding_deposits(mut self, value: Decimal) -> Self {
        self.outstanding_deposits = Some(value);
        self
    }

    /// Set the outstanding_checks field (default: `Decimal::from(0)`)
    pub fn outstanding_checks(mut self, value: Decimal) -> Self {
        self.outstanding_checks = Some(value);
        self
    }

    /// Set the deposits_in_transit field (default: `Decimal::from(0)`)
    pub fn deposits_in_transit(mut self, value: Decimal) -> Self {
        self.deposits_in_transit = Some(value);
        self
    }

    /// Set the bank_charges field (default: `Decimal::from(0)`)
    pub fn bank_charges(mut self, value: Decimal) -> Self {
        self.bank_charges = Some(value);
        self
    }

    /// Set the bank_interest field (default: `Decimal::from(0)`)
    pub fn bank_interest(mut self, value: Decimal) -> Self {
        self.bank_interest = Some(value);
        self
    }

    /// Set the nsf_checks field (default: `Decimal::from(0)`)
    pub fn nsf_checks(mut self, value: Decimal) -> Self {
        self.nsf_checks = Some(value);
        self
    }

    /// Set the other_adjustments field (default: `Decimal::from(0)`)
    pub fn other_adjustments(mut self, value: Decimal) -> Self {
        self.other_adjustments = Some(value);
        self
    }

    /// Set the adjusted_book_balance field (optional)
    pub fn adjusted_book_balance(mut self, value: Decimal) -> Self {
        self.adjusted_book_balance = Some(value);
        self
    }

    /// Set the adjusted_statement_balance field (optional)
    pub fn adjusted_statement_balance(mut self, value: Decimal) -> Self {
        self.adjusted_statement_balance = Some(value);
        self
    }

    /// Set the difference field (default: `Decimal::from(0)`)
    pub fn difference(mut self, value: Decimal) -> Self {
        self.difference = Some(value);
        self
    }

    /// Set the is_balanced field (default: `false`)
    pub fn is_balanced(mut self, value: bool) -> Self {
        self.is_balanced = Some(value);
        self
    }

    /// Set the status field (default: `ReconciliationStatus::default()`)
    pub fn status(mut self, value: ReconciliationStatus) -> Self {
        self.status = Some(value);
        self
    }

    /// Set the started_at field (optional)
    pub fn started_at(mut self, value: DateTime<Utc>) -> Self {
        self.started_at = Some(value);
        self
    }

    /// Set the started_by field (optional)
    pub fn started_by(mut self, value: Uuid) -> Self {
        self.started_by = Some(value);
        self
    }

    /// Set the completed_at field (optional)
    pub fn completed_at(mut self, value: DateTime<Utc>) -> Self {
        self.completed_at = Some(value);
        self
    }

    /// Set the completed_by field (optional)
    pub fn completed_by(mut self, value: Uuid) -> Self {
        self.completed_by = Some(value);
        self
    }

    /// Set the reviewed_at field (optional)
    pub fn reviewed_at(mut self, value: DateTime<Utc>) -> Self {
        self.reviewed_at = Some(value);
        self
    }

    /// Set the reviewed_by field (optional)
    pub fn reviewed_by(mut self, value: Uuid) -> Self {
        self.reviewed_by = Some(value);
        self
    }

    /// Set the has_adjusting_entries field (default: `false`)
    pub fn has_adjusting_entries(mut self, value: bool) -> Self {
        self.has_adjusting_entries = Some(value);
        self
    }

    /// Set the adjusting_journal_ids field (default: `serde_json::json!([])`)
    pub fn adjusting_journal_ids(mut self, value: serde_json::Value) -> Self {
        self.adjusting_journal_ids = Some(value);
        self
    }

    /// Set the statement_source field (optional)
    pub fn statement_source(mut self, value: String) -> Self {
        self.statement_source = Some(value);
        self
    }

    /// Set the statement_file_url field (optional)
    pub fn statement_file_url(mut self, value: String) -> Self {
        self.statement_file_url = Some(value);
        self
    }

    /// Set the import_date field (optional)
    pub fn import_date(mut self, value: DateTime<Utc>) -> Self {
        self.import_date = Some(value);
        self
    }

    /// Set the notes field (optional)
    pub fn notes(mut self, value: String) -> Self {
        self.notes = Some(value);
        self
    }

    /// Set the discrepancy_notes field (optional)
    pub fn discrepancy_notes(mut self, value: String) -> Self {
        self.discrepancy_notes = Some(value);
        self
    }

    /// Build the Reconciliation entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<Reconciliation, String> {
        let company_id = self.company_id.ok_or_else(|| "company_id is required".to_string())?;
        let reconciliation_number = self.reconciliation_number.ok_or_else(|| "reconciliation_number is required".to_string())?;
        let account_id = self.account_id.ok_or_else(|| "account_id is required".to_string())?;
        let account_number = self.account_number.ok_or_else(|| "account_number is required".to_string())?;
        let account_name = self.account_name.ok_or_else(|| "account_name is required".to_string())?;
        let period_start = self.period_start.ok_or_else(|| "period_start is required".to_string())?;
        let period_end = self.period_end.ok_or_else(|| "period_end is required".to_string())?;
        let statement_date = self.statement_date.ok_or_else(|| "statement_date is required".to_string())?;
        let opening_book_balance = self.opening_book_balance.ok_or_else(|| "opening_book_balance is required".to_string())?;
        let opening_statement_balance = self.opening_statement_balance.ok_or_else(|| "opening_statement_balance is required".to_string())?;
        let closing_book_balance = self.closing_book_balance.ok_or_else(|| "closing_book_balance is required".to_string())?;
        let closing_statement_balance = self.closing_statement_balance.ok_or_else(|| "closing_statement_balance is required".to_string())?;

        Ok(Reconciliation {
            id: Uuid::new_v4(),
            company_id,
            reconciliation_number,
            account_id,
            account_number,
            account_name,
            reconciliation_type: self.reconciliation_type.unwrap_or(ReconciliationType::default()),
            period_start,
            period_end,
            statement_date,
            opening_book_balance,
            opening_statement_balance,
            previous_reconciliation_id: self.previous_reconciliation_id,
            closing_book_balance,
            closing_statement_balance,
            total_matched_debits: self.total_matched_debits.unwrap_or(Decimal::from(0)),
            total_matched_credits: self.total_matched_credits.unwrap_or(Decimal::from(0)),
            matched_count: self.matched_count.unwrap_or(0),
            total_unmatched_book_debits: self.total_unmatched_book_debits.unwrap_or(Decimal::from(0)),
            total_unmatched_book_credits: self.total_unmatched_book_credits.unwrap_or(Decimal::from(0)),
            unmatched_book_count: self.unmatched_book_count.unwrap_or(0),
            total_unmatched_statement_debits: self.total_unmatched_statement_debits.unwrap_or(Decimal::from(0)),
            total_unmatched_statement_credits: self.total_unmatched_statement_credits.unwrap_or(Decimal::from(0)),
            unmatched_statement_count: self.unmatched_statement_count.unwrap_or(0),
            outstanding_deposits: self.outstanding_deposits.unwrap_or(Decimal::from(0)),
            outstanding_checks: self.outstanding_checks.unwrap_or(Decimal::from(0)),
            deposits_in_transit: self.deposits_in_transit.unwrap_or(Decimal::from(0)),
            bank_charges: self.bank_charges.unwrap_or(Decimal::from(0)),
            bank_interest: self.bank_interest.unwrap_or(Decimal::from(0)),
            nsf_checks: self.nsf_checks.unwrap_or(Decimal::from(0)),
            other_adjustments: self.other_adjustments.unwrap_or(Decimal::from(0)),
            adjusted_book_balance: self.adjusted_book_balance,
            adjusted_statement_balance: self.adjusted_statement_balance,
            difference: self.difference.unwrap_or(Decimal::from(0)),
            is_balanced: self.is_balanced.unwrap_or(false),
            status: self.status.unwrap_or(ReconciliationStatus::default()),
            started_at: self.started_at,
            started_by: self.started_by,
            completed_at: self.completed_at,
            completed_by: self.completed_by,
            reviewed_at: self.reviewed_at,
            reviewed_by: self.reviewed_by,
            has_adjusting_entries: self.has_adjusting_entries.unwrap_or(false),
            adjusting_journal_ids: self.adjusting_journal_ids.unwrap_or(serde_json::json!([])),
            statement_source: self.statement_source,
            statement_file_url: self.statement_file_url,
            import_date: self.import_date,
            notes: self.notes,
            discrepancy_notes: self.discrepancy_notes,
            metadata: AuditMetadata::default(),
        })
    }
}
