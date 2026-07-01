use chrono::{DateTime, Utc, NaiveDate};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use rust_decimal::Decimal;

use super::AccountType;
use super::AccountSubtype;
use super::NormalBalance;
use super::AccountStatus;
use super::AuditMetadata;

/// Strongly-typed ID for Account
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AccountId(pub Uuid);

impl AccountId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for AccountId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for AccountId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for AccountId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<AccountId> for Uuid {
    fn from(id: AccountId) -> Self { id.0 }
}

impl AsRef<Uuid> for AccountId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for AccountId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Account {
    pub id: Uuid,
    pub company_id: Uuid,
    pub account_number: String,
    pub account_code: String,
    pub name: String,
    pub name_en: Option<String>,
    pub description: Option<String>,
    pub account_type: AccountType,
    pub account_subtype: AccountSubtype,
    pub normal_balance: NormalBalance,
    pub parent_id: Option<Uuid>,
    pub level: i32,
    pub path: Option<String>,
    pub is_header: bool,
    pub is_detail: bool,
    pub currency: String,
    pub opening_balance: Decimal,
    pub opening_balance_date: Option<NaiveDate>,
    pub current_balance: Decimal,
    pub bank_name: Option<String>,
    pub bank_account_number: Option<String>,
    pub bank_account_name: Option<String>,
    pub bank_branch: Option<String>,
    pub is_taxable: bool,
    pub tax_rate: Option<Decimal>,
    pub tax_account_id: Option<Uuid>,
    pub is_reconcilable: bool,
    pub last_reconciled_at: Option<DateTime<Utc>>,
    pub last_reconciled_balance: Option<Decimal>,
    pub has_budget: bool,
    pub budget_amount: Option<Decimal>,
    pub allow_manual_entry: bool,
    pub require_cost_center: bool,
    pub require_project: bool,
    pub sort_order: i32,
    pub show_in_reports: bool,
    pub status: AccountStatus,
    pub is_system: bool,
    pub notes: Option<String>,
    pub source_id: Option<Uuid>,
    pub is_cloned: bool,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl Account {
    /// Create a builder for Account
    pub fn builder() -> AccountBuilder {
        AccountBuilder::default()
    }

    /// Create a new Account with required fields
    pub fn new(company_id: Uuid, account_number: String, account_code: String, name: String, account_type: AccountType, account_subtype: AccountSubtype, normal_balance: NormalBalance, level: i32, is_header: bool, is_detail: bool, currency: String, opening_balance: Decimal, current_balance: Decimal, is_taxable: bool, is_reconcilable: bool, has_budget: bool, allow_manual_entry: bool, require_cost_center: bool, require_project: bool, sort_order: i32, show_in_reports: bool, status: AccountStatus, is_system: bool, is_cloned: bool) -> Self {
        Self {
            id: Uuid::new_v4(),
            company_id,
            account_number,
            account_code,
            name,
            name_en: None,
            description: None,
            account_type,
            account_subtype,
            normal_balance,
            parent_id: None,
            level,
            path: None,
            is_header,
            is_detail,
            currency,
            opening_balance,
            opening_balance_date: None,
            current_balance,
            bank_name: None,
            bank_account_number: None,
            bank_account_name: None,
            bank_branch: None,
            is_taxable,
            tax_rate: None,
            tax_account_id: None,
            is_reconcilable,
            last_reconciled_at: None,
            last_reconciled_balance: None,
            has_budget,
            budget_amount: None,
            allow_manual_entry,
            require_cost_center,
            require_project,
            sort_order,
            show_in_reports,
            status,
            is_system,
            notes: None,
            source_id: None,
            is_cloned,
            metadata: AuditMetadata::default(),
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> AccountId {
        AccountId(self.id)
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
    pub fn status(&self) -> &AccountStatus {
        &self.status
    }


    // ==========================================================
    // Fluent Setters (with_* for optional fields)
    // ==========================================================

    /// Set the name_en field (chainable)
    pub fn with_name_en(mut self, value: String) -> Self {
        self.name_en = Some(value);
        self
    }

    /// Set the description field (chainable)
    pub fn with_description(mut self, value: String) -> Self {
        self.description = Some(value);
        self
    }

    /// Set the parent_id field (chainable)
    pub fn with_parent_id(mut self, value: Uuid) -> Self {
        self.parent_id = Some(value);
        self
    }

    /// Set the path field (chainable)
    pub fn with_path(mut self, value: String) -> Self {
        self.path = Some(value);
        self
    }

    /// Set the opening_balance_date field (chainable)
    pub fn with_opening_balance_date(mut self, value: NaiveDate) -> Self {
        self.opening_balance_date = Some(value);
        self
    }

    /// Set the bank_name field (chainable)
    pub fn with_bank_name(mut self, value: String) -> Self {
        self.bank_name = Some(value);
        self
    }

    /// Set the bank_account_number field (chainable)
    pub fn with_bank_account_number(mut self, value: String) -> Self {
        self.bank_account_number = Some(value);
        self
    }

    /// Set the bank_account_name field (chainable)
    pub fn with_bank_account_name(mut self, value: String) -> Self {
        self.bank_account_name = Some(value);
        self
    }

    /// Set the bank_branch field (chainable)
    pub fn with_bank_branch(mut self, value: String) -> Self {
        self.bank_branch = Some(value);
        self
    }

    /// Set the tax_rate field (chainable)
    pub fn with_tax_rate(mut self, value: Decimal) -> Self {
        self.tax_rate = Some(value);
        self
    }

    /// Set the tax_account_id field (chainable)
    pub fn with_tax_account_id(mut self, value: Uuid) -> Self {
        self.tax_account_id = Some(value);
        self
    }

    /// Set the last_reconciled_at field (chainable)
    pub fn with_last_reconciled_at(mut self, value: DateTime<Utc>) -> Self {
        self.last_reconciled_at = Some(value);
        self
    }

    /// Set the last_reconciled_balance field (chainable)
    pub fn with_last_reconciled_balance(mut self, value: Decimal) -> Self {
        self.last_reconciled_balance = Some(value);
        self
    }

    /// Set the budget_amount field (chainable)
    pub fn with_budget_amount(mut self, value: Decimal) -> Self {
        self.budget_amount = Some(value);
        self
    }

    /// Set the notes field (chainable)
    pub fn with_notes(mut self, value: String) -> Self {
        self.notes = Some(value);
        self
    }

    /// Set the source_id field (chainable)
    pub fn with_source_id(mut self, value: Uuid) -> Self {
        self.source_id = Some(value);
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
                "account_number" => {
                    if let Ok(v) = serde_json::from_value(value) { self.account_number = v; }
                }
                "account_code" => {
                    if let Ok(v) = serde_json::from_value(value) { self.account_code = v; }
                }
                "name" => {
                    if let Ok(v) = serde_json::from_value(value) { self.name = v; }
                }
                "name_en" => {
                    if let Ok(v) = serde_json::from_value(value) { self.name_en = v; }
                }
                "description" => {
                    if let Ok(v) = serde_json::from_value(value) { self.description = v; }
                }
                "account_type" => {
                    if let Ok(v) = serde_json::from_value(value) { self.account_type = v; }
                }
                "account_subtype" => {
                    if let Ok(v) = serde_json::from_value(value) { self.account_subtype = v; }
                }
                "normal_balance" => {
                    if let Ok(v) = serde_json::from_value(value) { self.normal_balance = v; }
                }
                "parent_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.parent_id = v; }
                }
                "level" => {
                    if let Ok(v) = serde_json::from_value(value) { self.level = v; }
                }
                "path" => {
                    if let Ok(v) = serde_json::from_value(value) { self.path = v; }
                }
                "is_header" => {
                    if let Ok(v) = serde_json::from_value(value) { self.is_header = v; }
                }
                "is_detail" => {
                    if let Ok(v) = serde_json::from_value(value) { self.is_detail = v; }
                }
                "currency" => {
                    if let Ok(v) = serde_json::from_value(value) { self.currency = v; }
                }
                "opening_balance" => {
                    if let Ok(v) = serde_json::from_value(value) { self.opening_balance = v; }
                }
                "opening_balance_date" => {
                    if let Ok(v) = serde_json::from_value(value) { self.opening_balance_date = v; }
                }
                "current_balance" => {
                    if let Ok(v) = serde_json::from_value(value) { self.current_balance = v; }
                }
                "bank_name" => {
                    if let Ok(v) = serde_json::from_value(value) { self.bank_name = v; }
                }
                "bank_account_number" => {
                    if let Ok(v) = serde_json::from_value(value) { self.bank_account_number = v; }
                }
                "bank_account_name" => {
                    if let Ok(v) = serde_json::from_value(value) { self.bank_account_name = v; }
                }
                "bank_branch" => {
                    if let Ok(v) = serde_json::from_value(value) { self.bank_branch = v; }
                }
                "is_taxable" => {
                    if let Ok(v) = serde_json::from_value(value) { self.is_taxable = v; }
                }
                "tax_rate" => {
                    if let Ok(v) = serde_json::from_value(value) { self.tax_rate = v; }
                }
                "tax_account_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.tax_account_id = v; }
                }
                "is_reconcilable" => {
                    if let Ok(v) = serde_json::from_value(value) { self.is_reconcilable = v; }
                }
                "last_reconciled_at" => {
                    if let Ok(v) = serde_json::from_value(value) { self.last_reconciled_at = v; }
                }
                "last_reconciled_balance" => {
                    if let Ok(v) = serde_json::from_value(value) { self.last_reconciled_balance = v; }
                }
                "has_budget" => {
                    if let Ok(v) = serde_json::from_value(value) { self.has_budget = v; }
                }
                "budget_amount" => {
                    if let Ok(v) = serde_json::from_value(value) { self.budget_amount = v; }
                }
                "allow_manual_entry" => {
                    if let Ok(v) = serde_json::from_value(value) { self.allow_manual_entry = v; }
                }
                "require_cost_center" => {
                    if let Ok(v) = serde_json::from_value(value) { self.require_cost_center = v; }
                }
                "require_project" => {
                    if let Ok(v) = serde_json::from_value(value) { self.require_project = v; }
                }
                "sort_order" => {
                    if let Ok(v) = serde_json::from_value(value) { self.sort_order = v; }
                }
                "show_in_reports" => {
                    if let Ok(v) = serde_json::from_value(value) { self.show_in_reports = v; }
                }
                "status" => {
                    if let Ok(v) = serde_json::from_value(value) { self.status = v; }
                }
                "is_system" => {
                    if let Ok(v) = serde_json::from_value(value) { self.is_system = v; }
                }
                "notes" => {
                    if let Ok(v) = serde_json::from_value(value) { self.notes = v; }
                }
                "source_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.source_id = v; }
                }
                "is_cloned" => {
                    if let Ok(v) = serde_json::from_value(value) { self.is_cloned = v; }
                }
                _ => {} // ignore unknown fields
            }
        }
    }

    // <<< CUSTOM METHODS START >>>
    // <<< CUSTOM METHODS END >>>
}

