use chrono::{DateTime, Utc, NaiveDate};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use rust_decimal::Decimal;

use super::AccountType;
use super::NormalBalance;
use super::AuditMetadata;

/// Strongly-typed ID for Ledger
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LedgerId(pub Uuid);

impl LedgerId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for LedgerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for LedgerId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for LedgerId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<LedgerId> for Uuid {
    fn from(id: LedgerId) -> Self { id.0 }
}

impl AsRef<Uuid> for LedgerId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for LedgerId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Ledger {
    pub id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_id: Option<Uuid>,
    pub account_id: Uuid,
    pub account_number: String,
    pub account_name: String,
    pub account_type: AccountType,
    pub normal_balance: NormalBalance,
    pub journal_id: Uuid,
    pub journal_number: String,
    pub journal_line_id: Uuid,
    pub transaction_date: NaiveDate,
    pub posting_date: NaiveDate,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fiscal_period_id: Option<Uuid>,
    pub fiscal_year: i32,
    pub fiscal_month: i32,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
    pub currency: String,
    pub debit_amount: Decimal,
    pub credit_amount: Decimal,
    pub balance_before: Decimal,
    pub balance_after: Decimal,
    pub balance_change: Decimal,
    pub sequence_number: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outlet_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost_center: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub department: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_reference: Option<String>,
    pub is_reconciled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reconciliation_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reconciled_at: Option<DateTime<Utc>>,
    pub is_opening_balance: bool,
    pub is_closing_entry: bool,
    pub is_adjustment: bool,
    pub is_reversed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reversed_by_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reverses_id: Option<Uuid>,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl Ledger {
    /// Create a builder for Ledger
    pub fn builder() -> LedgerBuilder {
        LedgerBuilder::default()
    }

    /// Create a new Ledger with required fields
    pub fn new(account_id: Uuid, account_number: String, account_name: String, account_type: AccountType, normal_balance: NormalBalance, journal_id: Uuid, journal_number: String, journal_line_id: Uuid, transaction_date: NaiveDate, posting_date: NaiveDate, fiscal_year: i32, fiscal_month: i32, description: String, currency: String, debit_amount: Decimal, credit_amount: Decimal, balance_before: Decimal, balance_after: Decimal, balance_change: Decimal, sequence_number: i32, is_reconciled: bool, is_opening_balance: bool, is_closing_entry: bool, is_adjustment: bool, is_reversed: bool) -> Self {
        Self {
            id: Uuid::new_v4(),
            provider_id: None,
            account_id,
            account_number,
            account_name,
            account_type,
            normal_balance,
            journal_id,
            journal_number,
            journal_line_id,
            transaction_date,
            posting_date,
            fiscal_period_id: None,
            fiscal_year,
            fiscal_month,
            description,
            reference: None,
            currency,
            debit_amount,
            credit_amount,
            balance_before,
            balance_after,
            balance_change,
            sequence_number,
            outlet_id: None,
            cost_center: None,
            project: None,
            department: None,
            source_type: None,
            source_id: None,
            source_reference: None,
            is_reconciled,
            reconciliation_id: None,
            reconciled_at: None,
            is_opening_balance,
            is_closing_entry,
            is_adjustment,
            is_reversed,
            reversed_by_id: None,
            reverses_id: None,
            metadata: AuditMetadata::default(),
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> LedgerId {
        LedgerId(self.id)
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

    /// Set the provider_id field (chainable)
    pub fn with_provider_id(mut self, value: Uuid) -> Self {
        self.provider_id = Some(value);
        self
    }

    /// Set the fiscal_period_id field (chainable)
    pub fn with_fiscal_period_id(mut self, value: Uuid) -> Self {
        self.fiscal_period_id = Some(value);
        self
    }

    /// Set the reference field (chainable)
    pub fn with_reference(mut self, value: String) -> Self {
        self.reference = Some(value);
        self
    }

    /// Set the outlet_id field (chainable)
    pub fn with_outlet_id(mut self, value: Uuid) -> Self {
        self.outlet_id = Some(value);
        self
    }

    /// Set the cost_center field (chainable)
    pub fn with_cost_center(mut self, value: String) -> Self {
        self.cost_center = Some(value);
        self
    }

    /// Set the project field (chainable)
    pub fn with_project(mut self, value: String) -> Self {
        self.project = Some(value);
        self
    }

    /// Set the department field (chainable)
    pub fn with_department(mut self, value: String) -> Self {
        self.department = Some(value);
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

    /// Set the reversed_by_id field (chainable)
    pub fn with_reversed_by_id(mut self, value: Uuid) -> Self {
        self.reversed_by_id = Some(value);
        self
    }

    /// Set the reverses_id field (chainable)
    pub fn with_reverses_id(mut self, value: Uuid) -> Self {
        self.reverses_id = Some(value);
        self
    }

    // ==========================================================
    // Partial Update
    // ==========================================================

    /// Apply partial updates from a map of field name to JSON value
    pub fn apply_patch(&mut self, fields: std::collections::HashMap<String, serde_json::Value>) {
        for (key, value) in fields {
            match key.as_str() {
                "provider_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.provider_id = v; }
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
                "account_type" => {
                    if let Ok(v) = serde_json::from_value(value) { self.account_type = v; }
                }
                "normal_balance" => {
                    if let Ok(v) = serde_json::from_value(value) { self.normal_balance = v; }
                }
                "journal_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.journal_id = v; }
                }
                "journal_number" => {
                    if let Ok(v) = serde_json::from_value(value) { self.journal_number = v; }
                }
                "journal_line_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.journal_line_id = v; }
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
                "reference" => {
                    if let Ok(v) = serde_json::from_value(value) { self.reference = v; }
                }
                "currency" => {
                    if let Ok(v) = serde_json::from_value(value) { self.currency = v; }
                }
                "debit_amount" => {
                    if let Ok(v) = serde_json::from_value(value) { self.debit_amount = v; }
                }
                "credit_amount" => {
                    if let Ok(v) = serde_json::from_value(value) { self.credit_amount = v; }
                }
                "balance_before" => {
                    if let Ok(v) = serde_json::from_value(value) { self.balance_before = v; }
                }
                "balance_after" => {
                    if let Ok(v) = serde_json::from_value(value) { self.balance_after = v; }
                }
                "balance_change" => {
                    if let Ok(v) = serde_json::from_value(value) { self.balance_change = v; }
                }
                "sequence_number" => {
                    if let Ok(v) = serde_json::from_value(value) { self.sequence_number = v; }
                }
                "outlet_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.outlet_id = v; }
                }
                "cost_center" => {
                    if let Ok(v) = serde_json::from_value(value) { self.cost_center = v; }
                }
                "project" => {
                    if let Ok(v) = serde_json::from_value(value) { self.project = v; }
                }
                "department" => {
                    if let Ok(v) = serde_json::from_value(value) { self.department = v; }
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
                "is_reconciled" => {
                    if let Ok(v) = serde_json::from_value(value) { self.is_reconciled = v; }
                }
                "reconciliation_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.reconciliation_id = v; }
                }
                "reconciled_at" => {
                    if let Ok(v) = serde_json::from_value(value) { self.reconciled_at = v; }
                }
                "is_opening_balance" => {
                    if let Ok(v) = serde_json::from_value(value) { self.is_opening_balance = v; }
                }
                "is_closing_entry" => {
                    if let Ok(v) = serde_json::from_value(value) { self.is_closing_entry = v; }
                }
                "is_adjustment" => {
                    if let Ok(v) = serde_json::from_value(value) { self.is_adjustment = v; }
                }
                "is_reversed" => {
                    if let Ok(v) = serde_json::from_value(value) { self.is_reversed = v; }
                }
                "reversed_by_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.reversed_by_id = v; }
                }
                "reverses_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.reverses_id = v; }
                }
                _ => {} // ignore unknown fields
            }
        }
    }

    // <<< CUSTOM METHODS START >>>
    // <<< CUSTOM METHODS END >>>
}

impl super::Entity for Ledger {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "Ledger"
    }
}

