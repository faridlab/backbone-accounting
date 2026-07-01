use chrono::{DateTime, Utc, NaiveDate};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use rust_decimal::Decimal;

use super::JournalType;
use super::JournalSource;
use super::JournalStatus;
use super::AuditMetadata;

/// Strongly-typed ID for Journal
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct JournalId(pub Uuid);

impl JournalId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for JournalId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for JournalId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for JournalId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<JournalId> for Uuid {
    fn from(id: JournalId) -> Self { id.0 }
}

impl AsRef<Uuid> for JournalId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for JournalId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Journal {
    pub id: Uuid,
    pub company_id: Uuid,
    pub journal_number: String,
    pub reference_number: Option<String>,
    pub journal_type: JournalType,
    pub branch_id: Option<Uuid>,
    pub transaction_date: NaiveDate,
    pub posting_date: Option<NaiveDate>,
    pub fiscal_period_id: Option<Uuid>,
    pub fiscal_year: Option<i32>,
    pub fiscal_month: Option<i32>,
    pub description: String,
    pub notes: Option<String>,
    pub currency: String,
    pub total_debit: Decimal,
    pub total_credit: Decimal,
    pub line_count: i32,
    pub source: JournalSource,
    pub source_type: Option<String>,
    pub source_id: Option<Uuid>,
    pub source_reference: Option<String>,
    pub is_reversed: bool,
    pub reversed_by_id: Option<Uuid>,
    pub reversed_at: Option<DateTime<Utc>>,
    pub reversal_reason: Option<String>,
    pub reverses_id: Option<Uuid>,
    pub is_reversing: bool,
    pub auto_reverse: bool,
    pub auto_reverse_date: Option<NaiveDate>,
    pub status: JournalStatus,
    pub requires_approval: bool,
    pub approval_threshold: Option<Decimal>,
    pub submitted_at: Option<DateTime<Utc>>,
    pub submitted_by: Option<Uuid>,
    pub approved_at: Option<DateTime<Utc>>,
    pub approved_by: Option<Uuid>,
    pub rejected_at: Option<DateTime<Utc>>,
    pub rejected_by: Option<Uuid>,
    pub rejection_reason: Option<String>,
    pub posted_at: Option<DateTime<Utc>>,
    pub posted_by: Option<Uuid>,
    pub is_voided: bool,
    pub voided_at: Option<DateTime<Utc>>,
    pub voided_by: Option<Uuid>,
    pub void_reason: Option<String>,
    pub attachments: serde_json::Value,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl Journal {
    /// Create a builder for Journal
    pub fn builder() -> JournalBuilder {
        JournalBuilder::default()
    }

    /// Create a new Journal with required fields
    pub fn new(company_id: Uuid, journal_number: String, journal_type: JournalType, transaction_date: NaiveDate, description: String, currency: String, total_debit: Decimal, total_credit: Decimal, line_count: i32, source: JournalSource, is_reversed: bool, is_reversing: bool, auto_reverse: bool, status: JournalStatus, requires_approval: bool, is_voided: bool, attachments: serde_json::Value) -> Self {
        Self {
            id: Uuid::new_v4(),
            company_id,
            journal_number,
            reference_number: None,
            journal_type,
            branch_id: None,
            transaction_date,
            posting_date: None,
            fiscal_period_id: None,
            fiscal_year: None,
            fiscal_month: None,
            description,
            notes: None,
            currency,
            total_debit,
            total_credit,
            line_count,
            source,
            source_type: None,
            source_id: None,
            source_reference: None,
            is_reversed,
            reversed_by_id: None,
            reversed_at: None,
            reversal_reason: None,
            reverses_id: None,
            is_reversing,
            auto_reverse,
            auto_reverse_date: None,
            status,
            requires_approval,
            approval_threshold: None,
            submitted_at: None,
            submitted_by: None,
            approved_at: None,
            approved_by: None,
            rejected_at: None,
            rejected_by: None,
            rejection_reason: None,
            posted_at: None,
            posted_by: None,
            is_voided,
            voided_at: None,
            voided_by: None,
            void_reason: None,
            attachments,
            metadata: AuditMetadata::default(),
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> JournalId {
        JournalId(self.id)
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
    pub fn status(&self) -> &JournalStatus {
        &self.status
    }


    // ==========================================================
    // Fluent Setters (with_* for optional fields)
    // ==========================================================

    /// Set the reference_number field (chainable)
    pub fn with_reference_number(mut self, value: String) -> Self {
        self.reference_number = Some(value);
        self
    }

    /// Set the branch_id field (chainable)
    pub fn with_branch_id(mut self, value: Uuid) -> Self {
        self.branch_id = Some(value);
        self
    }

    /// Set the posting_date field (chainable)
    pub fn with_posting_date(mut self, value: NaiveDate) -> Self {
        self.posting_date = Some(value);
        self
    }

    /// Set the fiscal_period_id field (chainable)
    pub fn with_fiscal_period_id(mut self, value: Uuid) -> Self {
        self.fiscal_period_id = Some(value);
        self
    }

    /// Set the fiscal_year field (chainable)
    pub fn with_fiscal_year(mut self, value: i32) -> Self {
        self.fiscal_year = Some(value);
        self
    }

    /// Set the fiscal_month field (chainable)
    pub fn with_fiscal_month(mut self, value: i32) -> Self {
        self.fiscal_month = Some(value);
        self
    }

    /// Set the notes field (chainable)
    pub fn with_notes(mut self, value: String) -> Self {
        self.notes = Some(value);
        self
    }

    /// Set the source_type field (chainable)
    pub fn with_source_type(mut self, value: String) -> Self {
        self.source_type = Some(value);
        self
    }

    /// Set the source_id field (chainable)
    pub fn with_source_id(mut self, value: Uuid) -> Self {
        self.source_id = Some(value);
        self
    }

    /// Set the source_reference field (chainable)
    pub fn with_source_reference(mut self, value: String) -> Self {
        self.source_reference = Some(value);
        self
    }

    /// Set the reversed_by_id field (chainable)
    pub fn with_reversed_by_id(mut self, value: Uuid) -> Self {
        self.reversed_by_id = Some(value);
        self
    }

    /// Set the reversed_at field (chainable)
    pub fn with_reversed_at(mut self, value: DateTime<Utc>) -> Self {
        self.reversed_at = Some(value);
        self
    }

    /// Set the reversal_reason field (chainable)
    pub fn with_reversal_reason(mut self, value: String) -> Self {
        self.reversal_reason = Some(value);
        self
    }

    /// Set the reverses_id field (chainable)
    pub fn with_reverses_id(mut self, value: Uuid) -> Self {
        self.reverses_id = Some(value);
        self
    }

    /// Set the auto_reverse_date field (chainable)
    pub fn with_auto_reverse_date(mut self, value: NaiveDate) -> Self {
        self.auto_reverse_date = Some(value);
        self
    }

    /// Set the approval_threshold field (chainable)
    pub fn with_approval_threshold(mut self, value: Decimal) -> Self {
        self.approval_threshold = Some(value);
        self
    }

    /// Set the submitted_at field (chainable)
    pub fn with_submitted_at(mut self, value: DateTime<Utc>) -> Self {
        self.submitted_at = Some(value);
        self
    }

    /// Set the submitted_by field (chainable)
    pub fn with_submitted_by(mut self, value: Uuid) -> Self {
        self.submitted_by = Some(value);
        self
    }

    /// Set the approved_at field (chainable)
    pub fn with_approved_at(mut self, value: DateTime<Utc>) -> Self {
        self.approved_at = Some(value);
        self
    }

    /// Set the approved_by field (chainable)
    pub fn with_approved_by(mut self, value: Uuid) -> Self {
        self.approved_by = Some(value);
        self
    }

    /// Set the rejected_at field (chainable)
    pub fn with_rejected_at(mut self, value: DateTime<Utc>) -> Self {
        self.rejected_at = Some(value);
        self
    }

    /// Set the rejected_by field (chainable)
    pub fn with_rejected_by(mut self, value: Uuid) -> Self {
        self.rejected_by = Some(value);
        self
    }

    /// Set the rejection_reason field (chainable)
    pub fn with_rejection_reason(mut self, value: String) -> Self {
        self.rejection_reason = Some(value);
        self
    }

    /// Set the posted_at field (chainable)
    pub fn with_posted_at(mut self, value: DateTime<Utc>) -> Self {
        self.posted_at = Some(value);
        self
    }

    /// Set the posted_by field (chainable)
    pub fn with_posted_by(mut self, value: Uuid) -> Self {
        self.posted_by = Some(value);
        self
    }

    /// Set the voided_at field (chainable)
    pub fn with_voided_at(mut self, value: DateTime<Utc>) -> Self {
        self.voided_at = Some(value);
        self
    }

    /// Set the voided_by field (chainable)
    pub fn with_voided_by(mut self, value: Uuid) -> Self {
        self.voided_by = Some(value);
        self
    }

    /// Set the void_reason field (chainable)
    pub fn with_void_reason(mut self, value: String) -> Self {
        self.void_reason = Some(value);
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
                "journal_number" => {
                    if let Ok(v) = serde_json::from_value(value) { self.journal_number = v; }
                }
                "reference_number" => {
                    if let Ok(v) = serde_json::from_value(value) { self.reference_number = v; }
                }
                "journal_type" => {
                    if let Ok(v) = serde_json::from_value(value) { self.journal_type = v; }
                }
                "branch_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.branch_id = v; }
                }
                "transaction_date" => {
                    if let Ok(v) = serde_json::from_value(value) { self.transaction_date = v; }
                }
                "posting_date" => {
                    if let Ok(v) = serde_json::from_value(value) { self.posting_date = v; }
                }
                "fiscal_period_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.fiscal_period_id = v; }
                }
                "fiscal_year" => {
                    if let Ok(v) = serde_json::from_value(value) { self.fiscal_year = v; }
                }
                "fiscal_month" => {
                    if let Ok(v) = serde_json::from_value(value) { self.fiscal_month = v; }
                }
                "description" => {
                    if let Ok(v) = serde_json::from_value(value) { self.description = v; }
                }
                "notes" => {
                    if let Ok(v) = serde_json::from_value(value) { self.notes = v; }
                }
                "currency" => {
                    if let Ok(v) = serde_json::from_value(value) { self.currency = v; }
                }
                "total_debit" => {
                    if let Ok(v) = serde_json::from_value(value) { self.total_debit = v; }
                }
                "total_credit" => {
                    if let Ok(v) = serde_json::from_value(value) { self.total_credit = v; }
                }
                "line_count" => {
                    if let Ok(v) = serde_json::from_value(value) { self.line_count = v; }
                }
                "source" => {
                    if let Ok(v) = serde_json::from_value(value) { self.source = v; }
                }
                "source_type" => {
                    if let Ok(v) = serde_json::from_value(value) { self.source_type = v; }
                }
                "source_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.source_id = v; }
                }
                "source_reference" => {
                    if let Ok(v) = serde_json::from_value(value) { self.source_reference = v; }
                }
                "is_reversed" => {
                    if let Ok(v) = serde_json::from_value(value) { self.is_reversed = v; }
                }
                "reversed_by_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.reversed_by_id = v; }
                }
                "reversed_at" => {
                    if let Ok(v) = serde_json::from_value(value) { self.reversed_at = v; }
                }
                "reversal_reason" => {
                    if let Ok(v) = serde_json::from_value(value) { self.reversal_reason = v; }
                }
                "reverses_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.reverses_id = v; }
                }
                "is_reversing" => {
                    if let Ok(v) = serde_json::from_value(value) { self.is_reversing = v; }
                }
                "auto_reverse" => {
                    if let Ok(v) = serde_json::from_value(value) { self.auto_reverse = v; }
                }
                "auto_reverse_date" => {
                    if let Ok(v) = serde_json::from_value(value) { self.auto_reverse_date = v; }
                }
                "status" => {
                    if let Ok(v) = serde_json::from_value(value) { self.status = v; }
                }
                "requires_approval" => {
                    if let Ok(v) = serde_json::from_value(value) { self.requires_approval = v; }
                }
                "approval_threshold" => {
                    if let Ok(v) = serde_json::from_value(value) { self.approval_threshold = v; }
                }
                "submitted_at" => {
                    if let Ok(v) = serde_json::from_value(value) { self.submitted_at = v; }
                }
                "submitted_by" => {
                    if let Ok(v) = serde_json::from_value(value) { self.submitted_by = v; }
                }
                "approved_at" => {
                    if let Ok(v) = serde_json::from_value(value) { self.approved_at = v; }
                }
                "approved_by" => {
                    if let Ok(v) = serde_json::from_value(value) { self.approved_by = v; }
                }
                "rejected_at" => {
                    if let Ok(v) = serde_json::from_value(value) { self.rejected_at = v; }
                }
                "rejected_by" => {
                    if let Ok(v) = serde_json::from_value(value) { self.rejected_by = v; }
                }
                "rejection_reason" => {
                    if let Ok(v) = serde_json::from_value(value) { self.rejection_reason = v; }
                }
                "posted_at" => {
                    if let Ok(v) = serde_json::from_value(value) { self.posted_at = v; }
                }
                "posted_by" => {
                    if let Ok(v) = serde_json::from_value(value) { self.posted_by = v; }
                }
                "is_voided" => {
                    if let Ok(v) = serde_json::from_value(value) { self.is_voided = v; }
                }
                "voided_at" => {
                    if let Ok(v) = serde_json::from_value(value) { self.voided_at = v; }
                }
                "voided_by" => {
                    if let Ok(v) = serde_json::from_value(value) { self.voided_by = v; }
                }
                "void_reason" => {
                    if let Ok(v) = serde_json::from_value(value) { self.void_reason = v; }
                }
                "attachments" => {
                    if let Ok(v) = serde_json::from_value(value) { self.attachments = v; }
                }
                _ => {} // ignore unknown fields
            }
        }
    }

    // <<< CUSTOM METHODS START >>>
    // <<< CUSTOM METHODS END >>>
}

