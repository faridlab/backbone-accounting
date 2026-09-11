use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// Strongly-typed ID for BankCheckSequence
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BankCheckSequenceId(pub Uuid);

impl BankCheckSequenceId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for BankCheckSequenceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for BankCheckSequenceId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for BankCheckSequenceId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<BankCheckSequenceId> for Uuid {
    fn from(id: BankCheckSequenceId) -> Self { id.0 }
}

impl AsRef<Uuid> for BankCheckSequenceId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for BankCheckSequenceId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct BankCheckSequence {
    pub id: Uuid,
    pub bank_account_id: Uuid,
    pub numbering_mode: String,
    pub next_number: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub metadata: serde_json::Value,
}

impl BankCheckSequence {
    /// Create a builder for BankCheckSequence
    pub fn builder() -> BankCheckSequenceBuilder {
        <BankCheckSequenceBuilder as Default>::default()
    }

    /// Create a new BankCheckSequence with required fields
    pub fn new(bank_account_id: Uuid, numbering_mode: String, next_number: i64, metadata: serde_json::Value) -> Self {
        Self {
            id: Uuid::new_v4(),
            bank_account_id,
            numbering_mode,
            next_number,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            metadata,
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> BankCheckSequenceId {
        BankCheckSequenceId(self.id)
    }

    /// Get when this entity was created
    pub fn created_at(&self) -> &DateTime<Utc> {
        &self.created_at
    }

    /// Get when this entity was last updated
    pub fn updated_at(&self) -> &DateTime<Utc> {
        &self.updated_at
    }


    // ==========================================================
    // Partial Update
    // ==========================================================

    /// Apply partial updates from a map of field name to JSON value
    pub fn apply_patch(&mut self, fields: std::collections::HashMap<String, serde_json::Value>) {
        for (key, value) in fields {
            match key.as_str() {
                "bank_account_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.bank_account_id = v; }
                }
                "numbering_mode" => {
                    if let Ok(v) = serde_json::from_value(value) { self.numbering_mode = v; }
                }
                "next_number" => {
                    if let Ok(v) = serde_json::from_value(value) { self.next_number = v; }
                }
                "metadata" => {
                    if let Ok(v) = serde_json::from_value(value) { self.metadata = v; }
                }
                _ => {} // ignore unknown fields
            }
        }
    }

    // <<< CUSTOM METHODS START >>>
    // <<< CUSTOM METHODS END >>>
}

impl super::Entity for BankCheckSequence {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "BankCheckSequence"
    }
}

impl backbone_core::PersistentEntity for BankCheckSequence {
    fn entity_id(&self) -> String {
        self.id.to_string()
    }
    fn set_entity_id(&mut self, id: String) {
        if let Ok(uuid) = uuid::Uuid::parse_str(&id) {
            self.id = uuid;
        }
    }
    fn created_at(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        Some(self.created_at)
    }
    fn set_created_at(&mut self, ts: chrono::DateTime<chrono::Utc>) {
        self.created_at = ts;
    }
    fn updated_at(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        Some(self.updated_at)
    }
    fn set_updated_at(&mut self, ts: chrono::DateTime<chrono::Utc>) {
        self.updated_at = ts;
    }
    fn deleted_at(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        None
    }
    fn set_deleted_at(&mut self, ts: Option<chrono::DateTime<chrono::Utc>>) {
        let _ = ts;
    }
}

impl backbone_orm::EntityRepoMeta for BankCheckSequence {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("bank_account_id".to_string(), "uuid".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &["numbering_mode"]
    }
}

/// Builder for BankCheckSequence entity
///
/// Provides a fluent API for constructing BankCheckSequence instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct BankCheckSequenceBuilder {
    bank_account_id: Option<Uuid>,
    numbering_mode: Option<String>,
    next_number: Option<i64>,
    metadata: Option<serde_json::Value>,
}

impl BankCheckSequenceBuilder {
    /// Set the bank_account_id field (required)
    pub fn bank_account_id(mut self, value: Uuid) -> Self {
        self.bank_account_id = Some(value);
        self
    }

    /// Set the numbering_mode field (default: `Default::default()`)
    pub fn numbering_mode(mut self, value: String) -> Self {
        self.numbering_mode = Some(value);
        self
    }

    /// Set the next_number field (default: `1`)
    pub fn next_number(mut self, value: i64) -> Self {
        self.next_number = Some(value);
        self
    }

    /// Set the metadata field (default: `serde_json::json!({})`)
    pub fn metadata(mut self, value: serde_json::Value) -> Self {
        self.metadata = Some(value);
        self
    }

    /// Build the BankCheckSequence entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<BankCheckSequence, String> {
        let bank_account_id = self.bank_account_id.ok_or_else(|| "bank_account_id is required".to_string())?;

        Ok(BankCheckSequence {
            id: Uuid::new_v4(),
            bank_account_id,
            numbering_mode: self.numbering_mode.unwrap_or_default(),
            next_number: self.next_number.unwrap_or(1),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            metadata: self.metadata.unwrap_or(serde_json::json!({})),
        })
    }
}
