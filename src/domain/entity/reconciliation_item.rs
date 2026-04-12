use chrono::{DateTime, Utc, NaiveDate};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use rust_decimal::Decimal;

use super::ReconciliationItemStatus;

/// Strongly-typed ID for ReconciliationItem
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ReconciliationItemId(pub Uuid);

impl ReconciliationItemId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for ReconciliationItemId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for ReconciliationItemId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for ReconciliationItemId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<ReconciliationItemId> for Uuid {
    fn from(id: ReconciliationItemId) -> Self { id.0 }
}

impl AsRef<Uuid> for ReconciliationItemId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for ReconciliationItemId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ReconciliationItem {
    pub id: Uuid,
    pub reconciliation_id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_id: Option<Uuid>,
    pub item_number: i32,
    pub source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ledger_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub journal_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub book_date: Option<NaiveDate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub book_reference: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub book_description: Option<String>,
    pub book_debit: Decimal,
    pub book_credit: Decimal,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub statement_date: Option<NaiveDate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub statement_reference: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub statement_description: Option<String>,
    pub statement_debit: Decimal,
    pub statement_credit: Decimal,
    pub status: ReconciliationItemStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matched_with_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub match_date: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub match_method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub match_confidence: Option<Decimal>,
    pub has_difference: bool,
    pub difference_amount: Decimal,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub difference_reason: Option<String>,
    pub requires_adjustment: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub adjustment_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub adjustment_journal_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub adjusted_at: Option<DateTime<Utc>>,
    pub is_written_off: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub write_off_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub write_off_approved_by: Option<Uuid>,
    pub is_outstanding: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outstanding_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_clear_date: Option<NaiveDate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    pub data: serde_json::Value,
}

impl ReconciliationItem {
    /// Create a builder for ReconciliationItem
    pub fn builder() -> ReconciliationItemBuilder {
        ReconciliationItemBuilder::default()
    }