impl backbone_core::PersistentEntity for Ledger {
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

impl backbone_orm::EntityRepoMeta for Ledger {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("provider_id".to_string(), "uuid".to_string());
        m.insert("account_id".to_string(), "uuid".to_string());
        m.insert("journal_id".to_string(), "uuid".to_string());
        m.insert("journal_line_id".to_string(), "uuid".to_string());
        m.insert("fiscal_period_id".to_string(), "uuid".to_string());
        m.insert("outlet_id".to_string(), "uuid".to_string());
        m.insert("source_id".to_string(), "uuid".to_string());
        m.insert("reconciliation_id".to_string(), "uuid".to_string());
        m.insert("reversed_by_id".to_string(), "uuid".to_string());
        m.insert("reverses_id".to_string(), "uuid".to_string());
        m.insert("account_type".to_string(), "account_type".to_string());
        m.insert("normal_balance".to_string(), "normal_balance".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &["account_number", "account_name", "journal_number", "description", "currency"]
    }
}

/// Builder for Ledger entity
///
/// Provides a fluent API for constructing Ledger instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct LedgerBuilder {
    provider_id: Option<Uuid>,
    account_id: Option<Uuid>,
    account_number: Option<String>,
    account_name: Option<String>,
    account_type: Option<AccountType>,
    normal_balance: Option<NormalBalance>,
    journal_id: Option<Uuid>,
    journal_number: Option<String>,
    journal_line_id: Option<Uuid>,
    transaction_date: Option<NaiveDate>,
    posting_date: Option<NaiveDate>,
    fiscal_period_id: Option<Uuid>,
    fiscal_year: Option<i32>,
    fiscal_month: Option<i32>,
    description: Option<String>,
    reference: Option<String>,
    currency: Option<String>,
    debit_amount: Option<Decimal>,
    credit_amount: Option<Decimal>,
    balance_before: Option<Decimal>,
    balance_after: Option<Decimal>,
    balance_change: Option<Decimal>,
    sequence_number: Option<i32>,
    outlet_id: Option<Uuid>,
    cost_center: Option<String>,
    project: Option<String>,
    department: Option<String>,
    source_type: Option<String>,
    source_id: Option<Uuid>,
    source_reference: Option<String>,
    is_reconciled: Option<bool>,
    reconciliation_id: Option<Uuid>,
    reconciled_at: Option<DateTime<Utc>>,
    is_opening_balance: Option<bool>,
    is_closing_entry: Option<bool>,
    is_adjustment: Option<bool>,
    is_reversed: Option<bool>,
    reversed_by_id: Option<Uuid>,
    reverses_id: Option<Uuid>,
}

