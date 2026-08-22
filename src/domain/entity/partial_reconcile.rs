use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use super::ReconcileOrigin;

/// Strongly-typed ID for PartialReconcile
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PartialReconcileId(pub Uuid);

impl PartialReconcileId {
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

impl std::fmt::Display for PartialReconcileId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for PartialReconcileId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for PartialReconcileId {
    fn from(id: Uuid) -> Self {
        Self(id)
    }
}

impl From<PartialReconcileId> for Uuid {
    fn from(id: PartialReconcileId) -> Self {
        id.0
    }
}

impl AsRef<Uuid> for PartialReconcileId {
    fn as_ref(&self) -> &Uuid {
        &self.0
    }
}

impl std::ops::Deref for PartialReconcileId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct PartialReconcile {
    pub id: Uuid,
    pub company_id: Uuid,
    pub debit_move_id: Uuid,
    pub credit_move_id: Uuid,
    pub full_reconcile_id: Option<Uuid>,
    pub exchange_move_id: Option<Uuid>,
    pub amount: Decimal,
    pub debit_amount_currency: Option<Decimal>,
    pub credit_amount_currency: Option<Decimal>,
    pub currency: String,
    pub max_date: NaiveDate,
    pub origin: ReconcileOrigin,
    pub source_type: Option<String>,
    pub source_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub metadata: Option<serde_json::Value>,
}

impl PartialReconcile {
    /// Create a builder for PartialReconcile
    pub fn builder() -> PartialReconcileBuilder {
        <PartialReconcileBuilder as Default>::default()
    }