    /// Create a new ReconciliationItem with required fields
    pub fn new(reconciliation_id: Uuid, item_number: i32, source: String, book_debit: Decimal, book_credit: Decimal, statement_debit: Decimal, statement_credit: Decimal, status: ReconciliationItemStatus, has_difference: bool, difference_amount: Decimal, requires_adjustment: bool, is_written_off: bool, is_outstanding: bool, data: serde_json::Value) -> Self {
        Self {
            id: Uuid::new_v4(),
            reconciliation_id,
            provider_id: None,
            item_number,
            source,
            ledger_id: None,
            journal_id: None,
            book_date: None,
            book_reference: None,
            book_description: None,
            book_debit,
            book_credit,
            statement_date: None,
            statement_reference: None,
            statement_description: None,
            statement_debit,
            statement_credit,
            status,
            matched_with_id: None,
            match_date: None,
            match_method: None,
            match_confidence: None,
            has_difference,
            difference_amount,
            difference_reason: None,
            requires_adjustment,
            adjustment_type: None,
            adjustment_journal_id: None,
            adjusted_at: None,
            is_written_off,
            write_off_reason: None,
            write_off_approved_by: None,
            is_outstanding,
            outstanding_type: None,
            expected_clear_date: None,
            notes: None,
            data,
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> ReconciliationItemId {
        ReconciliationItemId(self.id)
    }

    /// Get the current status
    pub fn status(&self) -> &ReconciliationItemStatus {
        &self.status
    }


    // ==========================================================
    // Fluent Setters (with_* for optional fields)
    // ==========================================================

    /// Set the provider_id field (chainable)
    pub fn with_provider_id(mut self, value: Uuid) -> Self {
        self.provider_id = Some(value);
        self
    }

    /// Set the ledger_id field (chainable)
    pub fn with_ledger_id(mut self, value: Uuid) -> Self {
        self.ledger_id = Some(value);
        self
    }

    /// Set the journal_id field (chainable)
    pub fn with_journal_id(mut self, value: Uuid) -> Self {
        self.journal_id = Some(value);
        self
    }

    /// Set the book_date field (chainable)
    pub fn with_book_date(mut self, value: NaiveDate) -> Self {
        self.book_date = Some(value);
        self
    }

    /// Set the book_reference field (chainable)
    pub fn with_book_reference(mut self, value: String) -> Self {
        self.book_reference = Some(value);
        self
    }

    /// Set the book_description field (chainable)
    pub fn with_book_description(mut self, value: String) -> Self {
        self.book_description = Some(value);
        self
    }

    /// Set the statement_date field (chainable)
    pub fn with_statement_date(mut self, value: NaiveDate) -> Self {
        self.statement_date = Some(value);
        self
    }

    /// Set the statement_reference field (chainable)
    pub fn with_statement_reference(mut self, value: String) -> Self {
        self.statement_reference = Some(value);
        self
    }

    /// Set the statement_description field (chainable)
    pub fn with_statement_description(mut self, value: String) -> Self {
        self.statement_description = Some(value);
        self
    }

    /// Set the matched_with_id field (chainable)
    pub fn with_matched_with_id(mut self, value: Uuid) -> Self {
        self.matched_with_id = Some(value);
        self
    }

    /// Set the match_date field (chainable)
    pub fn with_match_date(mut self, value: DateTime<Utc>) -> Self {
        self.match_date = Some(value);
        self
    }

    /// Set the match_method field (chainable)
    pub fn with_match_method(mut self, value: String) -> Self {
        self.match_method = Some(value);
        self
    }

    /// Set the match_confidence field (chainable)
    pub fn with_match_confidence(mut self, value: Decimal) -> Self {
        self.match_confidence = Some(value);
        self
    }

    /// Set the difference_reason field (chainable)
    pub fn with_difference_reason(mut self, value: String) -> Self {
        self.difference_reason = Some(value);
        self
    }

    /// Set the adjustment_type field (chainable)
    pub fn with_adjustment_type(mut self, value: String) -> Self {
        self.adjustment_type = Some(value);
        self
    }

    /// Set the adjustment_journal_id field (chainable)
    pub fn with_adjustment_journal_id(mut self, value: Uuid) -> Self {
        self.adjustment_journal_id = Some(value);
        self
    }

    /// Set the adjusted_at field (chainable)
    pub fn with_adjusted_at(mut self, value: DateTime<Utc>) -> Self {
        self.adjusted_at = Some(value);
        self
    }

    /// Set the write_off_reason field (chainable)
    pub fn with_write_off_reason(mut self, value: String) -> Self {
        self.write_off_reason = Some(value);
        self
    }

    /// Set the write_off_approved_by field (chainable)
    pub fn with_write_off_approved_by(mut self, value: Uuid) -> Self {
        self.write_off_approved_by = Some(value);
        self
    }

    /// Set the outstanding_type field (chainable)
    pub fn with_outstanding_type(mut self, value: String) -> Self {
        self.outstanding_type = Some(value);
        self
    }

    /// Set the expected_clear_date field (chainable)
    pub fn with_expected_clear_date(mut self, value: NaiveDate) -> Self {
        self.expected_clear_date = Some(value);
        self
    }

    /// Set the notes field (chainable)
    pub fn with_notes(mut self, value: String) -> Self {
        self.notes = Some(value);
        self
    }

    // ==========================================================
    // Partial Update
    // ==========================================================

    /// Apply partial updates from a map of field name to JSON value
    pub fn apply_patch(&mut self, fields: std::collections::HashMap<String, serde_json::Value>) {
        for (key, value) in fields {
            match key.as_str() {
                "reconciliation_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.reconciliation_id = v; }
                }
                "provider_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.provider_id = v; }
                }
                "item_number" => {
                    if let Ok(v) = serde_json::from_value(value) { self.item_number = v; }
                }
                "source" => {
                    if let Ok(v) = serde_json::from_value(value) { self.source = v; }
                }
                "ledger_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.ledger_id = v; }
                }
                "journal_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.journal_id = v; }
                }
                "book_date" => {
                    if let Ok(v) = serde_json::from_value(value) { self.book_date = v; }
                }
                "book_reference" => {
                    if let Ok(v) = serde_json::from_value(value) { self.book_reference = v; }
                }
                "book_description" => {
                    if let Ok(v) = serde_json::from_value(value) { self.book_description = v; }
                }
                "book_debit" => {
                    if let Ok(v) = serde_json::from_value(value) { self.book_debit = v; }
                }
                "book_credit" => {
                    if let Ok(v) = serde_json::from_value(value) { self.book_credit = v; }
                }
                "statement_date" => {
                    if let Ok(v) = serde_json::from_value(value) { self.statement_date = v; }
                }
                "statement_reference" => {
                    if let Ok(v) = serde_json::from_value(value) { self.statement_reference = v; }
                }
                "statement_description" => {
                    if let Ok(v) = serde_json::from_value(value) { self.statement_description = v; }
                }
                "statement_debit" => {
                    if let Ok(v) = serde_json::from_value(value) { self.statement_debit = v; }
                }
                "statement_credit" => {
                    if let Ok(v) = serde_json::from_value(value) { self.statement_credit = v; }
                }
                "status" => {
                    if let Ok(v) = serde_json::from_value(value) { self.status = v; }
                }
                "matched_with_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.matched_with_id = v; }
                }
                "match_date" => {
                    if let Ok(v) = serde_json::from_value(value) { self.match_date = v; }
                }
                "match_method" => {
                    if let Ok(v) = serde_json::from_value(value) { self.match_method = v; }
                }
                "match_confidence" => {
                    if let Ok(v) = serde_json::from_value(value) { self.match_confidence = v; }
                }
                "has_difference" => {
                    if let Ok(v) = serde_json::from_value(value) { self.has_difference = v; }
                }
                "difference_amount" => {
                    if let Ok(v) = serde_json::from_value(value) { self.difference_amount = v; }
                }
                "difference_reason" => {
                    if let Ok(v) = serde_json::from_value(value) { self.difference_reason = v; }
                }
                "requires_adjustment" => {
                    if let Ok(v) = serde_json::from_value(value) { self.requires_adjustment = v; }
                }
                "adjustment_type" => {
                    if let Ok(v) = serde_json::from_value(value) { self.adjustment_type = v; }
                }
                "adjustment_journal_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.adjustment_journal_id = v; }
                }
                "adjusted_at" => {
                    if let Ok(v) = serde_json::from_value(value) { self.adjusted_at = v; }
                }
                "is_written_off" => {
                    if let Ok(v) = serde_json::from_value(value) { self.is_written_off = v; }
                }
                "write_off_reason" => {
                    if let Ok(v) = serde_json::from_value(value) { self.write_off_reason = v; }
                }
                "write_off_approved_by" => {
                    if let Ok(v) = serde_json::from_value(value) { self.write_off_approved_by = v; }
                }
                "is_outstanding" => {
                    if let Ok(v) = serde_json::from_value(value) { self.is_outstanding = v; }
                }
                "outstanding_type" => {
                    if let Ok(v) = serde_json::from_value(value) { self.outstanding_type = v; }
                }
                "expected_clear_date" => {
                    if let Ok(v) = serde_json::from_value(value) { self.expected_clear_date = v; }
                }
                "notes" => {
                    if let Ok(v) = serde_json::from_value(value) { self.notes = v; }
                }
                "data" => {
                    if let Ok(v) = serde_json::from_value(value) { self.data = v; }
                }
                _ => {} // ignore unknown fields
            }
        }
    }

    // <<< CUSTOM METHODS START >>>
    // <<< CUSTOM METHODS END >>>
}

