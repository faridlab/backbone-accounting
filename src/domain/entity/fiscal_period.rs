use chrono::{DateTime, Utc, NaiveDate};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use rust_decimal::Decimal;

use super::PeriodType;
use super::PeriodStatus;
use super::AuditMetadata;

/// Strongly-typed ID for FiscalPeriod
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct FiscalPeriodId(pub Uuid);

impl FiscalPeriodId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for FiscalPeriodId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for FiscalPeriodId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for FiscalPeriodId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<FiscalPeriodId> for Uuid {
    fn from(id: FiscalPeriodId) -> Self { id.0 }
}

impl AsRef<Uuid> for FiscalPeriodId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for FiscalPeriodId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct FiscalPeriod {
    pub id: Uuid,
    pub company_id: Uuid,
    pub period_code: String,
    pub name: String,
    pub period_type: PeriodType,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub fiscal_year: i32,
    pub fiscal_quarter: Option<i32>,
    pub fiscal_month: Option<i32>,
    pub parent_id: Option<Uuid>,
    pub level: i32,
    pub status: PeriodStatus,
    pub is_current: bool,
    pub opening_balance_set: bool,
    pub opening_balance_date: Option<DateTime<Utc>>,
    pub opening_balance_by: Option<Uuid>,
    pub closing_started_at: Option<DateTime<Utc>>,
    pub closing_started_by: Option<Uuid>,
    pub closed_at: Option<DateTime<Utc>>,
    pub closed_by: Option<Uuid>,
    pub locked_at: Option<DateTime<Utc>>,
    pub locked_by: Option<Uuid>,
    pub lock_reason: Option<String>,
    pub allow_adjustments: bool,
    pub adjustment_deadline: Option<NaiveDate>,
    pub total_debits: Decimal,
    pub total_credits: Decimal,
    pub journal_count: i32,
    pub total_revenue: Decimal,
    pub total_expenses: Decimal,
    pub net_income: Decimal,
    pub total_assets: Decimal,
    pub total_liabilities: Decimal,
    pub total_equity: Decimal,
    pub balance_sheet_generated: bool,
    pub income_statement_generated: bool,
    pub statements_generated_at: Option<DateTime<Utc>>,
    pub notes: Option<String>,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl FiscalPeriod {
    /// Create a builder for FiscalPeriod
    pub fn builder() -> FiscalPeriodBuilder {
        FiscalPeriodBuilder::default()
    }

    /// Create a new FiscalPeriod with required fields
    pub fn new(company_id: Uuid, period_code: String, name: String, period_type: PeriodType, start_date: NaiveDate, end_date: NaiveDate, fiscal_year: i32, level: i32, status: PeriodStatus, is_current: bool, opening_balance_set: bool, allow_adjustments: bool, total_debits: Decimal, total_credits: Decimal, journal_count: i32, total_revenue: Decimal, total_expenses: Decimal, net_income: Decimal, total_assets: Decimal, total_liabilities: Decimal, total_equity: Decimal, balance_sheet_generated: bool, income_statement_generated: bool) -> Self {
        Self {
            id: Uuid::new_v4(),
            company_id,
            period_code,
            name,
            period_type,
            start_date,
            end_date,
            fiscal_year,
            fiscal_quarter: None,
            fiscal_month: None,
            parent_id: None,
            level,
            status,
            is_current,
            opening_balance_set,
            opening_balance_date: None,
            opening_balance_by: None,
            closing_started_at: None,
            closing_started_by: None,
            closed_at: None,
            closed_by: None,
            locked_at: None,
            locked_by: None,
            lock_reason: None,
            allow_adjustments,
            adjustment_deadline: None,
            total_debits,
            total_credits,
            journal_count,
            total_revenue,
            total_expenses,
            net_income,
            total_assets,
            total_liabilities,
            total_equity,
            balance_sheet_generated,
            income_statement_generated,
            statements_generated_at: None,
            notes: None,
            metadata: AuditMetadata::default(),
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> FiscalPeriodId {
        FiscalPeriodId(self.id)
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
    pub fn status(&self) -> &PeriodStatus {
        &self.status
    }


    // ==========================================================
    // Fluent Setters (with_* for optional fields)
    // ==========================================================

    /// Set the fiscal_quarter field (chainable)
    pub fn with_fiscal_quarter(mut self, value: i32) -> Self {
        self.fiscal_quarter = Some(value);
        self
    }

    /// Set the fiscal_month field (chainable)
    pub fn with_fiscal_month(mut self, value: i32) -> Self {
        self.fiscal_month = Some(value);
        self
    }

    /// Set the parent_id field (chainable)
    pub fn with_parent_id(mut self, value: Uuid) -> Self {
        self.parent_id = Some(value);
        self
    }

    /// Set the opening_balance_date field (chainable)
    pub fn with_opening_balance_date(mut self, value: DateTime<Utc>) -> Self {
        self.opening_balance_date = Some(value);
        self
    }

    /// Set the opening_balance_by field (chainable)
    pub fn with_opening_balance_by(mut self, value: Uuid) -> Self {
        self.opening_balance_by = Some(value);
        self
    }

    /// Set the closing_started_at field (chainable)
    pub fn with_closing_started_at(mut self, value: DateTime<Utc>) -> Self {
        self.closing_started_at = Some(value);
        self
    }

    /// Set the closing_started_by field (chainable)
    pub fn with_closing_started_by(mut self, value: Uuid) -> Self {
        self.closing_started_by = Some(value);
        self
    }

    /// Set the closed_at field (chainable)
    pub fn with_closed_at(mut self, value: DateTime<Utc>) -> Self {
        self.closed_at = Some(value);
        self
    }

    /// Set the closed_by field (chainable)
    pub fn with_closed_by(mut self, value: Uuid) -> Self {
        self.closed_by = Some(value);
        self
    }

    /// Set the locked_at field (chainable)
    pub fn with_locked_at(mut self, value: DateTime<Utc>) -> Self {
        self.locked_at = Some(value);
        self
    }

    /// Set the locked_by field (chainable)
    pub fn with_locked_by(mut self, value: Uuid) -> Self {
        self.locked_by = Some(value);
        self
    }

    /// Set the lock_reason field (chainable)
    pub fn with_lock_reason(mut self, value: String) -> Self {
        self.lock_reason = Some(value);
        self
    }

    /// Set the adjustment_deadline field (chainable)
    pub fn with_adjustment_deadline(mut self, value: NaiveDate) -> Self {
        self.adjustment_deadline = Some(value);
        self
    }

    /// Set the statements_generated_at field (chainable)
    pub fn with_statements_generated_at(mut self, value: DateTime<Utc>) -> Self {
        self.statements_generated_at = Some(value);
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
                "period_code" => {
                    if let Ok(v) = serde_json::from_value(value) { self.period_code = v; }
                }
                "name" => {
                    if let Ok(v) = serde_json::from_value(value) { self.name = v; }
                }
                "period_type" => {
                    if let Ok(v) = serde_json::from_value(value) { self.period_type = v; }
                }
                "start_date" => {
                    if let Ok(v) = serde_json::from_value(value) { self.start_date = v; }
                }
                "end_date" => {
                    if let Ok(v) = serde_json::from_value(value) { self.end_date = v; }
                }
                "fiscal_year" => {
                    if let Ok(v) = serde_json::from_value(value) { self.fiscal_year = v; }
                }
                "fiscal_quarter" => {
                    if let Ok(v) = serde_json::from_value(value) { self.fiscal_quarter = v; }
                }
                "fiscal_month" => {
                    if let Ok(v) = serde_json::from_value(value) { self.fiscal_month = v; }
                }
                "parent_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.parent_id = v; }
                }
                "level" => {
                    if let Ok(v) = serde_json::from_value(value) { self.level = v; }
                }
                "status" => {
                    if let Ok(v) = serde_json::from_value(value) { self.status = v; }
                }
                "is_current" => {
                    if let Ok(v) = serde_json::from_value(value) { self.is_current = v; }
                }
                "opening_balance_set" => {
                    if let Ok(v) = serde_json::from_value(value) { self.opening_balance_set = v; }
                }
                "opening_balance_date" => {
                    if let Ok(v) = serde_json::from_value(value) { self.opening_balance_date = v; }
                }
                "opening_balance_by" => {
                    if let Ok(v) = serde_json::from_value(value) { self.opening_balance_by = v; }
                }
                "closing_started_at" => {
                    if let Ok(v) = serde_json::from_value(value) { self.closing_started_at = v; }
                }
                "closing_started_by" => {
                    if let Ok(v) = serde_json::from_value(value) { self.closing_started_by = v; }
                }
                "closed_at" => {
                    if let Ok(v) = serde_json::from_value(value) { self.closed_at = v; }
                }
                "closed_by" => {
                    if let Ok(v) = serde_json::from_value(value) { self.closed_by = v; }
                }
                "locked_at" => {
                    if let Ok(v) = serde_json::from_value(value) { self.locked_at = v; }
                }
                "locked_by" => {
                    if let Ok(v) = serde_json::from_value(value) { self.locked_by = v; }
                }
                "lock_reason" => {
                    if let Ok(v) = serde_json::from_value(value) { self.lock_reason = v; }
                }
                "allow_adjustments" => {
                    if let Ok(v) = serde_json::from_value(value) { self.allow_adjustments = v; }
                }
                "adjustment_deadline" => {
                    if let Ok(v) = serde_json::from_value(value) { self.adjustment_deadline = v; }
                }
                "total_debits" => {
                    if let Ok(v) = serde_json::from_value(value) { self.total_debits = v; }
                }
                "total_credits" => {
                    if let Ok(v) = serde_json::from_value(value) { self.total_credits = v; }
                }
                "journal_count" => {
                    if let Ok(v) = serde_json::from_value(value) { self.journal_count = v; }
                }
                "total_revenue" => {
                    if let Ok(v) = serde_json::from_value(value) { self.total_revenue = v; }
                }
                "total_expenses" => {
                    if let Ok(v) = serde_json::from_value(value) { self.total_expenses = v; }
                }
                "net_income" => {
                    if let Ok(v) = serde_json::from_value(value) { self.net_income = v; }
                }
                "total_assets" => {
                    if let Ok(v) = serde_json::from_value(value) { self.total_assets = v; }
                }
                "total_liabilities" => {
                    if let Ok(v) = serde_json::from_value(value) { self.total_liabilities = v; }
                }
                "total_equity" => {
                    if let Ok(v) = serde_json::from_value(value) { self.total_equity = v; }
                }
                "balance_sheet_generated" => {
                    if let Ok(v) = serde_json::from_value(value) { self.balance_sheet_generated = v; }
                }
                "income_statement_generated" => {
                    if let Ok(v) = serde_json::from_value(value) { self.income_statement_generated = v; }
                }
                "statements_generated_at" => {
                    if let Ok(v) = serde_json::from_value(value) { self.statements_generated_at = v; }
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

impl super::Entity for FiscalPeriod {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "FiscalPeriod"
    }
}

impl backbone_core::PersistentEntity for FiscalPeriod {
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

impl backbone_orm::EntityRepoMeta for FiscalPeriod {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("company_id".to_string(), "uuid".to_string());
        m.insert("parent_id".to_string(), "uuid".to_string());
        m.insert("period_type".to_string(), "period_type".to_string());
        m.insert("status".to_string(), "period_status".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &["period_code", "name"]
    }
    fn relations() -> &'static [(&'static str, &'static str, &'static str)] {
        &[("parent", "fiscal_periods", "parentId")]
    }
}

/// Builder for FiscalPeriod entity
///
/// Provides a fluent API for constructing FiscalPeriod instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct FiscalPeriodBuilder {
    company_id: Option<Uuid>,
    period_code: Option<String>,
    name: Option<String>,
    period_type: Option<PeriodType>,
    start_date: Option<NaiveDate>,
    end_date: Option<NaiveDate>,
    fiscal_year: Option<i32>,
    fiscal_quarter: Option<i32>,
    fiscal_month: Option<i32>,
    parent_id: Option<Uuid>,
    level: Option<i32>,
    status: Option<PeriodStatus>,
    is_current: Option<bool>,
    opening_balance_set: Option<bool>,
    opening_balance_date: Option<DateTime<Utc>>,
    opening_balance_by: Option<Uuid>,
    closing_started_at: Option<DateTime<Utc>>,
    closing_started_by: Option<Uuid>,
    closed_at: Option<DateTime<Utc>>,
    closed_by: Option<Uuid>,
    locked_at: Option<DateTime<Utc>>,
    locked_by: Option<Uuid>,
    lock_reason: Option<String>,
    allow_adjustments: Option<bool>,
    adjustment_deadline: Option<NaiveDate>,
    total_debits: Option<Decimal>,
    total_credits: Option<Decimal>,
    journal_count: Option<i32>,
    total_revenue: Option<Decimal>,
    total_expenses: Option<Decimal>,
    net_income: Option<Decimal>,
    total_assets: Option<Decimal>,
    total_liabilities: Option<Decimal>,
    total_equity: Option<Decimal>,
    balance_sheet_generated: Option<bool>,
    income_statement_generated: Option<bool>,
    statements_generated_at: Option<DateTime<Utc>>,
    notes: Option<String>,
}

impl FiscalPeriodBuilder {
    /// Set the company_id field (required)
    pub fn company_id(mut self, value: Uuid) -> Self {
        self.company_id = Some(value);
        self
    }

    /// Set the period_code field (required)
    pub fn period_code(mut self, value: String) -> Self {
        self.period_code = Some(value);
        self
    }

    /// Set the name field (required)
    pub fn name(mut self, value: String) -> Self {
        self.name = Some(value);
        self
    }

    /// Set the period_type field (default: `PeriodType::default()`)
    pub fn period_type(mut self, value: PeriodType) -> Self {
        self.period_type = Some(value);
        self
    }

    /// Set the start_date field (required)
    pub fn start_date(mut self, value: NaiveDate) -> Self {
        self.start_date = Some(value);
        self
    }

    /// Set the end_date field (required)
    pub fn end_date(mut self, value: NaiveDate) -> Self {
        self.end_date = Some(value);
        self
    }

    /// Set the fiscal_year field (required)
    pub fn fiscal_year(mut self, value: i32) -> Self {
        self.fiscal_year = Some(value);
        self
    }

    /// Set the fiscal_quarter field (optional)
    pub fn fiscal_quarter(mut self, value: i32) -> Self {
        self.fiscal_quarter = Some(value);
        self
    }

    /// Set the fiscal_month field (optional)
    pub fn fiscal_month(mut self, value: i32) -> Self {
        self.fiscal_month = Some(value);
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

    /// Set the status field (default: `PeriodStatus::default()`)
    pub fn status(mut self, value: PeriodStatus) -> Self {
        self.status = Some(value);
        self
    }

    /// Set the is_current field (default: `false`)
    pub fn is_current(mut self, value: bool) -> Self {
        self.is_current = Some(value);
        self
    }

    /// Set the opening_balance_set field (default: `false`)
    pub fn opening_balance_set(mut self, value: bool) -> Self {
        self.opening_balance_set = Some(value);
        self
    }

    /// Set the opening_balance_date field (optional)
    pub fn opening_balance_date(mut self, value: DateTime<Utc>) -> Self {
        self.opening_balance_date = Some(value);
        self
    }

    /// Set the opening_balance_by field (optional)
    pub fn opening_balance_by(mut self, value: Uuid) -> Self {
        self.opening_balance_by = Some(value);
        self
    }

    /// Set the closing_started_at field (optional)
    pub fn closing_started_at(mut self, value: DateTime<Utc>) -> Self {
        self.closing_started_at = Some(value);
        self
    }

    /// Set the closing_started_by field (optional)
    pub fn closing_started_by(mut self, value: Uuid) -> Self {
        self.closing_started_by = Some(value);
        self
    }

    /// Set the closed_at field (optional)
    pub fn closed_at(mut self, value: DateTime<Utc>) -> Self {
        self.closed_at = Some(value);
        self
    }

    /// Set the closed_by field (optional)
    pub fn closed_by(mut self, value: Uuid) -> Self {
        self.closed_by = Some(value);
        self
    }

    /// Set the locked_at field (optional)
    pub fn locked_at(mut self, value: DateTime<Utc>) -> Self {
        self.locked_at = Some(value);
        self
    }

    /// Set the locked_by field (optional)
    pub fn locked_by(mut self, value: Uuid) -> Self {
        self.locked_by = Some(value);
        self
    }

    /// Set the lock_reason field (optional)
    pub fn lock_reason(mut self, value: String) -> Self {
        self.lock_reason = Some(value);
        self
    }

    /// Set the allow_adjustments field (default: `false`)
    pub fn allow_adjustments(mut self, value: bool) -> Self {
        self.allow_adjustments = Some(value);
        self
    }

    /// Set the adjustment_deadline field (optional)
    pub fn adjustment_deadline(mut self, value: NaiveDate) -> Self {
        self.adjustment_deadline = Some(value);
        self
    }

    /// Set the total_debits field (default: `Decimal::from(0)`)
    pub fn total_debits(mut self, value: Decimal) -> Self {
        self.total_debits = Some(value);
        self
    }

    /// Set the total_credits field (default: `Decimal::from(0)`)
    pub fn total_credits(mut self, value: Decimal) -> Self {
        self.total_credits = Some(value);
        self
    }

    /// Set the journal_count field (default: `0`)
    pub fn journal_count(mut self, value: i32) -> Self {
        self.journal_count = Some(value);
        self
    }

    /// Set the total_revenue field (default: `Decimal::from(0)`)
    pub fn total_revenue(mut self, value: Decimal) -> Self {
        self.total_revenue = Some(value);
        self
    }

    /// Set the total_expenses field (default: `Decimal::from(0)`)
    pub fn total_expenses(mut self, value: Decimal) -> Self {
        self.total_expenses = Some(value);
        self
    }

    /// Set the net_income field (default: `Decimal::from(0)`)
    pub fn net_income(mut self, value: Decimal) -> Self {
        self.net_income = Some(value);
        self
    }

    /// Set the total_assets field (default: `Decimal::from(0)`)
    pub fn total_assets(mut self, value: Decimal) -> Self {
        self.total_assets = Some(value);
        self
    }

    /// Set the total_liabilities field (default: `Decimal::from(0)`)
    pub fn total_liabilities(mut self, value: Decimal) -> Self {
        self.total_liabilities = Some(value);
        self
    }

    /// Set the total_equity field (default: `Decimal::from(0)`)
    pub fn total_equity(mut self, value: Decimal) -> Self {
        self.total_equity = Some(value);
        self
    }

    /// Set the balance_sheet_generated field (default: `false`)
    pub fn balance_sheet_generated(mut self, value: bool) -> Self {
        self.balance_sheet_generated = Some(value);
        self
    }

    /// Set the income_statement_generated field (default: `false`)
    pub fn income_statement_generated(mut self, value: bool) -> Self {
        self.income_statement_generated = Some(value);
        self
    }

    /// Set the statements_generated_at field (optional)
    pub fn statements_generated_at(mut self, value: DateTime<Utc>) -> Self {
        self.statements_generated_at = Some(value);
        self
    }

    /// Set the notes field (optional)
    pub fn notes(mut self, value: String) -> Self {
        self.notes = Some(value);
        self
    }

    /// Build the FiscalPeriod entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<FiscalPeriod, String> {
        let company_id = self.company_id.ok_or_else(|| "company_id is required".to_string())?;
        let period_code = self.period_code.ok_or_else(|| "period_code is required".to_string())?;
        let name = self.name.ok_or_else(|| "name is required".to_string())?;
        let start_date = self.start_date.ok_or_else(|| "start_date is required".to_string())?;
        let end_date = self.end_date.ok_or_else(|| "end_date is required".to_string())?;
        let fiscal_year = self.fiscal_year.ok_or_else(|| "fiscal_year is required".to_string())?;

        Ok(FiscalPeriod {
            id: Uuid::new_v4(),
            company_id,
            period_code,
            name,
            period_type: self.period_type.unwrap_or(PeriodType::default()),
            start_date,
            end_date,
            fiscal_year,
            fiscal_quarter: self.fiscal_quarter,
            fiscal_month: self.fiscal_month,
            parent_id: self.parent_id,
            level: self.level.unwrap_or(0),
            status: self.status.unwrap_or(PeriodStatus::default()),
            is_current: self.is_current.unwrap_or(false),
            opening_balance_set: self.opening_balance_set.unwrap_or(false),
            opening_balance_date: self.opening_balance_date,
            opening_balance_by: self.opening_balance_by,
            closing_started_at: self.closing_started_at,
            closing_started_by: self.closing_started_by,
            closed_at: self.closed_at,
            closed_by: self.closed_by,
            locked_at: self.locked_at,
            locked_by: self.locked_by,
            lock_reason: self.lock_reason,
            allow_adjustments: self.allow_adjustments.unwrap_or(false),
            adjustment_deadline: self.adjustment_deadline,
            total_debits: self.total_debits.unwrap_or(Decimal::from(0)),
            total_credits: self.total_credits.unwrap_or(Decimal::from(0)),
            journal_count: self.journal_count.unwrap_or(0),
            total_revenue: self.total_revenue.unwrap_or(Decimal::from(0)),
            total_expenses: self.total_expenses.unwrap_or(Decimal::from(0)),
            net_income: self.net_income.unwrap_or(Decimal::from(0)),
            total_assets: self.total_assets.unwrap_or(Decimal::from(0)),
            total_liabilities: self.total_liabilities.unwrap_or(Decimal::from(0)),
            total_equity: self.total_equity.unwrap_or(Decimal::from(0)),
            balance_sheet_generated: self.balance_sheet_generated.unwrap_or(false),
            income_statement_generated: self.income_statement_generated.unwrap_or(false),
            statements_generated_at: self.statements_generated_at,
            notes: self.notes,
            metadata: AuditMetadata::default(),
        })
    }
}
