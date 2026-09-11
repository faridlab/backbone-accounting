use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use rust_decimal::Decimal;

/// Strongly-typed ID for PrintedCheck
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PrintedCheckId(pub Uuid);

impl PrintedCheckId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for PrintedCheckId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for PrintedCheckId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for PrintedCheckId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<PrintedCheckId> for Uuid {
    fn from(id: PrintedCheckId) -> Self { id.0 }
}

impl AsRef<Uuid> for PrintedCheckId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for PrintedCheckId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct PrintedCheck {
    pub id: Uuid,
    pub bank_account_id: Uuid,
    pub payment_id: Uuid,
    pub payment_number: Option<String>,
    pub check_number: String,
    pub amount: Decimal,
    pub payee_name: Option<String>,
    pub status: String,
    pub printed_at: DateTime<Utc>,
    pub printed_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub metadata: serde_json::Value,
}

impl PrintedCheck {
    /// Create a builder for PrintedCheck
    pub fn builder() -> PrintedCheckBuilder {
        <PrintedCheckBuilder as Default>::default()
    }

    /// Create a new PrintedCheck with required fields
    pub fn new(bank_account_id: Uuid, payment_id: Uuid, check_number: String, amount: Decimal, status: String, printed_at: DateTime<Utc>, metadata: serde_json::Value) -> Self {
        Self {
            id: Uuid::new_v4(),
            bank_account_id,
            payment_id,
            payment_number: None,
            check_number,
            amount,
            payee_name: None,
            status,
            printed_at,
            printed_by: None,
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
    pub fn typed_id(&self) -> PrintedCheckId {
        PrintedCheckId(self.id)
    }

    /// Get when this entity was created
    pub fn created_at(&self) -> &DateTime<Utc> {
        &self.created_at
    }

    /// Get when this entity was last updated
    pub fn updated_at(&self) -> &DateTime<Utc> {
        &self.updated_at
    }

    /// Get the current status
    pub fn status(&self) -> &String {
        &self.status
    }


    // ==========================================================
    // Fluent Setters (with_* for optional fields)
    // ==========================================================

    /// Set the payment_number field (chainable)
    pub fn with_payment_number(mut self, value: String) -> Self {
        self.payment_number = Some(value);
        self
    }

    /// Set the payee_name field (chainable)
    pub fn with_payee_name(mut self, value: String) -> Self {
        self.payee_name = Some(value);
        self
    }

    /// Set the printed_by field (chainable)
    pub fn with_printed_by(mut self, value: Uuid) -> Self {
        self.printed_by = Some(value);
        self
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
                "payment_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.payment_id = v; }
                }
                "payment_number" => {
                    if let Ok(v) = serde_json::from_value(value) { self.payment_number = v; }
                }
                "check_number" => {
                    if let Ok(v) = serde_json::from_value(value) { self.check_number = v; }
                }
                "amount" => {
                    if let Ok(v) = serde_json::from_value(value) { self.amount = v; }
                }
                "payee_name" => {
                    if let Ok(v) = serde_json::from_value(value) { self.payee_name = v; }
                }
                "status" => {
                    if let Ok(v) = serde_json::from_value(value) { self.status = v; }
                }
                "printed_at" => {
                    if let Ok(v) = serde_json::from_value(value) { self.printed_at = v; }
                }
                "printed_by" => {
                    if let Ok(v) = serde_json::from_value(value) { self.printed_by = v; }
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

impl super::Entity for PrintedCheck {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "PrintedCheck"
    }
}

impl backbone_core::PersistentEntity for PrintedCheck {
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

impl backbone_orm::EntityRepoMeta for PrintedCheck {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("bank_account_id".to_string(), "uuid".to_string());
        m.insert("payment_id".to_string(), "uuid".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &["check_number", "status"]
    }
}

/// Builder for PrintedCheck entity
///
/// Provides a fluent API for constructing PrintedCheck instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct PrintedCheckBuilder {
    bank_account_id: Option<Uuid>,
    payment_id: Option<Uuid>,
    payment_number: Option<String>,
    check_number: Option<String>,
    amount: Option<Decimal>,
    payee_name: Option<String>,
    status: Option<String>,
    printed_at: Option<DateTime<Utc>>,
    printed_by: Option<Uuid>,
    metadata: Option<serde_json::Value>,
}

impl PrintedCheckBuilder {
    /// Set the bank_account_id field (required)
    pub fn bank_account_id(mut self, value: Uuid) -> Self {
        self.bank_account_id = Some(value);
        self
    }

    /// Set the payment_id field (required)
    pub fn payment_id(mut self, value: Uuid) -> Self {
        self.payment_id = Some(value);
        self
    }

    /// Set the payment_number field (optional)
    pub fn payment_number(mut self, value: String) -> Self {
        self.payment_number = Some(value);
        self
    }

    /// Set the check_number field (required)
    pub fn check_number(mut self, value: String) -> Self {
        self.check_number = Some(value);
        self
    }

    /// Set the amount field (required)
    pub fn amount(mut self, value: Decimal) -> Self {
        self.amount = Some(value);
        self
    }

    /// Set the payee_name field (optional)
    pub fn payee_name(mut self, value: String) -> Self {
        self.payee_name = Some(value);
        self
    }

    /// Set the status field (default: `Default::default()`)
    pub fn status(mut self, value: String) -> Self {
        self.status = Some(value);
        self
    }

    /// Set the printed_at field (default: `Utc::now()`)
    pub fn printed_at(mut self, value: DateTime<Utc>) -> Self {
        self.printed_at = Some(value);
        self
    }

    /// Set the printed_by field (optional)
    pub fn printed_by(mut self, value: Uuid) -> Self {
        self.printed_by = Some(value);
        self
    }

    /// Set the metadata field (default: `serde_json::json!({})`)
    pub fn metadata(mut self, value: serde_json::Value) -> Self {
        self.metadata = Some(value);
        self
    }

    /// Build the PrintedCheck entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<PrintedCheck, String> {
        let bank_account_id = self.bank_account_id.ok_or_else(|| "bank_account_id is required".to_string())?;
        let payment_id = self.payment_id.ok_or_else(|| "payment_id is required".to_string())?;
        let check_number = self.check_number.ok_or_else(|| "check_number is required".to_string())?;
        let amount = self.amount.ok_or_else(|| "amount is required".to_string())?;

        Ok(PrintedCheck {
            id: Uuid::new_v4(),
            bank_account_id,
            payment_id,
            payment_number: self.payment_number,
            check_number,
            amount,
            payee_name: self.payee_name,
            status: self.status.unwrap_or_default(),
            printed_at: self.printed_at.unwrap_or(Utc::now()),
            printed_by: self.printed_by,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            metadata: self.metadata.unwrap_or(serde_json::json!({})),
        })
    }
}