impl super::Entity for Journal {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "Journal"
    }
}

impl backbone_core::PersistentEntity for Journal {
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

impl backbone_orm::EntityRepoMeta for Journal {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("company_id".to_string(), "uuid".to_string());
        m.insert("branch_id".to_string(), "uuid".to_string());
        m.insert("fiscal_period_id".to_string(), "uuid".to_string());
        m.insert("source_id".to_string(), "uuid".to_string());
        m.insert("reversed_by_id".to_string(), "uuid".to_string());
        m.insert("reverses_id".to_string(), "uuid".to_string());
        m.insert("journal_type".to_string(), "journal_type".to_string());
        m.insert("source".to_string(), "journal_source".to_string());
        m.insert("status".to_string(), "journal_status".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &["journal_number", "description", "currency"]
    }
    fn relations() -> &'static [(&'static str, &'static str, &'static str)] {
        &[("fiscalPeriod", "fiscal_periods", "fiscalPeriodId"), ("reversedBy", "journals", "reversedById"), ("reverses", "journals", "reversesId")]
    }
}

/// Builder for Journal entity
///
/// Provides a fluent API for constructing Journal instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct JournalBuilder {
    company_id: Option<Uuid>,
    journal_number: Option<String>,
    reference_number: Option<String>,
    journal_type: Option<JournalType>,
    branch_id: Option<Uuid>,
    transaction_date: Option<NaiveDate>,
    posting_date: Option<NaiveDate>,
    fiscal_period_id: Option<Uuid>,
    fiscal_year: Option<i32>,
    fiscal_month: Option<i32>,
    description: Option<String>,
    notes: Option<String>,
    currency: Option<String>,
    total_debit: Option<Decimal>,
    total_credit: Option<Decimal>,
    line_count: Option<i32>,
    source: Option<JournalSource>,
    source_type: Option<String>,
    source_id: Option<Uuid>,
    source_reference: Option<String>,
    is_reversed: Option<bool>,
    reversed_by_id: Option<Uuid>,
    reversed_at: Option<DateTime<Utc>>,
    reversal_reason: Option<String>,
    reverses_id: Option<Uuid>,
    is_reversing: Option<bool>,
    auto_reverse: Option<bool>,
    auto_reverse_date: Option<NaiveDate>,
    status: Option<JournalStatus>,
    requires_approval: Option<bool>,
    approval_threshold: Option<Decimal>,
    submitted_at: Option<DateTime<Utc>>,
    submitted_by: Option<Uuid>,
    approved_at: Option<DateTime<Utc>>,
    approved_by: Option<Uuid>,
    rejected_at: Option<DateTime<Utc>>,
    rejected_by: Option<Uuid>,
    rejection_reason: Option<String>,
    posted_at: Option<DateTime<Utc>>,
    posted_by: Option<Uuid>,
    is_voided: Option<bool>,
    voided_at: Option<DateTime<Utc>>,
    voided_by: Option<Uuid>,
    void_reason: Option<String>,
    attachments: Option<serde_json::Value>,
}

