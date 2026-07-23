use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use rust_decimal::Decimal;

use super::PostingSourceType;
use super::PostingType;
use super::PostingStatus;
use super::AuditMetadata;

/// Strongly-typed ID for AccountingPost
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AccountingPostId(pub Uuid);

impl AccountingPostId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for AccountingPostId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for AccountingPostId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for AccountingPostId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<AccountingPostId> for Uuid {
    fn from(id: AccountingPostId) -> Self { id.0 }
}

impl AsRef<Uuid> for AccountingPostId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for AccountingPostId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AccountingPost {
    pub id: Uuid,
    pub company_id: Uuid,
    pub branch_id: Option<Uuid>,
    pub source_type: PostingSourceType,
    pub source_id: Uuid,
    pub source_reference: Option<String>,
    pub journal_id: Option<Uuid>,
    pub posting_type: PostingType,
    pub posting_status: PostingStatus,
    pub currency: String,
    pub total_debit: Decimal,
    pub total_credit: Decimal,
    pub scheduled_at: Option<DateTime<Utc>>,
    pub posted_at: Option<DateTime<Utc>>,
    pub failed_at: Option<DateTime<Utc>>,
    pub retry_count: i32,
    pub max_retries: i32,
    pub next_retry_at: Option<DateTime<Utc>>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub reverses_post_id: Option<Uuid>,
    pub reversed_by_post_id: Option<Uuid>,
    pub posted_by: Option<Uuid>,
    pub notes: Option<String>,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl AccountingPost {
    /// Create a builder for AccountingPost
    pub fn builder() -> AccountingPostBuilder {
        AccountingPostBuilder::default()
    }

    /// Create a new AccountingPost with required fields
    pub fn new(company_id: Uuid, source_type: PostingSourceType, source_id: Uuid, posting_type: PostingType, posting_status: PostingStatus, currency: String, total_debit: Decimal, total_credit: Decimal, retry_count: i32, max_retries: i32) -> Self {
        Self {
            id: Uuid::new_v4(),
            company_id,
            branch_id: None,
            source_type,
            source_id,
            source_reference: None,
            journal_id: None,
            posting_type,
            posting_status,
            currency,
            total_debit,
            total_credit,
            scheduled_at: None,
            posted_at: None,
            failed_at: None,
            retry_count,
            max_retries,
            next_retry_at: None,
            error_code: None,
            error_message: None,
            reverses_post_id: None,
            reversed_by_post_id: None,
            posted_by: None,
            notes: None,
            metadata: AuditMetadata::default(),
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> AccountingPostId {
        AccountingPostId(self.id)
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


    // ==========================================================
    // Fluent Setters (with_* for optional fields)
    // ==========================================================

    /// Set the branch_id field (chainable)
    pub fn with_branch_id(mut self, value: Uuid) -> Self {
        self.branch_id = Some(value);
        self
    }

    /// Set the source_reference field (chainable)
    pub fn with_source_reference(mut self, value: String) -> Self {
        self.source_reference = Some(value);
        self
    }

    /// Set the journal_id field (chainable)
    pub fn with_journal_id(mut self, value: Uuid) -> Self {
        self.journal_id = Some(value);
        self
    }

    /// Set the scheduled_at field (chainable)
    pub fn with_scheduled_at(mut self, value: DateTime<Utc>) -> Self {
        self.scheduled_at = Some(value);
        self
    }

    /// Set the posted_at field (chainable)
    pub fn with_posted_at(mut self, value: DateTime<Utc>) -> Self {
        self.posted_at = Some(value);
        self
    }

    /// Set the failed_at field (chainable)
    pub fn with_failed_at(mut self, value: DateTime<Utc>) -> Self {
        self.failed_at = Some(value);
        self
    }

    /// Set the next_retry_at field (chainable)
    pub fn with_next_retry_at(mut self, value: DateTime<Utc>) -> Self {
        self.next_retry_at = Some(value);
        self
    }

    /// Set the error_code field (chainable)
    pub fn with_error_code(mut self, value: String) -> Self {
        self.error_code = Some(value);
        self
    }

    /// Set the error_message field (chainable)
    pub fn with_error_message(mut self, value: String) -> Self {
        self.error_message = Some(value);
        self
    }

    /// Set the reverses_post_id field (chainable)
    pub fn with_reverses_post_id(mut self, value: Uuid) -> Self {
        self.reverses_post_id = Some(value);
        self
    }

    /// Set the reversed_by_post_id field (chainable)
    pub fn with_reversed_by_post_id(mut self, value: Uuid) -> Self {
        self.reversed_by_post_id = Some(value);
        self
    }

    /// Set the posted_by field (chainable)
    pub fn with_posted_by(mut self, value: Uuid) -> Self {
        self.posted_by = Some(value);
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
                "company_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.company_id = v; }
                }
                "branch_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.branch_id = v; }
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
                "journal_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.journal_id = v; }
                }
                "posting_type" => {
                    if let Ok(v) = serde_json::from_value(value) { self.posting_type = v; }
                }
                "posting_status" => {
                    if let Ok(v) = serde_json::from_value(value) { self.posting_status = v; }
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
                "scheduled_at" => {
                    if let Ok(v) = serde_json::from_value(value) { self.scheduled_at = v; }
                }
                "posted_at" => {
                    if let Ok(v) = serde_json::from_value(value) { self.posted_at = v; }
                }
                "failed_at" => {
                    if let Ok(v) = serde_json::from_value(value) { self.failed_at = v; }
                }
                "retry_count" => {
                    if let Ok(v) = serde_json::from_value(value) { self.retry_count = v; }
                }
                "max_retries" => {
                    if let Ok(v) = serde_json::from_value(value) { self.max_retries = v; }
                }
                "next_retry_at" => {
                    if let Ok(v) = serde_json::from_value(value) { self.next_retry_at = v; }
                }
                "error_code" => {
                    if let Ok(v) = serde_json::from_value(value) { self.error_code = v; }
                }
                "error_message" => {
                    if let Ok(v) = serde_json::from_value(value) { self.error_message = v; }
                }
                "reverses_post_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.reverses_post_id = v; }
                }
                "reversed_by_post_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.reversed_by_post_id = v; }
                }
                "posted_by" => {
                    if let Ok(v) = serde_json::from_value(value) { self.posted_by = v; }
                }
                "notes" => {
                    if let Ok(v) = serde_json::from_value(value) { self.notes = v; }
                }
                _ => {} // ignore unknown fields
            }
        }
    }

    // <<< CUSTOM METHODS START >>>
    // <<< CUSTOM METHODS END >>>
}

