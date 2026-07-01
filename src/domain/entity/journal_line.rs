use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use rust_decimal::Decimal;

use super::PartyType;

/// Strongly-typed ID for JournalLine
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct JournalLineId(pub Uuid);

impl JournalLineId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for JournalLineId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for JournalLineId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for JournalLineId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<JournalLineId> for Uuid {
    fn from(id: JournalLineId) -> Self { id.0 }
}

impl AsRef<Uuid> for JournalLineId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for JournalLineId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct JournalLine {
    pub id: Uuid,
    pub journal_id: Uuid,
    pub company_id: Uuid,
    pub branch_id: Option<Uuid>,
    pub party_type: Option<PartyType>,
    pub party_id: Option<Uuid>,
    pub line_number: i32,
    pub account_id: Uuid,
    pub account_number: String,
    pub account_name: String,
    pub debit_amount: Decimal,
    pub credit_amount: Decimal,
    pub currency: String,
    pub exchange_rate: Decimal,
    pub base_debit_amount: Decimal,
    pub base_credit_amount: Decimal,
    pub description: Option<String>,
    pub cost_center_id: Option<Uuid>,
    pub project_id: Option<Uuid>,
    pub department_id: Option<Uuid>,
    pub dimensions: Option<serde_json::Value>,
    pub source_type: Option<String>,
    pub source_id: Option<Uuid>,
    pub source_reference: Option<String>,
    pub is_tax_line: bool,
    pub tax_rate: Option<Decimal>,
    pub tax_base_amount: Option<Decimal>,
    pub related_line_id: Option<Uuid>,
    pub has_quantity: bool,
    pub quantity: Option<Decimal>,
    pub unit: Option<String>,
    pub unit_price: Option<Decimal>,
    pub is_reconciled: bool,
    pub reconciliation_id: Option<Uuid>,
    pub reconciled_at: Option<DateTime<Utc>>,
    pub is_posted: bool,
    pub ledger_id: Option<Uuid>,
    pub posted_at: Option<DateTime<Utc>>,
    pub tags: serde_json::Value,
    pub data: serde_json::Value,
}

impl JournalLine {
    /// Create a builder for JournalLine
    pub fn builder() -> JournalLineBuilder {
        JournalLineBuilder::default()
    }