impl JournalBuilder {
    /// Set the company_id field (required)
    pub fn company_id(mut self, value: Uuid) -> Self {
        self.company_id = Some(value);
        self
    }

    /// Set the journal_number field (required)
    pub fn journal_number(mut self, value: String) -> Self {
        self.journal_number = Some(value);
        self
    }

    /// Set the reference_number field (optional)
    pub fn reference_number(mut self, value: String) -> Self {
        self.reference_number = Some(value);
        self
    }

    /// Set the journal_type field (default: `JournalType::default()`)
    pub fn journal_type(mut self, value: JournalType) -> Self {
        self.journal_type = Some(value);
        self
    }

    /// Set the branch_id field (optional)
    pub fn branch_id(mut self, value: Uuid) -> Self {
        self.branch_id = Some(value);
        self
    }

    /// Set the transaction_date field (required)
    pub fn transaction_date(mut self, value: NaiveDate) -> Self {
        self.transaction_date = Some(value);
        self
    }

    /// Set the posting_date field (optional)
    pub fn posting_date(mut self, value: NaiveDate) -> Self {
        self.posting_date = Some(value);
        self
    }

    /// Set the fiscal_period_id field (optional)
    pub fn fiscal_period_id(mut self, value: Uuid) -> Self {
        self.fiscal_period_id = Some(value);
        self
    }