impl super::Entity for Account {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "Account"
    }
}

impl backbone_core::PersistentEntity for Account {
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

impl backbone_orm::EntityRepoMeta for Account {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("company_id".to_string(), "uuid".to_string());
        m.insert("parent_id".to_string(), "uuid".to_string());
        m.insert("tax_account_id".to_string(), "uuid".to_string());
        m.insert("source_id".to_string(), "uuid".to_string());
        m.insert("account_type".to_string(), "account_type".to_string());
        m.insert("account_subtype".to_string(), "account_subtype".to_string());
        m.insert("normal_balance".to_string(), "normal_balance".to_string());
        m.insert("status".to_string(), "account_status".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &["account_number", "account_code", "name", "currency"]
    }
    fn relations() -> &'static [(&'static str, &'static str, &'static str)] {
        &[("parent", "accounts", "parentId"), ("source", "accounts", "sourceId"), ("taxAccount", "accounts", "taxAccountId")]
    }
}

/// Builder for Account entity
///
/// Provides a fluent API for constructing Account instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct AccountBuilder {
    company_id: Option<Uuid>,
    account_number: Option<String>,
    account_code: Option<String>,
    name: Option<String>,
    name_en: Option<String>,
    description: Option<String>,
    account_type: Option<AccountType>,
    account_subtype: Option<AccountSubtype>,
    normal_balance: Option<NormalBalance>,
    parent_id: Option<Uuid>,
    level: Option<i32>,
    path: Option<String>,
    is_header: Option<bool>,
    is_detail: Option<bool>,
    currency: Option<String>,
    opening_balance: Option<Decimal>,
    opening_balance_date: Option<NaiveDate>,
    current_balance: Option<Decimal>,
    bank_name: Option<String>,
    bank_account_number: Option<String>,
    bank_account_name: Option<String>,
    bank_branch: Option<String>,
    is_taxable: Option<bool>,
    tax_rate: Option<Decimal>,
    tax_account_id: Option<Uuid>,
    is_reconcilable: Option<bool>,
    last_reconciled_at: Option<DateTime<Utc>>,
    last_reconciled_balance: Option<Decimal>,
    has_budget: Option<bool>,
    budget_amount: Option<Decimal>,
    allow_manual_entry: Option<bool>,
    require_cost_center: Option<bool>,
    require_project: Option<bool>,
    sort_order: Option<i32>,
    show_in_reports: Option<bool>,
    status: Option<AccountStatus>,
    is_system: Option<bool>,
    notes: Option<String>,
    source_id: Option<Uuid>,
    is_cloned: Option<bool>,
}