    /// Create a new JournalLine with required fields
    pub fn new(journal_id: Uuid, company_id: Uuid, line_number: i32, account_id: Uuid, account_number: String, account_name: String, debit_amount: Decimal, credit_amount: Decimal, currency: String, exchange_rate: Decimal, base_debit_amount: Decimal, base_credit_amount: Decimal, is_tax_line: bool, has_quantity: bool, is_reconciled: bool, is_posted: bool, tags: serde_json::Value, data: serde_json::Value) -> Self {
        Self {
            id: Uuid::new_v4(),
            journal_id,
            company_id,
            branch_id: None,
            party_type: None,
            party_id: None,
            line_number,
            account_id,
            account_number,
            account_name,
            debit_amount,
            credit_amount,
            currency,
            exchange_rate,
            base_debit_amount,
            base_credit_amount,
            description: None,
            cost_center_id: None,
            project_id: None,
            department_id: None,
            dimensions: None,
            source_type: None,
            source_id: None,
            source_reference: None,
            is_tax_line,
            tax_rate: None,
            tax_base_amount: None,
            related_line_id: None,
            has_quantity,
            quantity: None,
            unit: None,
            unit_price: None,
            is_reconciled,
            reconciliation_id: None,
            reconciled_at: None,
            is_posted,
            ledger_id: None,
            posted_at: None,
            tags,
            data,
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> JournalLineId {
        JournalLineId(self.id)
    }


    // ==========================================================
    // Fluent Setters (with_* for optional fields)
    // ==========================================================

    /// Set the branch_id field (chainable)
    pub fn with_branch_id(mut self, value: Uuid) -> Self {
        self.branch_id = Some(value);
        self
    }

    /// Set the party_type field (chainable)
    pub fn with_party_type(mut self, value: PartyType) -> Self {
        self.party_type = Some(value);
        self
    }

    /// Set the party_id field (chainable)
    pub fn with_party_id(mut self, value: Uuid) -> Self {
        self.party_id = Some(value);
        self
    }

    /// Set the description field (chainable)
    pub fn with_description(mut self, value: String) -> Self {
        self.description = Some(value);
        self
    }

    /// Set the cost_center_id field (chainable)
    pub fn with_cost_center_id(mut self, value: Uuid) -> Self {
        self.cost_center_id = Some(value);
        self
    }

    /// Set the project_id field (chainable)
    pub fn with_project_id(mut self, value: Uuid) -> Self {
        self.project_id = Some(value);
        self
    }

    /// Set the department_id field (chainable)
    pub fn with_department_id(mut self, value: Uuid) -> Self {
        self.department_id = Some(value);
        self
    }

    /// Set the dimensions field (chainable)
    pub fn with_dimensions(mut self, value: serde_json::Value) -> Self {
        self.dimensions = Some(value);
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

    /// Set the tax_rate field (chainable)
    pub fn with_tax_rate(mut self, value: Decimal) -> Self {
        self.tax_rate = Some(value);
        self
    }

    /// Set the tax_base_amount field (chainable)
    pub fn with_tax_base_amount(mut self, value: Decimal) -> Self {
        self.tax_base_amount = Some(value);
        self
    }

    /// Set the related_line_id field (chainable)
    pub fn with_related_line_id(mut self, value: Uuid) -> Self {
        self.related_line_id = Some(value);
        self
    }

    /// Set the quantity field (chainable)
    pub fn with_quantity(mut self, value: Decimal) -> Self {
        self.quantity = Some(value);
        self
    }

    /// Set the unit field (chainable)
    pub fn with_unit(mut self, value: String) -> Self {
        self.unit = Some(value);
        self
    }

    /// Set the unit_price field (chainable)
    pub fn with_unit_price(mut self, value: Decimal) -> Self {
        self.unit_price = Some(value);
        self
    }

    /// Set the reconciliation_id field (chainable)
    pub fn with_reconciliation_id(mut self, value: Uuid) -> Self {
        self.reconciliation_id = Some(value);
        self
    }

    /// Set the reconciled_at field (chainable)
    pub fn with_reconciled_at(mut self, value: DateTime<Utc>) -> Self {
        self.reconciled_at = Some(value);
        self
    }

    /// Set the ledger_id field (chainable)
    pub fn with_ledger_id(mut self, value: Uuid) -> Self {
        self.ledger_id = Some(value);
        self
    }

    /// Set the posted_at field (chainable)
    pub fn with_posted_at(mut self, value: DateTime<Utc>) -> Self {
        self.posted_at = Some(value);
        self
    }

    // ==========================================================
    // Partial Update
    // ==========================================================

    /// Apply partial updates from a map of field name to JSON value
    pub fn apply_patch(&mut self, fields: std::collections::HashMap<String, serde_json::Value>) {
        for (key, value) in fields {
            match key.as_str() {
                "journal_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.journal_id = v; }
                }
                "company_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.company_id = v; }
                }
                "branch_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.branch_id = v; }
                }
                "party_type" => {
                    if let Ok(v) = serde_json::from_value(value) { self.party_type = v; }
                }
                "party_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.party_id = v; }
                }
                "line_number" => {
                    if let Ok(v) = serde_json::from_value(value) { self.line_number = v; }
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
                "debit_amount" => {
                    if let Ok(v) = serde_json::from_value(value) { self.debit_amount = v; }
                }
                "credit_amount" => {
                    if let Ok(v) = serde_json::from_value(value) { self.credit_amount = v; }
                }
                "currency" => {
                    if let Ok(v) = serde_json::from_value(value) { self.currency = v; }
                }
                "exchange_rate" => {
                    if let Ok(v) = serde_json::from_value(value) { self.exchange_rate = v; }
                }
                "base_debit_amount" => {
                    if let Ok(v) = serde_json::from_value(value) { self.base_debit_amount = v; }
                }
                "base_credit_amount" => {
                    if let Ok(v) = serde_json::from_value(value) { self.base_credit_amount = v; }
                }
                "description" => {
                    if let Ok(v) = serde_json::from_value(value) { self.description = v; }
                }
                "cost_center_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.cost_center_id = v; }
                }
                "project_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.project_id = v; }
                }
                "department_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.department_id = v; }
                }
                "dimensions" => {
                    if let Ok(v) = serde_json::from_value(value) { self.dimensions = v; }
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
                "is_tax_line" => {
                    if let Ok(v) = serde_json::from_value(value) { self.is_tax_line = v; }
                }
                "tax_rate" => {
                    if let Ok(v) = serde_json::from_value(value) { self.tax_rate = v; }
                }
                "tax_base_amount" => {
                    if let Ok(v) = serde_json::from_value(value) { self.tax_base_amount = v; }
                }
                "related_line_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.related_line_id = v; }
                }
                "has_quantity" => {
                    if let Ok(v) = serde_json::from_value(value) { self.has_quantity = v; }
                }
                "quantity" => {
                    if let Ok(v) = serde_json::from_value(value) { self.quantity = v; }
                }
                "unit" => {
                    if let Ok(v) = serde_json::from_value(value) { self.unit = v; }
                }
                "unit_price" => {
                    if let Ok(v) = serde_json::from_value(value) { self.unit_price = v; }
                }
                "is_reconciled" => {
                    if let Ok(v) = serde_json::from_value(value) { self.is_reconciled = v; }
                }
                "reconciliation_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.reconciliation_id = v; }
                }
                "reconciled_at" => {
                    if let Ok(v) = serde_json::from_value(value) { self.reconciled_at = v; }
                }
                "is_posted" => {
                    if let Ok(v) = serde_json::from_value(value) { self.is_posted = v; }
                }
                "ledger_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.ledger_id = v; }
                }
                "posted_at" => {
                    if let Ok(v) = serde_json::from_value(value) { self.posted_at = v; }
                }
                "tags" => {
                    if let Ok(v) = serde_json::from_value(value) { self.tags = v; }
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

impl super::Entity for JournalLine {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "JournalLine"
    }
}