    /// Set the fiscal_year field (optional)
    pub fn fiscal_year(mut self, value: i32) -> Self {
        self.fiscal_year = Some(value);
        self
    }

    /// Set the fiscal_month field (optional)
    pub fn fiscal_month(mut self, value: i32) -> Self {
        self.fiscal_month = Some(value);
        self
    }

    /// Set the description field (required)
    pub fn description(mut self, value: String) -> Self {
        self.description = Some(value);
        self
    }

    /// Set the notes field (optional)
    pub fn notes(mut self, value: String) -> Self {
        self.notes = Some(value);
        self
    }

    /// Set the currency field (default: `"IDR".to_string()`)
    pub fn currency(mut self, value: String) -> Self {
        self.currency = Some(value);
        self
    }

    /// Set the total_debit field (default: `Decimal::from(0)`)
    pub fn total_debit(mut self, value: Decimal) -> Self {
        self.total_debit = Some(value);
        self
    }

    /// Set the total_credit field (default: `Decimal::from(0)`)
    pub fn total_credit(mut self, value: Decimal) -> Self {
        self.total_credit = Some(value);
        self
    }

    /// Set the line_count field (default: `0`)
    pub fn line_count(mut self, value: i32) -> Self {
        self.line_count = Some(value);
        self
    }

