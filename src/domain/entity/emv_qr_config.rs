use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// Strongly-typed ID for EmvQrConfig
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EmvQrConfigId(pub Uuid);

impl EmvQrConfigId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for EmvQrConfigId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for EmvQrConfigId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for EmvQrConfigId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<EmvQrConfigId> for Uuid {
    fn from(id: EmvQrConfigId) -> Self { id.0 }
}

impl AsRef<Uuid> for EmvQrConfigId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for EmvQrConfigId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct EmvQrConfig {
    pub id: Uuid,
    pub merchant_name: String,
    pub merchant_city: String,
    pub country_code: String,
    pub mcc: String,
    pub gui: String,
    pub merchant_identifier: String,
    pub currency: String,
    pub initiation_method: String,
    pub bank_account_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub metadata: serde_json::Value,
}

impl EmvQrConfig {
    /// Create a builder for EmvQrConfig
    pub fn builder() -> EmvQrConfigBuilder {
        <EmvQrConfigBuilder as Default>::default()
    }

    /// Create a new EmvQrConfig with required fields
    pub fn new(merchant_name: String, merchant_city: String, country_code: String, mcc: String, gui: String, merchant_identifier: String, currency: String, initiation_method: String, metadata: serde_json::Value) -> Self {
        Self {
            id: Uuid::new_v4(),
            merchant_name,
            merchant_city,
            country_code,
            mcc,
            gui,
            merchant_identifier,
            currency,
            initiation_method,
            bank_account_id: None,
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
    pub fn typed_id(&self) -> EmvQrConfigId {
        EmvQrConfigId(self.id)
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

    /// Set the bank_account_id field (chainable)
    pub fn with_bank_account_id(mut self, value: Uuid) -> Self {
        self.bank_account_id = Some(value);
        self
    }

    // ==========================================================
    // Partial Update
    // ==========================================================

    /// Apply partial updates from a map of field name to JSON value
    pub fn apply_patch(&mut self, fields: std::collections::HashMap<String, serde_json::Value>) {
        for (key, value) in fields {
            match key.as_str() {
                "merchant_name" => {
                    if let Ok(v) = serde_json::from_value(value) { self.merchant_name = v; }
                }
                "merchant_city" => {
                    if let Ok(v) = serde_json::from_value(value) { self.merchant_city = v; }
                }
                "country_code" => {
                    if let Ok(v) = serde_json::from_value(value) { self.country_code = v; }
                }
                "mcc" => {
                    if let Ok(v) = serde_json::from_value(value) { self.mcc = v; }
                }
                "gui" => {
                    if let Ok(v) = serde_json::from_value(value) { self.gui = v; }
                }
                "merchant_identifier" => {
                    if let Ok(v) = serde_json::from_value(value) { self.merchant_identifier = v; }
                }
                "currency" => {
                    if let Ok(v) = serde_json::from_value(value) { self.currency = v; }
                }
                "initiation_method" => {
                    if let Ok(v) = serde_json::from_value(value) { self.initiation_method = v; }
                }
                "bank_account_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.bank_account_id = v; }
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

impl super::Entity for EmvQrConfig {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "EmvQrConfig"
    }
}

impl backbone_core::PersistentEntity for EmvQrConfig {
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

impl backbone_orm::EntityRepoMeta for EmvQrConfig {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("bank_account_id".to_string(), "uuid".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &["merchant_name", "merchant_city", "country_code", "mcc", "gui", "merchant_identifier", "currency", "initiation_method"]
    }
}

/// Builder for EmvQrConfig entity
///
/// Provides a fluent API for constructing EmvQrConfig instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct EmvQrConfigBuilder {
    merchant_name: Option<String>,
    merchant_city: Option<String>,
    country_code: Option<String>,
    mcc: Option<String>,
    gui: Option<String>,
    merchant_identifier: Option<String>,
    currency: Option<String>,
    initiation_method: Option<String>,
    bank_account_id: Option<Uuid>,
    metadata: Option<serde_json::Value>,
}

impl EmvQrConfigBuilder {
    /// Set the merchant_name field (required)
    pub fn merchant_name(mut self, value: String) -> Self {
        self.merchant_name = Some(value);
        self
    }

    /// Set the merchant_city field (required)
    pub fn merchant_city(mut self, value: String) -> Self {
        self.merchant_city = Some(value);
        self
    }

    /// Set the country_code field (default: `"ID".to_string()`)
    pub fn country_code(mut self, value: String) -> Self {
        self.country_code = Some(value);
        self
    }

    /// Set the mcc field (required)
    pub fn mcc(mut self, value: String) -> Self {
        self.mcc = Some(value);
        self
    }

    /// Set the gui field (default: `"ID.CO.QRIS.WWW".to_string()`)
    pub fn gui(mut self, value: String) -> Self {
        self.gui = Some(value);
        self
    }

    /// Set the merchant_identifier field (required)
    pub fn merchant_identifier(mut self, value: String) -> Self {
        self.merchant_identifier = Some(value);
        self
    }

    /// Set the currency field (default: `"IDR".to_string()`)
    pub fn currency(mut self, value: String) -> Self {
        self.currency = Some(value);
        self
    }

    /// Set the initiation_method field (default: `"11".to_string()`)
    pub fn initiation_method(mut self, value: String) -> Self {
        self.initiation_method = Some(value);
        self
    }

    /// Set the bank_account_id field (optional)
    pub fn bank_account_id(mut self, value: Uuid) -> Self {
        self.bank_account_id = Some(value);
        self
    }

    /// Set the metadata field (default: `serde_json::json!({})`)
    pub fn metadata(mut self, value: serde_json::Value) -> Self {
        self.metadata = Some(value);
        self
    }

    /// Build the EmvQrConfig entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<EmvQrConfig, String> {
        let merchant_name = self.merchant_name.ok_or_else(|| "merchant_name is required".to_string())?;
        let merchant_city = self.merchant_city.ok_or_else(|| "merchant_city is required".to_string())?;
        let mcc = self.mcc.ok_or_else(|| "mcc is required".to_string())?;
        let merchant_identifier = self.merchant_identifier.ok_or_else(|| "merchant_identifier is required".to_string())?;

        Ok(EmvQrConfig {
            id: Uuid::new_v4(),
            merchant_name,
            merchant_city,
            country_code: self.country_code.unwrap_or("ID".to_string()),
            mcc,
            gui: self.gui.unwrap_or("ID.CO.QRIS.WWW".to_string()),
            merchant_identifier,
            currency: self.currency.unwrap_or("IDR".to_string()),
            initiation_method: self.initiation_method.unwrap_or("11".to_string()),
            bank_account_id: self.bank_account_id,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            metadata: self.metadata.unwrap_or(serde_json::json!({})),
        })
    }
}