impl super::Entity for ReconciliationItem {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "ReconciliationItem"
    }
}

impl backbone_core::PersistentEntity for ReconciliationItem {
    fn entity_id(&self) -> String {
        self.id.to_string()
    }
    fn set_entity_id(&mut self, id: String) {
        if let Ok(uuid) = uuid::Uuid::parse_str(&id) {
            self.id = uuid;
        }
    }
    fn created_at(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        None
    }
    fn set_created_at(&mut self, ts: chrono::DateTime<chrono::Utc>) {
        let _ = ts;
    }
    fn updated_at(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        None
    }
    fn set_updated_at(&mut self, ts: chrono::DateTime<chrono::Utc>) {
        let _ = ts;
    }
    fn deleted_at(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        None
    }
    fn set_deleted_at(&mut self, ts: Option<chrono::DateTime<chrono::Utc>>) {
        let _ = ts;
    }
}

impl backbone_orm::EntityRepoMeta for ReconciliationItem {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("reconciliation_id".to_string(), "uuid".to_string());
        m.insert("provider_id".to_string(), "uuid".to_string());
        m.insert("ledger_id".to_string(), "uuid".to_string());
        m.insert("journal_id".to_string(), "uuid".to_string());
        m.insert("matched_with_id".to_string(), "uuid".to_string());
        m.insert("adjustment_journal_id".to_string(), "uuid".to_string());
        m.insert("status".to_string(), "reconciliation_item_status".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &["source"]
    }
}