    /// Set the source field (default: `JournalSource::default()`)
    pub fn source(mut self, value: JournalSource) -> Self {
        self.source = Some(value);
        self
    }

    /// Set the source_type field (optional)
    pub fn source_type(mut self, value: String) -> Self {
        self.source_type = Some(value);
        self
    }

    /// Set the source_id field (optional)
    pub fn source_id(mut self, value: Uuid) -> Self {
        self.source_id = Some(value);
        self
    }

    /// Set the source_reference field (optional)
    pub fn source_reference(mut self, value: String) -> Self {
        self.source_reference = Some(value);
        self
    }

    /// Set the is_reversed field (default: `false`)
    pub fn is_reversed(mut self, value: bool) -> Self {
        self.is_reversed = Some(value);
        self
    }

    /// Set the reversed_by_id field (optional)
    pub fn reversed_by_id(mut self, value: Uuid) -> Self {
        self.reversed_by_id = Some(value);
        self
    }

    /// Set the reversed_at field (optional)
    pub fn reversed_at(mut self, value: DateTime<Utc>) -> Self {
        self.reversed_at = Some(value);
        self
    }

    /// Set the reversal_reason field (optional)
    pub fn reversal_reason(mut self, value: String) -> Self {
        self.reversal_reason = Some(value);
        self
    }