impl backbone_core::PersistentEntity for JournalLine {
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

impl backbone_orm::EntityRepoMeta for JournalLine {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("journal_id".to_string(), "uuid".to_string());
        m.insert("company_id".to_string(), "uuid".to_string());
        m.insert("branch_id".to_string(), "uuid".to_string());
        m.insert("party_id".to_string(), "uuid".to_string());
        m.insert("account_id".to_string(), "uuid".to_string());
        m.insert("cost_center_id".to_string(), "uuid".to_string());
        m.insert("project_id".to_string(), "uuid".to_string());
        m.insert("department_id".to_string(), "uuid".to_string());
        m.insert("source_id".to_string(), "uuid".to_string());
        m.insert("related_line_id".to_string(), "uuid".to_string());
        m.insert("reconciliation_id".to_string(), "uuid".to_string());
        m.insert("ledger_id".to_string(), "uuid".to_string());
        m.insert("party_type".to_string(), "party_type".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &["account_number", "account_name", "currency"]
    }
    fn relations() -> &'static [(&'static str, &'static str, &'static str)] {
        &[("journal", "journals", "journalId"), ("account", "accounts", "accountId"), ("relatedLine", "journal_lines", "relatedLineId"), ("reconciliation", "reconciliations", "reconciliationId"), ("ledger", "ledgers", "ledgerId"), ("costCenter", "cost_centers", "costCenterId")]
    }
}

/// Builder for JournalLine entity
///
/// Provides a fluent API for constructing JournalLine instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct JournalLineBuilder {
    journal_id: Option<Uuid>,
    company_id: Option<Uuid>,
    branch_id: Option<Uuid>,
    party_type: Option<PartyType>,
    party_id: Option<Uuid>,
    line_number: Option<i32>,
    account_id: Option<Uuid>,
    account_number: Option<String>,
    account_name: Option<String>,
    debit_amount: Option<Decimal>,
    credit_amount: Option<Decimal>,
    currency: Option<String>,
    exchange_rate: Option<Decimal>,
    base_debit_amount: Option<Decimal>,
    base_credit_amount: Option<Decimal>,
    description: Option<String>,
    cost_center_id: Option<Uuid>,
    project_id: Option<Uuid>,
    department_id: Option<Uuid>,
    dimensions: Option<serde_json::Value>,
    source_type: Option<String>,
    source_id: Option<Uuid>,
    source_reference: Option<String>,
    is_tax_line: Option<bool>,
    tax_rate: Option<Decimal>,
    tax_base_amount: Option<Decimal>,
    related_line_id: Option<Uuid>,
    has_quantity: Option<bool>,
    quantity: Option<Decimal>,
    unit: Option<String>,
    unit_price: Option<Decimal>,
    is_reconciled: Option<bool>,
    reconciliation_id: Option<Uuid>,
    reconciled_at: Option<DateTime<Utc>>,
    is_posted: Option<bool>,
    ledger_id: Option<Uuid>,
    posted_at: Option<DateTime<Utc>>,
    tags: Option<serde_json::Value>,
    data: Option<serde_json::Value>,
}