/// Builder for ReconciliationItem entity
///
/// Provides a fluent API for constructing ReconciliationItem instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct ReconciliationItemBuilder {
    reconciliation_id: Option<Uuid>,
    provider_id: Option<Uuid>,
    item_number: Option<i32>,
    source: Option<String>,
    ledger_id: Option<Uuid>,
    journal_id: Option<Uuid>,
    book_date: Option<NaiveDate>,
    book_reference: Option<String>,
    book_description: Option<String>,
    book_debit: Option<Decimal>,
    book_credit: Option<Decimal>,
    statement_date: Option<NaiveDate>,
    statement_reference: Option<String>,
    statement_description: Option<String>,
    statement_debit: Option<Decimal>,
    statement_credit: Option<Decimal>,
    status: Option<ReconciliationItemStatus>,
    matched_with_id: Option<Uuid>,
    match_date: Option<DateTime<Utc>>,
    match_method: Option<String>,
    match_confidence: Option<Decimal>,
    has_difference: Option<bool>,
    difference_amount: Option<Decimal>,
    difference_reason: Option<String>,
    requires_adjustment: Option<bool>,
    adjustment_type: Option<String>,
    adjustment_journal_id: Option<Uuid>,
    adjusted_at: Option<DateTime<Utc>>,
    is_written_off: Option<bool>,
    write_off_reason: Option<String>,
    write_off_approved_by: Option<Uuid>,
    is_outstanding: Option<bool>,
    outstanding_type: Option<String>,
    expected_clear_date: Option<NaiveDate>,
    notes: Option<String>,
    data: Option<serde_json::Value>,
}

impl ReconciliationItemBuilder {
    /// Set the reconciliation_id field (required)
    pub fn reconciliation_id(mut self, value: Uuid) -> Self {
        self.reconciliation_id = Some(value);
        self
    }

    /// Set the provider_id field (optional)
    pub fn provider_id(mut self, value: Uuid) -> Self {
        self.provider_id = Some(value);
        self
    }

    /// Set the item_number field (required)
    pub fn item_number(mut self, value: i32) -> Self {
        self.item_number = Some(value);
        self
    }

    /// Set the source field (required)
    pub fn source(mut self, value: String) -> Self {
        self.source = Some(value);
        self
    }

    /// Set the ledger_id field (optional)
    pub fn ledger_id(mut self, value: Uuid) -> Self {
        self.ledger_id = Some(value);
        self
    }

    /// Set the journal_id field (optional)
    pub fn journal_id(mut self, value: Uuid) -> Self {
        self.journal_id = Some(value);
        self
    }

    /// Set the book_date field (optional)
    pub fn book_date(mut self, value: NaiveDate) -> Self {
        self.book_date = Some(value);
        self
    }

    /// Set the book_reference field (optional)
    pub fn book_reference(mut self, value: String) -> Self {
        self.book_reference = Some(value);
        self
    }

    /// Set the book_description field (optional)
    pub fn book_description(mut self, value: String) -> Self {
        self.book_description = Some(value);
        self
    }

    /// Set the book_debit field (default: `Decimal::from(0)`)
    pub fn book_debit(mut self, value: Decimal) -> Self {
        self.book_debit = Some(value);
        self
    }

    /// Set the book_credit field (default: `Decimal::from(0)`)
    pub fn book_credit(mut self, value: Decimal) -> Self {
        self.book_credit = Some(value);
        self
    }

    /// Set the statement_date field (optional)
    pub fn statement_date(mut self, value: NaiveDate) -> Self {
        self.statement_date = Some(value);
        self
    }

    /// Set the statement_reference field (optional)
    pub fn statement_reference(mut self, value: String) -> Self {
        self.statement_reference = Some(value);
        self
    }

    /// Set the statement_description field (optional)
    pub fn statement_description(mut self, value: String) -> Self {
        self.statement_description = Some(value);
        self
    }

    /// Set the statement_debit field (default: `Decimal::from(0)`)
    pub fn statement_debit(mut self, value: Decimal) -> Self {
        self.statement_debit = Some(value);
        self
    }

    /// Set the statement_credit field (default: `Decimal::from(0)`)
    pub fn statement_credit(mut self, value: Decimal) -> Self {
        self.statement_credit = Some(value);
        self
    }

    /// Set the status field (default: `ReconciliationItemStatus::default()`)
    pub fn status(mut self, value: ReconciliationItemStatus) -> Self {
        self.status = Some(value);
        self
    }

    /// Set the matched_with_id field (optional)
    pub fn matched_with_id(mut self, value: Uuid) -> Self {
        self.matched_with_id = Some(value);
        self
    }

    /// Set the match_date field (optional)
    pub fn match_date(mut self, value: DateTime<Utc>) -> Self {
        self.match_date = Some(value);
        self
    }

    /// Set the match_method field (optional)
    pub fn match_method(mut self, value: String) -> Self {
        self.match_method = Some(value);
        self
    }

    /// Set the match_confidence field (optional)
    pub fn match_confidence(mut self, value: Decimal) -> Self {
        self.match_confidence = Some(value);
        self
    }

    /// Set the has_difference field (default: `false`)
    pub fn has_difference(mut self, value: bool) -> Self {
        self.has_difference = Some(value);
        self
    }