    /// Set the reverses_id field (optional)
    pub fn reverses_id(mut self, value: Uuid) -> Self {
        self.reverses_id = Some(value);
        self
    }

    /// Set the is_reversing field (default: `false`)
    pub fn is_reversing(mut self, value: bool) -> Self {
        self.is_reversing = Some(value);
        self
    }

    /// Set the auto_reverse field (default: `false`)
    pub fn auto_reverse(mut self, value: bool) -> Self {
        self.auto_reverse = Some(value);
        self
    }

    /// Set the auto_reverse_date field (optional)
    pub fn auto_reverse_date(mut self, value: NaiveDate) -> Self {
        self.auto_reverse_date = Some(value);
        self
    }

    /// Set the status field (default: `JournalStatus::default()`)
    pub fn status(mut self, value: JournalStatus) -> Self {
        self.status = Some(value);
        self
    }

    /// Set the requires_approval field (default: `false`)
    pub fn requires_approval(mut self, value: bool) -> Self {
        self.requires_approval = Some(value);
        self
    }

    /// Set the approval_threshold field (optional)
    pub fn approval_threshold(mut self, value: Decimal) -> Self {
        self.approval_threshold = Some(value);
        self
    }

    /// Set the submitted_at field (optional)
    pub fn submitted_at(mut self, value: DateTime<Utc>) -> Self {
        self.submitted_at = Some(value);
        self
    }

    /// Set the submitted_by field (optional)
    pub fn submitted_by(mut self, value: Uuid) -> Self {
        self.submitted_by = Some(value);
        self
    }

    /// Set the approved_at field (optional)
    pub fn approved_at(mut self, value: DateTime<Utc>) -> Self {
        self.approved_at = Some(value);
        self
    }

    /// Set the approved_by field (optional)
    pub fn approved_by(mut self, value: Uuid) -> Self {
        self.approved_by = Some(value);
        self
    }

    /// Set the rejected_at field (optional)
    pub fn rejected_at(mut self, value: DateTime<Utc>) -> Self {
        self.rejected_at = Some(value);
        self
    }

    /// Set the rejected_by field (optional)
    pub fn rejected_by(mut self, value: Uuid) -> Self {
        self.rejected_by = Some(value);
        self
    }