impl JournalLineBuilder {
    /// Set the journal_id field (required)
    pub fn journal_id(mut self, value: Uuid) -> Self {
        self.journal_id = Some(value);
        self
    }

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

    /// Set the party_type field (optional)
    pub fn party_type(mut self, value: PartyType) -> Self {
        self.party_type = Some(value);
        self
    }

    /// Set the party_id field (optional)
    pub fn party_id(mut self, value: Uuid) -> Self {
        self.party_id = Some(value);
        self
    }

    /// Set the line_number field (required)
    pub fn line_number(mut self, value: i32) -> Self {
        self.line_number = Some(value);
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

    /// Set the debit_amount field (default: `Decimal::from(0)`)
    pub fn debit_amount(mut self, value: Decimal) -> Self {
        self.debit_amount = Some(value);
        self
    }

    /// Set the credit_amount field (default: `Decimal::from(0)`)
    pub fn credit_amount(mut self, value: Decimal) -> Self {
        self.credit_amount = Some(value);
        self
    }

    /// Set the currency field (default: `"IDR".to_string()`)
    pub fn currency(mut self, value: String) -> Self {
        self.currency = Some(value);
        self
    }

    /// Set the exchange_rate field (default: `Decimal::from(1)`)
    pub fn exchange_rate(mut self, value: Decimal) -> Self {
        self.exchange_rate = Some(value);
        self
    }

    /// Set the base_debit_amount field (default: `Decimal::from(0)`)
    pub fn base_debit_amount(mut self, value: Decimal) -> Self {
        self.base_debit_amount = Some(value);
        self
    }

    /// Set the base_credit_amount field (default: `Decimal::from(0)`)
    pub fn base_credit_amount(mut self, value: Decimal) -> Self {
        self.base_credit_amount = Some(value);
        self
    }

    /// Set the description field (optional)
    pub fn description(mut self, value: String) -> Self {
        self.description = Some(value);
        self
    }

    /// Set the cost_center_id field (optional)
    pub fn cost_center_id(mut self, value: Uuid) -> Self {
        self.cost_center_id = Some(value);
        self
    }

    /// Set the project_id field (optional)
    pub fn project_id(mut self, value: Uuid) -> Self {
        self.project_id = Some(value);
        self
    }

    /// Set the department_id field (optional)
    pub fn department_id(mut self, value: Uuid) -> Self {
        self.department_id = Some(value);
        self
    }

    /// Set the dimensions field (optional)
    pub fn dimensions(mut self, value: serde_json::Value) -> Self {
        self.dimensions = Some(value);
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

    /// Set the is_tax_line field (default: `false`)
    pub fn is_tax_line(mut self, value: bool) -> Self {
        self.is_tax_line = Some(value);
        self
    }

    /// Set the tax_rate field (optional)
    pub fn tax_rate(mut self, value: Decimal) -> Self {
        self.tax_rate = Some(value);
        self
    }

    /// Set the tax_base_amount field (optional)
    pub fn tax_base_amount(mut self, value: Decimal) -> Self {
        self.tax_base_amount = Some(value);
        self
    }

    /// Set the related_line_id field (optional)
    pub fn related_line_id(mut self, value: Uuid) -> Self {
        self.related_line_id = Some(value);
        self
    }

    /// Set the has_quantity field (default: `false`)
    pub fn has_quantity(mut self, value: bool) -> Self {
        self.has_quantity = Some(value);
        self
    }

    /// Set the quantity field (optional)
    pub fn quantity(mut self, value: Decimal) -> Self {
        self.quantity = Some(value);
        self
    }

    /// Set the unit field (optional)
    pub fn unit(mut self, value: String) -> Self {
        self.unit = Some(value);
        self
    }

    /// Set the unit_price field (optional)
    pub fn unit_price(mut self, value: Decimal) -> Self {
        self.unit_price = Some(value);
        self
    }

    /// Set the is_reconciled field (default: `false`)
    pub fn is_reconciled(mut self, value: bool) -> Self {
        self.is_reconciled = Some(value);
        self
    }

    /// Set the reconciliation_id field (optional)
    pub fn reconciliation_id(mut self, value: Uuid) -> Self {
        self.reconciliation_id = Some(value);
        self
    }

    /// Set the reconciled_at field (optional)
    pub fn reconciled_at(mut self, value: DateTime<Utc>) -> Self {
        self.reconciled_at = Some(value);
        self
    }

    /// Set the is_posted field (default: `false`)
    pub fn is_posted(mut self, value: bool) -> Self {
        self.is_posted = Some(value);
        self
    }

    /// Set the ledger_id field (optional)
    pub fn ledger_id(mut self, value: Uuid) -> Self {
        self.ledger_id = Some(value);
        self
    }

    /// Set the posted_at field (optional)
    pub fn posted_at(mut self, value: DateTime<Utc>) -> Self {
        self.posted_at = Some(value);
        self
    }

    /// Set the tags field (default: `serde_json::json!([])`)
    pub fn tags(mut self, value: serde_json::Value) -> Self {
        self.tags = Some(value);
        self
    }

    /// Set the data field (default: `serde_json::json!({})`)
    pub fn data(mut self, value: serde_json::Value) -> Self {
        self.data = Some(value);
        self
    }

    /// Build the JournalLine entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<JournalLine, String> {
        let journal_id = self.journal_id.ok_or_else(|| "journal_id is required".to_string())?;
        let company_id = self.company_id.ok_or_else(|| "company_id is required".to_string())?;
        let line_number = self.line_number.ok_or_else(|| "line_number is required".to_string())?;
        let account_id = self.account_id.ok_or_else(|| "account_id is required".to_string())?;
        let account_number = self.account_number.ok_or_else(|| "account_number is required".to_string())?;
        let account_name = self.account_name.ok_or_else(|| "account_name is required".to_string())?;

        Ok(JournalLine {
            id: Uuid::new_v4(),
            journal_id,
            company_id,
            branch_id: self.branch_id,
            party_type: self.party_type,
            party_id: self.party_id,
            line_number,
            account_id,
            account_number,
            account_name,
            debit_amount: self.debit_amount.unwrap_or(Decimal::from(0)),
            credit_amount: self.credit_amount.unwrap_or(Decimal::from(0)),
            currency: self.currency.unwrap_or("IDR".to_string()),
            exchange_rate: self.exchange_rate.unwrap_or(Decimal::from(1)),
            base_debit_amount: self.base_debit_amount.unwrap_or(Decimal::from(0)),
            base_credit_amount: self.base_credit_amount.unwrap_or(Decimal::from(0)),
            description: self.description,
            cost_center_id: self.cost_center_id,
            project_id: self.project_id,
            department_id: self.department_id,
            dimensions: self.dimensions,
            source_type: self.source_type,
            source_id: self.source_id,
            source_reference: self.source_reference,
            is_tax_line: self.is_tax_line.unwrap_or(false),
            tax_rate: self.tax_rate,
            tax_base_amount: self.tax_base_amount,
            related_line_id: self.related_line_id,
            has_quantity: self.has_quantity.unwrap_or(false),
            quantity: self.quantity,
            unit: self.unit,
            unit_price: self.unit_price,
            is_reconciled: self.is_reconciled.unwrap_or(false),
            reconciliation_id: self.reconciliation_id,
            reconciled_at: self.reconciled_at,
            is_posted: self.is_posted.unwrap_or(false),
            ledger_id: self.ledger_id,
            posted_at: self.posted_at,
            tags: self.tags.unwrap_or(serde_json::json!([])),
            data: self.data.unwrap_or(serde_json::json!({})),
        })
    }
}