    /// Set the difference_amount field (default: `Decimal::from(0)`)
    pub fn difference_amount(mut self, value: Decimal) -> Self {
        self.difference_amount = Some(value);
        self
    }

    /// Set the difference_reason field (optional)
    pub fn difference_reason(mut self, value: String) -> Self {
        self.difference_reason = Some(value);
        self
    }

    /// Set the requires_adjustment field (default: `false`)
    pub fn requires_adjustment(mut self, value: bool) -> Self {
        self.requires_adjustment = Some(value);
        self
    }

    /// Set the adjustment_type field (optional)
    pub fn adjustment_type(mut self, value: String) -> Self {
        self.adjustment_type = Some(value);
        self
    }

    /// Set the adjustment_journal_id field (optional)
    pub fn adjustment_journal_id(mut self, value: Uuid) -> Self {
        self.adjustment_journal_id = Some(value);
        self
    }

    /// Set the adjusted_at field (optional)
    pub fn adjusted_at(mut self, value: DateTime<Utc>) -> Self {
        self.adjusted_at = Some(value);
        self
    }

    /// Set the is_written_off field (default: `false`)
    pub fn is_written_off(mut self, value: bool) -> Self {
        self.is_written_off = Some(value);
        self
    }

    /// Set the write_off_reason field (optional)
    pub fn write_off_reason(mut self, value: String) -> Self {
        self.write_off_reason = Some(value);
        self
    }

    /// Set the write_off_approved_by field (optional)
    pub fn write_off_approved_by(mut self, value: Uuid) -> Self {
        self.write_off_approved_by = Some(value);
        self
    }

    /// Set the is_outstanding field (default: `false`)
    pub fn is_outstanding(mut self, value: bool) -> Self {
        self.is_outstanding = Some(value);
        self
    }

    /// Set the outstanding_type field (optional)
    pub fn outstanding_type(mut self, value: String) -> Self {
        self.outstanding_type = Some(value);
        self
    }

    /// Set the expected_clear_date field (optional)
    pub fn expected_clear_date(mut self, value: NaiveDate) -> Self {
        self.expected_clear_date = Some(value);
        self
    }

    /// Set the notes field (optional)
    pub fn notes(mut self, value: String) -> Self {
        self.notes = Some(value);
        self
    }

    /// Set the data field (default: `serde_json::json!({})`)
    pub fn data(mut self, value: serde_json::Value) -> Self {
        self.data = Some(value);
        self
    }

    /// Build the ReconciliationItem entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<ReconciliationItem, String> {
        let reconciliation_id = self.reconciliation_id.ok_or_else(|| "reconciliation_id is required".to_string())?;
        let item_number = self.item_number.ok_or_else(|| "item_number is required".to_string())?;
        let source = self.source.ok_or_else(|| "source is required".to_string())?;

        Ok(ReconciliationItem {
            id: Uuid::new_v4(),
            reconciliation_id,
            provider_id: self.provider_id,
            item_number,
            source,
            ledger_id: self.ledger_id,
            journal_id: self.journal_id,
            book_date: self.book_date,
            book_reference: self.book_reference,
            book_description: self.book_description,
            book_debit: self.book_debit.unwrap_or(Decimal::from(0)),
            book_credit: self.book_credit.unwrap_or(Decimal::from(0)),
            statement_date: self.statement_date,
            statement_reference: self.statement_reference,
            statement_description: self.statement_description,
            statement_debit: self.statement_debit.unwrap_or(Decimal::from(0)),
            statement_credit: self.statement_credit.unwrap_or(Decimal::from(0)),
            status: self.status.unwrap_or(ReconciliationItemStatus::default()),
            matched_with_id: self.matched_with_id,
            match_date: self.match_date,
            match_method: self.match_method,
            match_confidence: self.match_confidence,
            has_difference: self.has_difference.unwrap_or(false),
            difference_amount: self.difference_amount.unwrap_or(Decimal::from(0)),
            difference_reason: self.difference_reason,
            requires_adjustment: self.requires_adjustment.unwrap_or(false),
            adjustment_type: self.adjustment_type,
            adjustment_journal_id: self.adjustment_journal_id,
            adjusted_at: self.adjusted_at,
            is_written_off: self.is_written_off.unwrap_or(false),
            write_off_reason: self.write_off_reason,
            write_off_approved_by: self.write_off_approved_by,
            is_outstanding: self.is_outstanding.unwrap_or(false),
            outstanding_type: self.outstanding_type,
            expected_clear_date: self.expected_clear_date,
            notes: self.notes,
            data: self.data.unwrap_or(serde_json::json!({})),
        })
    }
}