impl AccountBuilder {
    /// Set the company_id field (required)
    pub fn company_id(mut self, value: Uuid) -> Self {
        self.company_id = Some(value);
        self
    }

    /// Set the account_number field (required)
    pub fn account_number(mut self, value: String) -> Self {
        self.account_number = Some(value);
        self
    }

    /// Set the account_code field (required)
    pub fn account_code(mut self, value: String) -> Self {
        self.account_code = Some(value);
        self
    }

    /// Set the name field (required)
    pub fn name(mut self, value: String) -> Self {
        self.name = Some(value);
        self
    }

    /// Set the name_en field (optional)
    pub fn name_en(mut self, value: String) -> Self {
        self.name_en = Some(value);
        self
    }

    /// Set the description field (optional)
    pub fn description(mut self, value: String) -> Self {
        self.description = Some(value);
        self
    }

    /// Set the account_type field (required)
    pub fn account_type(mut self, value: AccountType) -> Self {
        self.account_type = Some(value);
        self
    }

    /// Set the account_subtype field (required)
    pub fn account_subtype(mut self, value: AccountSubtype) -> Self {
        self.account_subtype = Some(value);
        self
    }

    /// Set the normal_balance field (required)
    pub fn normal_balance(mut self, value: NormalBalance) -> Self {
        self.normal_balance = Some(value);
        self
    }