impl super::Entity for AccountingPost {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "AccountingPost"
    }
}

impl backbone_core::PersistentEntity for AccountingPost {
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

impl backbone_orm::EntityRepoMeta for AccountingPost {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("company_id".to_string(), "uuid".to_string());
        m.insert("branch_id".to_string(), "uuid".to_string());
        m.insert("source_id".to_string(), "uuid".to_string());
        m.insert("journal_id".to_string(), "uuid".to_string());
        m.insert("reverses_post_id".to_string(), "uuid".to_string());
        m.insert("reversed_by_post_id".to_string(), "uuid".to_string());
        m.insert("source_type".to_string(), "posting_source_type".to_string());
        m.insert("posting_type".to_string(), "posting_type".to_string());
        m.insert("posting_status".to_string(), "posting_status".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &["currency"]
    }
    fn company_field() -> Option<&'static str> {
        Some("company_id")
    }
    fn relations() -> &'static [(&'static str, &'static str, &'static str)] {
        &[("journal", "journals", "journalId"), ("reversesPost", "accounting_posts", "reversesPostId"), ("reversedByPost", "accounting_posts", "reversedByPostId")]
    }
}

/// Builder for AccountingPost entity
///
/// Provides a fluent API for constructing AccountingPost instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct AccountingPostBuilder {
    company_id: Option<Uuid>,
    branch_id: Option<Uuid>,
    source_type: Option<PostingSourceType>,
    source_id: Option<Uuid>,
    source_reference: Option<String>,
    journal_id: Option<Uuid>,
    posting_type: Option<PostingType>,
    posting_status: Option<PostingStatus>,
    currency: Option<String>,
    total_debit: Option<Decimal>,
    total_credit: Option<Decimal>,
    scheduled_at: Option<DateTime<Utc>>,
    posted_at: Option<DateTime<Utc>>,
    failed_at: Option<DateTime<Utc>>,
    retry_count: Option<i32>,
    max_retries: Option<i32>,
    next_retry_at: Option<DateTime<Utc>>,
    error_code: Option<String>,
    error_message: Option<String>,
    reverses_post_id: Option<Uuid>,
    reversed_by_post_id: Option<Uuid>,
    posted_by: Option<Uuid>,
    notes: Option<String>,
}

impl AccountingPostBuilder {
    /// Set the company_id field (required)
    pub fn company_id(mut self, value: Uuid) -> Self {
        self.company_id = Some(value);
        self
    }