    /// Create a new PartialReconcile with required fields
    pub fn new(
        company_id: Uuid,
        debit_move_id: Uuid,
        credit_move_id: Uuid,
        amount: Decimal,
        currency: String,
        max_date: NaiveDate,
        origin: ReconcileOrigin,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            company_id,
            debit_move_id,
            credit_move_id,
            full_reconcile_id: None,
            exchange_move_id: None,
            amount,
            debit_amount_currency: None,
            credit_amount_currency: None,
            currency,
            max_date,
            origin,
            source_type: None,
            source_id: None,
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
    pub fn typed_id(&self) -> PartialReconcileId {
        PartialReconcileId(self.id)
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

    /// Set the full_reconcile_id field (chainable)
    pub fn with_full_reconcile_id(mut self, value: Uuid) -> Self {
        self.full_reconcile_id = Some(value);
        self
    }

    /// Set the exchange_move_id field (chainable)
    pub fn with_exchange_move_id(mut self, value: Uuid) -> Self {
        self.exchange_move_id = Some(value);
        self
    }

    /// Set the debit_amount_currency field (chainable)
    pub fn with_debit_amount_currency(mut self, value: Decimal) -> Self {
        self.debit_amount_currency = Some(value);
        self
    }

    /// Set the credit_amount_currency field (chainable)
    pub fn with_credit_amount_currency(mut self, value: Decimal) -> Self {
        self.credit_amount_currency = Some(value);
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
                "debit_move_id" => {
                    if let Ok(v) = serde_json::from_value(value) {
                        self.debit_move_id = v;
                    }
                }
                "credit_move_id" => {
                    if let Ok(v) = serde_json::from_value(value) {
                        self.credit_move_id = v;
                    }
                }
                "full_reconcile_id" => {
                    if let Ok(v) = serde_json::from_value(value) {
                        self.full_reconcile_id = v;
                    }
                }
                "exchange_move_id" => {
                    if let Ok(v) = serde_json::from_value(value) {
                        self.exchange_move_id = v;
                    }
                }
                "amount" => {
                    if let Ok(v) = serde_json::from_value(value) {
                        self.amount = v;
                    }
                }
                "debit_amount_currency" => {
                    if let Ok(v) = serde_json::from_value(value) {
                        self.debit_amount_currency = v;
                    }
                }
                "credit_amount_currency" => {
                    if let Ok(v) = serde_json::from_value(value) {
                        self.credit_amount_currency = v;
                    }
                }
                "currency" => {
                    if let Ok(v) = serde_json::from_value(value) {
                        self.currency = v;
                    }
                }
                "max_date" => {
                    if let Ok(v) = serde_json::from_value(value) {
                        self.max_date = v;
                    }
                }
                "origin" => {
                    if let Ok(v) = serde_json::from_value(value) {
                        self.origin = v;
                    }
                }
                "source_type" => {
                    if let Ok(v) = serde_json::from_value(value) {
                        self.source_type = v;
                    }
                }
                "source_id" => {
                    if let Ok(v) = serde_json::from_value(value) {
                        self.source_id = v;
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

impl super::Entity for PartialReconcile {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "PartialReconcile"
    }
}

impl backbone_core::PersistentEntity for PartialReconcile {
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

impl backbone_orm::EntityRepoMeta for PartialReconcile {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("company_id".to_string(), "uuid".to_string());
        m.insert("debit_move_id".to_string(), "uuid".to_string());
        m.insert("credit_move_id".to_string(), "uuid".to_string());
        m.insert("full_reconcile_id".to_string(), "uuid".to_string());
        m.insert("exchange_move_id".to_string(), "uuid".to_string());
        m.insert("source_id".to_string(), "uuid".to_string());
        m.insert("origin".to_string(), "reconcile_origin".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &["currency"]
    }
    fn company_field() -> Option<&'static str> {
        Some("company_id")
    }
    fn relations() -> &'static [(&'static str, &'static str, &'static str)] {
        &[
            ("debitMove", "journal_lines", "debitMoveId"),
            ("creditMove", "journal_lines", "creditMoveId"),
            ("fullReconcile", "full_reconciles", "fullReconcileId"),
        ]
    }
}

/// Builder for PartialReconcile entity
///
/// Provides a fluent API for constructing PartialReconcile instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct PartialReconcileBuilder {
    company_id: Option<Uuid>,
    debit_move_id: Option<Uuid>,
    credit_move_id: Option<Uuid>,
    full_reconcile_id: Option<Uuid>,
    exchange_move_id: Option<Uuid>,
    amount: Option<Decimal>,
    debit_amount_currency: Option<Decimal>,
    credit_amount_currency: Option<Decimal>,
    currency: Option<String>,
    max_date: Option<NaiveDate>,
    origin: Option<ReconcileOrigin>,
    source_type: Option<String>,
    source_id: Option<Uuid>,
    metadata: Option<serde_json::Value>,
}

impl PartialReconcileBuilder {
    /// Set the company_id field (required)
    pub fn company_id(mut self, value: Uuid) -> Self {
        self.company_id = Some(value);
        self
    }

    /// Set the debit_move_id field (required)
    pub fn debit_move_id(mut self, value: Uuid) -> Self {
        self.debit_move_id = Some(value);
        self
    }

    /// Set the credit_move_id field (required)
    pub fn credit_move_id(mut self, value: Uuid) -> Self {
        self.credit_move_id = Some(value);
        self
    }

    /// Set the full_reconcile_id field (optional)
    pub fn full_reconcile_id(mut self, value: Uuid) -> Self {
        self.full_reconcile_id = Some(value);
        self
    }

    /// Set the exchange_move_id field (optional)
    pub fn exchange_move_id(mut self, value: Uuid) -> Self {
        self.exchange_move_id = Some(value);
        self
    }

    /// Set the amount field (required)
    pub fn amount(mut self, value: Decimal) -> Self {
        self.amount = Some(value);
        self
    }

    /// Set the debit_amount_currency field (optional)
    pub fn debit_amount_currency(mut self, value: Decimal) -> Self {
        self.debit_amount_currency = Some(value);
        self
    }

    /// Set the credit_amount_currency field (optional)
    pub fn credit_amount_currency(mut self, value: Decimal) -> Self {
        self.credit_amount_currency = Some(value);
        self
    }

    /// Set the currency field (default: `"IDR".to_string()`)
    pub fn currency(mut self, value: String) -> Self {
        self.currency = Some(value);
        self
    }

    /// Set the max_date field (required)
    pub fn max_date(mut self, value: NaiveDate) -> Self {
        self.max_date = Some(value);
        self
    }

    /// Set the origin field (default: `ReconcileOrigin::default()`)
    pub fn origin(mut self, value: ReconcileOrigin) -> Self {
        self.origin = Some(value);
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

    /// Set the metadata field (optional)
    pub fn metadata(mut self, value: serde_json::Value) -> Self {
        self.metadata = Some(value);
        self
    }

    /// Build the PartialReconcile entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<PartialReconcile, String> {
        let company_id = self
            .company_id
            .ok_or_else(|| "company_id is required".to_string())?;
        let debit_move_id = self
            .debit_move_id
            .ok_or_else(|| "debit_move_id is required".to_string())?;
        let credit_move_id = self
            .credit_move_id
            .ok_or_else(|| "credit_move_id is required".to_string())?;
        let amount = self
            .amount
            .ok_or_else(|| "amount is required".to_string())?;
        let max_date = self
            .max_date
            .ok_or_else(|| "max_date is required".to_string())?;

        Ok(PartialReconcile {
            id: Uuid::new_v4(),
            company_id,
            debit_move_id,
            credit_move_id,
            full_reconcile_id: self.full_reconcile_id,
            exchange_move_id: self.exchange_move_id,
            amount,
            debit_amount_currency: self.debit_amount_currency,
            credit_amount_currency: self.credit_amount_currency,
            currency: self.currency.unwrap_or("IDR".to_string()),
            max_date,
            origin: self.origin.unwrap_or_default(),
            source_type: self.source_type,
            source_id: self.source_id,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            metadata: self.metadata,
        })
    }
}