    /// Set the parent_id field (optional)
    pub fn parent_id(mut self, value: Uuid) -> Self {
        self.parent_id = Some(value);
        self
    }

    /// Set the level field (default: `0`)
    pub fn level(mut self, value: i32) -> Self {
        self.level = Some(value);
        self
    }

    /// Set the path field (optional)
    pub fn path(mut self, value: String) -> Self {
        self.path = Some(value);
        self
    }

    /// Set the is_header field (default: `false`)
    pub fn is_header(mut self, value: bool) -> Self {
        self.is_header = Some(value);
        self
    }

    /// Set the is_detail field (default: `true`)
    pub fn is_detail(mut self, value: bool) -> Self {
        self.is_detail = Some(value);
        self
    }

    /// Set the currency field (default: `"IDR".to_string()`)
    pub fn currency(mut self, value: String) -> Self {
        self.currency = Some(value);
        self
    }

    /// Set the opening_balance field (default: `Decimal::from(0)`)
    pub fn opening_balance(mut self, value: Decimal) -> Self {
        self.opening_balance = Some(value);
        self
    }

    /// Set the opening_balance_date field (optional)
    pub fn opening_balance_date(mut self, value: NaiveDate) -> Self {
        self.opening_balance_date = Some(value);
        self
    }

    /// Set the current_balance field (default: `Decimal::from(0)`)
    pub fn current_balance(mut self, value: Decimal) -> Self {
        self.current_balance = Some(value);
        self
    }

    /// Set the bank_name field (optional)
    pub fn bank_name(mut self, value: String) -> Self {
        self.bank_name = Some(value);
        self
    }

    /// Set the bank_account_number field (optional)
    pub fn bank_account_number(mut self, value: String) -> Self {
        self.bank_account_number = Some(value);
        self
    }

    /// Set the bank_account_name field (optional)
    pub fn bank_account_name(mut self, value: String) -> Self {
        self.bank_account_name = Some(value);
        self
    }

    /// Set the bank_branch field (optional)
    pub fn bank_branch(mut self, value: String) -> Self {
        self.bank_branch = Some(value);
        self
    }

    /// Set the is_taxable field (default: `false`)
    pub fn is_taxable(mut self, value: bool) -> Self {
        self.is_taxable = Some(value);
        self
    }

    /// Set the tax_rate field (optional)
    pub fn tax_rate(mut self, value: Decimal) -> Self {
        self.tax_rate = Some(value);
        self
    }

    /// Set the tax_account_id field (optional)
    pub fn tax_account_id(mut self, value: Uuid) -> Self {
        self.tax_account_id = Some(value);
        self
    }

    /// Set the is_reconcilable field (default: `false`)
    pub fn is_reconcilable(mut self, value: bool) -> Self {
        self.is_reconcilable = Some(value);
        self
    }

    /// Set the last_reconciled_at field (optional)
    pub fn last_reconciled_at(mut self, value: DateTime<Utc>) -> Self {
        self.last_reconciled_at = Some(value);
        self
    }

    /// Set the last_reconciled_balance field (optional)
    pub fn last_reconciled_balance(mut self, value: Decimal) -> Self {
        self.last_reconciled_balance = Some(value);
        self
    }