    /// Set the branch_id field (optional)
    pub fn branch_id(mut self, value: Uuid) -> Self {
        self.branch_id = Some(value);
        self
    }

    /// Set the source_type field (required)
    pub fn source_type(mut self, value: PostingSourceType) -> Self {
        self.source_type = Some(value);
        self
    }

    /// Set the source_id field (required)
    pub fn source_id(mut self, value: Uuid) -> Self {
        self.source_id = Some(value);
        self
    }

    /// Set the source_reference field (optional)
    pub fn source_reference(mut self, value: String) -> Self {
        self.source_reference = Some(value);
        self
    }

    /// Set the journal_id field (optional)
    pub fn journal_id(mut self, value: Uuid) -> Self {
        self.journal_id = Some(value);
        self
    }

    /// Set the posting_type field (default: `PostingType::default()`)
    pub fn posting_type(mut self, value: PostingType) -> Self {
        self.posting_type = Some(value);
        self
    }

    /// Set the posting_status field (default: `PostingStatus::default()`)
    pub fn posting_status(mut self, value: PostingStatus) -> Self {
        self.posting_status = Some(value);
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

    /// Set the scheduled_at field (optional)
    pub fn scheduled_at(mut self, value: DateTime<Utc>) -> Self {
        self.scheduled_at = Some(value);
        self
    }

    /// Set the posted_at field (optional)
    pub fn posted_at(mut self, value: DateTime<Utc>) -> Self {
        self.posted_at = Some(value);
        self
    }

    /// Set the failed_at field (optional)
    pub fn failed_at(mut self, value: DateTime<Utc>) -> Self {
        self.failed_at = Some(value);
        self
    }

    /// Set the retry_count field (default: `0`)
    pub fn retry_count(mut self, value: i32) -> Self {
        self.retry_count = Some(value);
        self
    }

    /// Set the max_retries field (default: `3`)
    pub fn max_retries(mut self, value: i32) -> Self {
        self.max_retries = Some(value);
        self
    }

    /// Set the next_retry_at field (optional)
    pub fn next_retry_at(mut self, value: DateTime<Utc>) -> Self {
        self.next_retry_at = Some(value);
        self
    }

    /// Set the error_code field (optional)
    pub fn error_code(mut self, value: String) -> Self {
        self.error_code = Some(value);
        self
    }

    /// Set the error_message field (optional)
    pub fn error_message(mut self, value: String) -> Self {
        self.error_message = Some(value);
        self
    }

    /// Set the reverses_post_id field (optional)
    pub fn reverses_post_id(mut self, value: Uuid) -> Self {
        self.reverses_post_id = Some(value);
        self
    }

    /// Set the reversed_by_post_id field (optional)
    pub fn reversed_by_post_id(mut self, value: Uuid) -> Self {
        self.reversed_by_post_id = Some(value);
        self
    }

    /// Set the posted_by field (optional)
    pub fn posted_by(mut self, value: Uuid) -> Self {
        self.posted_by = Some(value);
        self
    }

    /// Set the notes field (optional)
    pub fn notes(mut self, value: String) -> Self {
        self.notes = Some(value);
        self
    }

    /// Build the AccountingPost entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<AccountingPost, String> {
        let company_id = self.company_id.ok_or_else(|| "company_id is required".to_string())?;
        let source_type = self.source_type.ok_or_else(|| "source_type is required".to_string())?;
        let source_id = self.source_id.ok_or_else(|| "source_id is required".to_string())?;

        Ok(AccountingPost {
            id: Uuid::new_v4(),
            company_id,
            branch_id: self.branch_id,
            source_type,
            source_id,
            source_reference: self.source_reference,
            journal_id: self.journal_id,
            posting_type: self.posting_type.unwrap_or(PostingType::default()),
            posting_status: self.posting_status.unwrap_or(PostingStatus::default()),
            currency: self.currency.unwrap_or("IDR".to_string()),
            total_debit: self.total_debit.unwrap_or(Decimal::from(0)),
            total_credit: self.total_credit.unwrap_or(Decimal::from(0)),
            scheduled_at: self.scheduled_at,
            posted_at: self.posted_at,
            failed_at: self.failed_at,
            retry_count: self.retry_count.unwrap_or(0),
            max_retries: self.max_retries.unwrap_or(3),
            next_retry_at: self.next_retry_at,
            error_code: self.error_code,
            error_message: self.error_message,
            reverses_post_id: self.reverses_post_id,
            reversed_by_post_id: self.reversed_by_post_id,
            posted_by: self.posted_by,
            notes: self.notes,
            metadata: AuditMetadata::default(),
        })
    }
}