impl LedgerBuilder {
    /// Set the provider_id field (optional)
    pub fn provider_id(mut self, value: Uuid) -> Self {
        self.provider_id = Some(value);
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

    /// Set the account_type field (required)
    pub fn account_type(mut self, value: AccountType) -> Self {
        self.account_type = Some(value);
        self
    }

    /// Set the normal_balance field (required)
    pub fn normal_balance(mut self, value: NormalBalance) -> Self {
        self.normal_balance = Some(value);
        self
    }

    /// Set the journal_id field (required)
    pub fn journal_id(mut self, value: Uuid) -> Self {
        self.journal_id = Some(value);
        self
    }

    /// Set the journal_number field (required)
    pub fn journal_number(mut self, value: String) -> Self {
        self.journal_number = Some(value);
        self
    }

    /// Set the journal_line_id field (required)
    pub fn journal_line_id(mut self, value: Uuid) -> Self {
        self.journal_line_id = Some(value);
        self
    }

    /// Set the transaction_date field (required)
    pub fn transaction_date(mut self, value: NaiveDate) -> Self {
        self.transaction_date = Some(value);
        self
    }

    /// Set the posting_date field (required)
    pub fn posting_date(mut self, value: NaiveDate) -> Self {
        self.posting_date = Some(value);
        self
    }

    /// Set the fiscal_period_id field (optional)
    pub fn fiscal_period_id(mut self, value: Uuid) -> Self {
        self.fiscal_period_id = Some(value);
        self
    }

    /// Set the fiscal_year field (required)
    pub fn fiscal_year(mut self, value: i32) -> Self {
        self.fiscal_year = Some(value);
        self
    }

    /// Set the fiscal_month field (required)
    pub fn fiscal_month(mut self, value: i32) -> Self {
        self.fiscal_month = Some(value);
        self
    }

    /// Set the description field (required)
    pub fn description(mut self, value: String) -> Self {
        self.description = Some(value);
        self
    }

    /// Set the reference field (optional)
    pub fn reference(mut self, value: String) -> Self {
        self.reference = Some(value);
        self
    }

    /// Set the currency field (default: `"IDR".to_string()`)
    pub fn currency(mut self, value: String) -> Self {
        self.currency = Some(value);
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

    /// Set the balance_before field (required)
    pub fn balance_before(mut self, value: Decimal) -> Self {
        self.balance_before = Some(value);
        self
    }

    /// Set the balance_after field (required)
    pub fn balance_after(mut self, value: Decimal) -> Self {
        self.balance_after = Some(value);
        self
    }

    /// Set the balance_change field (required)
    pub fn balance_change(mut self, value: Decimal) -> Self {
        self.balance_change = Some(value);
        self
    }

    /// Set the sequence_number field (required)
    pub fn sequence_number(mut self, value: i32) -> Self {
        self.sequence_number = Some(value);
        self
    }

    /// Set the outlet_id field (optional)
    pub fn outlet_id(mut self, value: Uuid) -> Self {
        self.outlet_id = Some(value);
        self
    }

    /// Set the cost_center field (optional)
    pub fn cost_center(mut self, value: String) -> Self {
        self.cost_center = Some(value);
        self
    }

    /// Set the project field (optional)
    pub fn project(mut self, value: String) -> Self {
        self.project = Some(value);
        self
    }

    /// Set the department field (optional)
    pub fn department(mut self, value: String) -> Self {
        self.department = Some(value);
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

    /// Set the is_opening_balance field (default: `false`)
    pub fn is_opening_balance(mut self, value: bool) -> Self {
        self.is_opening_balance = Some(value);
        self
    }

    /// Set the is_closing_entry field (default: `false`)
    pub fn is_closing_entry(mut self, value: bool) -> Self {
        self.is_closing_entry = Some(value);
        self
    }

    /// Set the is_adjustment field (default: `false`)
    pub fn is_adjustment(mut self, value: bool) -> Self {
        self.is_adjustment = Some(value);
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

    /// Set the reverses_id field (optional)
    pub fn reverses_id(mut self, value: Uuid) -> Self {
        self.reverses_id = Some(value);
        self
    }

    /// Build the Ledger entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<Ledger, String> {
        let account_id = self.account_id.ok_or_else(|| "account_id is required".to_string())?;
        let account_number = self.account_number.ok_or_else(|| "account_number is required".to_string())?;
        let account_name = self.account_name.ok_or_else(|| "account_name is required".to_string())?;
        let account_type = self.account_type.ok_or_else(|| "account_type is required".to_string())?;
        let normal_balance = self.normal_balance.ok_or_else(|| "normal_balance is required".to_string())?;
        let journal_id = self.journal_id.ok_or_else(|| "journal_id is required".to_string())?;
        let journal_number = self.journal_number.ok_or_else(|| "journal_number is required".to_string())?;
        let journal_line_id = self.journal_line_id.ok_or_else(|| "journal_line_id is required".to_string())?;
        let transaction_date = self.transaction_date.ok_or_else(|| "transaction_date is required".to_string())?;
        let posting_date = self.posting_date.ok_or_else(|| "posting_date is required".to_string())?;
        let fiscal_year = self.fiscal_year.ok_or_else(|| "fiscal_year is required".to_string())?;
        let fiscal_month = self.fiscal_month.ok_or_else(|| "fiscal_month is required".to_string())?;
        let description = self.description.ok_or_else(|| "description is required".to_string())?;
        let balance_before = self.balance_before.ok_or_else(|| "balance_before is required".to_string())?;
        let balance_after = self.balance_after.ok_or_else(|| "balance_after is required".to_string())?;
        let balance_change = self.balance_change.ok_or_else(|| "balance_change is required".to_string())?;
        let sequence_number = self.sequence_number.ok_or_else(|| "sequence_number is required".to_string())?;

        Ok(Ledger {
            id: Uuid::new_v4(),
            provider_id: self.provider_id,
            account_id,
            account_number,
            account_name,
            account_type,
            normal_balance,
            journal_id,
            journal_number,
            journal_line_id,
            transaction_date,
            posting_date,
            fiscal_period_id: self.fiscal_period_id,
            fiscal_year,
            fiscal_month,
            description,
            reference: self.reference,
            currency: self.currency.unwrap_or("IDR".to_string()),
            debit_amount: self.debit_amount.unwrap_or(Decimal::from(0)),
            credit_amount: self.credit_amount.unwrap_or(Decimal::from(0)),
            balance_before,
            balance_after,
            balance_change,
            sequence_number,
            outlet_id: self.outlet_id,
            cost_center: self.cost_center,
            project: self.project,
            department: self.department,
            source_type: self.source_type,
            source_id: self.source_id,
            source_reference: self.source_reference,
            is_reconciled: self.is_reconciled.unwrap_or(false),
            reconciliation_id: self.reconciliation_id,
            reconciled_at: self.reconciled_at,
            is_opening_balance: self.is_opening_balance.unwrap_or(false),
            is_closing_entry: self.is_closing_entry.unwrap_or(false),
            is_adjustment: self.is_adjustment.unwrap_or(false),
            is_reversed: self.is_reversed.unwrap_or(false),
            reversed_by_id: self.reversed_by_id,
            reverses_id: self.reverses_id,
            metadata: AuditMetadata::default(),
        })
    }
}