    /// Set the has_budget field (default: `false`)
    pub fn has_budget(mut self, value: bool) -> Self {
        self.has_budget = Some(value);
        self
    }

    /// Set the budget_amount field (optional)
    pub fn budget_amount(mut self, value: Decimal) -> Self {
        self.budget_amount = Some(value);
        self
    }

    /// Set the allow_manual_entry field (default: `true`)
    pub fn allow_manual_entry(mut self, value: bool) -> Self {
        self.allow_manual_entry = Some(value);
        self
    }

    /// Set the require_cost_center field (default: `false`)
    pub fn require_cost_center(mut self, value: bool) -> Self {
        self.require_cost_center = Some(value);
        self
    }

    /// Set the require_project field (default: `false`)
    pub fn require_project(mut self, value: bool) -> Self {
        self.require_project = Some(value);
        self
    }

    /// Set the sort_order field (default: `0`)
    pub fn sort_order(mut self, value: i32) -> Self {
        self.sort_order = Some(value);
        self
    }

    /// Set the show_in_reports field (default: `true`)
    pub fn show_in_reports(mut self, value: bool) -> Self {
        self.show_in_reports = Some(value);
        self
    }

    /// Set the status field (default: `AccountStatus::default()`)
    pub fn status(mut self, value: AccountStatus) -> Self {
        self.status = Some(value);
        self
    }

    /// Set the is_system field (default: `false`)
    pub fn is_system(mut self, value: bool) -> Self {
        self.is_system = Some(value);
        self
    }

    /// Set the notes field (optional)
    pub fn notes(mut self, value: String) -> Self {
        self.notes = Some(value);
        self
    }

    /// Set the source_id field (optional)
    pub fn source_id(mut self, value: Uuid) -> Self {
        self.source_id = Some(value);
        self
    }

    /// Set the is_cloned field (default: `false`)
    pub fn is_cloned(mut self, value: bool) -> Self {
        self.is_cloned = Some(value);
        self
    }

    /// Build the Account entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<Account, String> {
        let company_id = self.company_id.ok_or_else(|| "company_id is required".to_string())?;
        let account_number = self.account_number.ok_or_else(|| "account_number is required".to_string())?;
        let account_code = self.account_code.ok_or_else(|| "account_code is required".to_string())?;
        let name = self.name.ok_or_else(|| "name is required".to_string())?;
        let account_type = self.account_type.ok_or_else(|| "account_type is required".to_string())?;
        let account_subtype = self.account_subtype.ok_or_else(|| "account_subtype is required".to_string())?;
        let normal_balance = self.normal_balance.ok_or_else(|| "normal_balance is required".to_string())?;

        Ok(Account {
            id: Uuid::new_v4(),
            company_id,
            account_number,
            account_code,
            name,
            name_en: self.name_en,
            description: self.description,
            account_type,
            account_subtype,
            normal_balance,
            parent_id: self.parent_id,
            level: self.level.unwrap_or(0),
            path: self.path,
            is_header: self.is_header.unwrap_or(false),
            is_detail: self.is_detail.unwrap_or(true),
            currency: self.currency.unwrap_or("IDR".to_string()),
            opening_balance: self.opening_balance.unwrap_or(Decimal::from(0)),
            opening_balance_date: self.opening_balance_date,
            current_balance: self.current_balance.unwrap_or(Decimal::from(0)),
            bank_name: self.bank_name,
            bank_account_number: self.bank_account_number,
            bank_account_name: self.bank_account_name,
            bank_branch: self.bank_branch,
            is_taxable: self.is_taxable.unwrap_or(false),
            tax_rate: self.tax_rate,
            tax_account_id: self.tax_account_id,
            is_reconcilable: self.is_reconcilable.unwrap_or(false),
            last_reconciled_at: self.last_reconciled_at,
            last_reconciled_balance: self.last_reconciled_balance,
            has_budget: self.has_budget.unwrap_or(false),
            budget_amount: self.budget_amount,
            allow_manual_entry: self.allow_manual_entry.unwrap_or(true),
            require_cost_center: self.require_cost_center.unwrap_or(false),
            require_project: self.require_project.unwrap_or(false),
            sort_order: self.sort_order.unwrap_or(0),
            show_in_reports: self.show_in_reports.unwrap_or(true),
            status: self.status.unwrap_or(AccountStatus::default()),
            is_system: self.is_system.unwrap_or(false),
            notes: self.notes,
            source_id: self.source_id,
            is_cloned: self.is_cloned.unwrap_or(false),
            metadata: AuditMetadata::default(),
        })
    }
}