    /// Set the rejection_reason field (optional)
    pub fn rejection_reason(mut self, value: String) -> Self {
        self.rejection_reason = Some(value);
        self
    }

    /// Set the posted_at field (optional)
    pub fn posted_at(mut self, value: DateTime<Utc>) -> Self {
        self.posted_at = Some(value);
        self
    }

    /// Set the posted_by field (optional)
    pub fn posted_by(mut self, value: Uuid) -> Self {
        self.posted_by = Some(value);
        self
    }

    /// Set the is_voided field (default: `false`)
    pub fn is_voided(mut self, value: bool) -> Self {
        self.is_voided = Some(value);
        self
    }

    /// Set the voided_at field (optional)
    pub fn voided_at(mut self, value: DateTime<Utc>) -> Self {
        self.voided_at = Some(value);
        self
    }

    /// Set the voided_by field (optional)
    pub fn voided_by(mut self, value: Uuid) -> Self {
        self.voided_by = Some(value);
        self
    }

    /// Set the void_reason field (optional)
    pub fn void_reason(mut self, value: String) -> Self {
        self.void_reason = Some(value);
        self
    }

    /// Set the attachments field (default: `serde_json::json!([])`)
    pub fn attachments(mut self, value: serde_json::Value) -> Self {
        self.attachments = Some(value);
        self
    }

    /// Build the Journal entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<Journal, String> {
        let company_id = self.company_id.ok_or_else(|| "company_id is required".to_string())?;
        let journal_number = self.journal_number.ok_or_else(|| "journal_number is required".to_string())?;
        let transaction_date = self.transaction_date.ok_or_else(|| "transaction_date is required".to_string())?;
        let description = self.description.ok_or_else(|| "description is required".to_string())?;

        Ok(Journal {
            id: Uuid::new_v4(),
            company_id,
            journal_number,
            reference_number: self.reference_number,
            journal_type: self.journal_type.unwrap_or(JournalType::default()),
            branch_id: self.branch_id,
            transaction_date,
            posting_date: self.posting_date,
            fiscal_period_id: self.fiscal_period_id,
            fiscal_year: self.fiscal_year,
            fiscal_month: self.fiscal_month,
            description,
            notes: self.notes,
            currency: self.currency.unwrap_or("IDR".to_string()),
            total_debit: self.total_debit.unwrap_or(Decimal::from(0)),
            total_credit: self.total_credit.unwrap_or(Decimal::from(0)),
            line_count: self.line_count.unwrap_or(0),
            source: self.source.unwrap_or(JournalSource::default()),
            source_type: self.source_type,
            source_id: self.source_id,
            source_reference: self.source_reference,
            is_reversed: self.is_reversed.unwrap_or(false),
            reversed_by_id: self.reversed_by_id,
            reversed_at: self.reversed_at,
            reversal_reason: self.reversal_reason,
            reverses_id: self.reverses_id,
            is_reversing: self.is_reversing.unwrap_or(false),
            auto_reverse: self.auto_reverse.unwrap_or(false),
            auto_reverse_date: self.auto_reverse_date,
            status: self.status.unwrap_or(JournalStatus::default()),
            requires_approval: self.requires_approval.unwrap_or(false),
            approval_threshold: self.approval_threshold,
            submitted_at: self.submitted_at,
            submitted_by: self.submitted_by,
            approved_at: self.approved_at,
            approved_by: self.approved_by,
            rejected_at: self.rejected_at,
            rejected_by: self.rejected_by,
            rejection_reason: self.rejection_reason,
            posted_at: self.posted_at,
            posted_by: self.posted_by,
            is_voided: self.is_voided.unwrap_or(false),
            voided_at: self.voided_at,
            voided_by: self.voided_by,
            void_reason: self.void_reason,
            attachments: self.attachments.unwrap_or(serde_json::json!([])),
            metadata: AuditMetadata::default(),
        })
    }
}
