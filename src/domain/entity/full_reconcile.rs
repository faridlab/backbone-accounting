use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// Strongly-typed ID for FullReconcile
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct FullReconcileId(pub Uuid);

impl FullReconcileId {
    pub fn new(id: Uuid) -> Self {
        Self(id)
    }
    pub fn generate() -> Self {
        Self(Uuid::new_v4())
    }
    pub fn into_inner(self) -> Uuid {
        self.0
    }
}

impl std::fmt::Display for FullReconcileId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for FullReconcileId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for FullReconcileId {
    fn from(id: Uuid) -> Self {
        Self(id)
    }
}

impl From<FullReconcileId> for Uuid {
    fn from(id: FullReconcileId) -> Self {
        id.0
    }
}

impl AsRef<Uuid> for FullReconcileId {
    fn as_ref(&self) -> &Uuid {
        &self.0
    }
}

impl std::ops::Deref for FullReconcileId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct FullReconcile {
    pub id: Uuid,
    pub company_id: Uuid,
    pub exchange_total: Decimal,
    pub reconciled_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub metadata: Option<serde_json::Value>,
}

impl FullReconcile {
    /// Create a builder for FullReconcile
    pub fn builder() -> FullReconcileBuilder {
        <FullReconcileBuilder as Default>::default()
    }

    /// Create a new FullReconcile with required fields
    pub fn new(company_id: Uuid, exchange_total: Decimal, reconciled_at: DateTime<Utc>) -> Self {
        Self {
            id: Uuid::new_v4(),
            company_id,
            exchange_total,
            reconciled_at,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            metadata: None,
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> FullReconcileId {
        FullReconcileId(self.id)
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
    // Fluent Setters (with_* for optional fields)
    // ==========================================================

    /// Set the metadata field (chainable)
    pub fn with_metadata(mut self, value: serde_json::Value) -> Self {
        self.metadata = Some(value);
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
                    if let Ok(v) = serde_json::from_value(value) {
                        self.company_id = v;
                    }
                }
                "exchange_total" => {
                    if let Ok(v) = serde_json::from_value(value) {
                        self.exchange_total = v;
                    }
                }
                "reconciled_at" => {
                    if let Ok(v) = serde_json::from_value(value) {
                        self.reconciled_at = v;
                    }
                }
                "metadata" => {
                    if let Ok(v) = serde_json::from_value(value) {
                        self.metadata = v;
                    }
                }
                _ => {} // ignore unknown fields
            }
        }
    }

    // <<< CUSTOM METHODS START >>>
    // <<< CUSTOM METHODS END >>>
}

impl super::Entity for FullReconcile {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "FullReconcile"
    }
}

impl backbone_core::PersistentEntity for FullReconcile {
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

impl backbone_orm::EntityRepoMeta for FullReconcile {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("company_id".to_string(), "uuid".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &[]
    }
    fn company_field() -> Option<&'static str> {
        Some("company_id")
    }
}

/// Builder for FullReconcile entity
///
/// Provides a fluent API for constructing FullReconcile instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct FullReconcileBuilder {
    company_id: Option<Uuid>,
    exchange_total: Option<Decimal>,
    reconciled_at: Option<DateTime<Utc>>,
    metadata: Option<serde_json::Value>,
}

impl FullReconcileBuilder {
    /// Set the company_id field (required)
    pub fn company_id(mut self, value: Uuid) -> Self {
        self.company_id = Some(value);
        self
    }

    /// Set the exchange_total field (default: `Decimal::from(0)`)
    pub fn exchange_total(mut self, value: Decimal) -> Self {
        self.exchange_total = Some(value);
        self
    }

    /// Set the reconciled_at field (default: `Utc::now()`)
    pub fn reconciled_at(mut self, value: DateTime<Utc>) -> Self {
        self.reconciled_at = Some(value);
        self
    }

    /// Set the metadata field (optional)
    pub fn metadata(mut self, value: serde_json::Value) -> Self {
        self.metadata = Some(value);
        self
    }

    /// Build the FullReconcile entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<FullReconcile, String> {
        let company_id = self
            .company_id
            .ok_or_else(|| "company_id is required".to_string())?;

        Ok(FullReconcile {
            id: Uuid::new_v4(),
            company_id,
            exchange_total: self.exchange_total.unwrap_or(Decimal::from(0)),
            reconciled_at: self.reconciled_at.unwrap_or(Utc::now()),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            metadata: self.metadata,
        })
    }
}
