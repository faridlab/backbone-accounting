use chrono::{DateTime, Utc, NaiveDate};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use rust_decimal::Decimal;

use super::StatementType;
use super::StatementStatus;
use super::AuditMetadata;

/// Strongly-typed ID for FinancialStatement
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct FinancialStatementId(pub Uuid);

impl FinancialStatementId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for FinancialStatementId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for FinancialStatementId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for FinancialStatementId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<FinancialStatementId> for Uuid {
    fn from(id: FinancialStatementId) -> Self { id.0 }
}

impl AsRef<Uuid> for FinancialStatementId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for FinancialStatementId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct FinancialStatement {
    pub id: Uuid,
    pub company_id: Uuid,
    pub statement_number: String,
    pub statement_type: StatementType,
    pub name: String,
    pub fiscal_period_id: Option<Uuid>,
    pub fiscal_year: i32,
    pub fiscal_month: Option<i32>,
    pub as_of_date: NaiveDate,
    pub period_start: Option<NaiveDate>,
    pub period_end: Option<NaiveDate>,
    pub is_comparative: bool,
    pub comparative_period_id: Option<Uuid>,
    pub comparative_as_of_date: Option<NaiveDate>,
    pub status: StatementStatus,
    pub total_current_assets: Option<Decimal>,
    pub total_non_current_assets: Option<Decimal>,
    pub total_assets: Option<Decimal>,
    pub total_current_liabilities: Option<Decimal>,
    pub total_non_current_liabilities: Option<Decimal>,
    pub total_liabilities: Option<Decimal>,
    pub total_equity: Option<Decimal>,
    pub balance_check: bool,
    pub balance_difference: Option<Decimal>,
    pub total_revenue: Option<Decimal>,
    pub total_cogs: Option<Decimal>,
    pub gross_profit: Option<Decimal>,
    pub gross_profit_margin: Option<Decimal>,
    pub total_operating_expenses: Option<Decimal>,
    pub operating_income: Option<Decimal>,
    pub operating_margin: Option<Decimal>,
    pub total_other_income: Option<Decimal>,
    pub total_other_expenses: Option<Decimal>,
    pub income_before_tax: Option<Decimal>,
    pub tax_expense: Option<Decimal>,
    pub net_income: Option<Decimal>,
    pub net_profit_margin: Option<Decimal>,
    pub cash_from_operations: Option<Decimal>,
    pub cash_from_investing: Option<Decimal>,
    pub cash_from_financing: Option<Decimal>,
    pub net_cash_change: Option<Decimal>,
    pub beginning_cash: Option<Decimal>,
    pub ending_cash: Option<Decimal>,
    pub total_debits: Option<Decimal>,
    pub total_credits: Option<Decimal>,
    pub trial_balance_check: bool,
    pub line_items: serde_json::Value,
    pub comparative_data: serde_json::Value,
    pub variance_data: serde_json::Value,
    pub notes: serde_json::Value,
    pub management_notes: Option<String>,
    pub generated_at: Option<DateTime<Utc>>,
    pub generated_by: Option<Uuid>,
    pub generation_parameters: serde_json::Value,
    pub reviewed_at: Option<DateTime<Utc>>,
    pub reviewed_by: Option<Uuid>,
    pub approved_at: Option<DateTime<Utc>>,
    pub approved_by: Option<Uuid>,
    pub published_at: Option<DateTime<Utc>>,
    pub pdf_url: Option<String>,
    pub excel_url: Option<String>,
    pub currency: String,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl FinancialStatement {
    /// Create a builder for FinancialStatement
    pub fn builder() -> FinancialStatementBuilder {
        FinancialStatementBuilder::default()
    }

    /// Create a new FinancialStatement with required fields
    pub fn new(company_id: Uuid, statement_number: String, statement_type: StatementType, name: String, fiscal_year: i32, as_of_date: NaiveDate, is_comparative: bool, status: StatementStatus, balance_check: bool, trial_balance_check: bool, line_items: serde_json::Value, comparative_data: serde_json::Value, variance_data: serde_json::Value, notes: serde_json::Value, generation_parameters: serde_json::Value, currency: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            company_id,
            statement_number,
            statement_type,
            name,
            fiscal_period_id: None,
            fiscal_year,
            fiscal_month: None,
            as_of_date,
            period_start: None,
            period_end: None,
            is_comparative,
            comparative_period_id: None,
            comparative_as_of_date: None,
            status,
            total_current_assets: None,
            total_non_current_assets: None,
            total_assets: None,
            total_current_liabilities: None,
            total_non_current_liabilities: None,
            total_liabilities: None,
            total_equity: None,
            balance_check,
            balance_difference: None,
            total_revenue: None,
            total_cogs: None,
            gross_profit: None,
            gross_profit_margin: None,
            total_operating_expenses: None,
            operating_income: None,
            operating_margin: None,
            total_other_income: None,
            total_other_expenses: None,
            income_before_tax: None,
            tax_expense: None,
            net_income: None,
            net_profit_margin: None,
            cash_from_operations: None,
            cash_from_investing: None,
            cash_from_financing: None,
            net_cash_change: None,
            beginning_cash: None,
            ending_cash: None,
            total_debits: None,
            total_credits: None,
            trial_balance_check,
            line_items,
            comparative_data,
            variance_data,
            notes,
            management_notes: None,
            generated_at: None,
            generated_by: None,
            generation_parameters,
            reviewed_at: None,
            reviewed_by: None,
            approved_at: None,
            approved_by: None,
            published_at: None,
            pdf_url: None,
            excel_url: None,
            currency,
            metadata: AuditMetadata::default(),
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> FinancialStatementId {
        FinancialStatementId(self.id)
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
    pub fn status(&self) -> &StatementStatus {
        &self.status
    }


    // ==========================================================
    // Fluent Setters (with_* for optional fields)
    // ==========================================================

    /// Set the fiscal_period_id field (chainable)
    pub fn with_fiscal_period_id(mut self, value: Uuid) -> Self {
        self.fiscal_period_id = Some(value);
        self
    }

    /// Set the fiscal_month field (chainable)
    pub fn with_fiscal_month(mut self, value: i32) -> Self {
        self.fiscal_month = Some(value);
        self
    }

    /// Set the period_start field (chainable)
    pub fn with_period_start(mut self, value: NaiveDate) -> Self {
        self.period_start = Some(value);
        self
    }

    /// Set the period_end field (chainable)
    pub fn with_period_end(mut self, value: NaiveDate) -> Self {
        self.period_end = Some(value);
        self
    }

    /// Set the comparative_period_id field (chainable)
    pub fn with_comparative_period_id(mut self, value: Uuid) -> Self {
        self.comparative_period_id = Some(value);
        self
    }

    /// Set the comparative_as_of_date field (chainable)
    pub fn with_comparative_as_of_date(mut self, value: NaiveDate) -> Self {
        self.comparative_as_of_date = Some(value);
        self
    }

    /// Set the total_current_assets field (chainable)
    pub fn with_total_current_assets(mut self, value: Decimal) -> Self {
        self.total_current_assets = Some(value);
        self
    }

    /// Set the total_non_current_assets field (chainable)
    pub fn with_total_non_current_assets(mut self, value: Decimal) -> Self {
        self.total_non_current_assets = Some(value);
        self
    }

    /// Set the total_assets field (chainable)
    pub fn with_total_assets(mut self, value: Decimal) -> Self {
        self.total_assets = Some(value);
        self
    }

    /// Set the total_current_liabilities field (chainable)
    pub fn with_total_current_liabilities(mut self, value: Decimal) -> Self {
        self.total_current_liabilities = Some(value);
        self
    }

    /// Set the total_non_current_liabilities field (chainable)
    pub fn with_total_non_current_liabilities(mut self, value: Decimal) -> Self {
        self.total_non_current_liabilities = Some(value);
        self
    }

    /// Set the total_liabilities field (chainable)
    pub fn with_total_liabilities(mut self, value: Decimal) -> Self {
        self.total_liabilities = Some(value);
        self
    }

    /// Set the total_equity field (chainable)
    pub fn with_total_equity(mut self, value: Decimal) -> Self {
        self.total_equity = Some(value);
        self
    }

    /// Set the balance_difference field (chainable)
    pub fn with_balance_difference(mut self, value: Decimal) -> Self {
        self.balance_difference = Some(value);
        self
    }

    /// Set the total_revenue field (chainable)
    pub fn with_total_revenue(mut self, value: Decimal) -> Self {
        self.total_revenue = Some(value);
        self
    }

    /// Set the total_cogs field (chainable)
    pub fn with_total_cogs(mut self, value: Decimal) -> Self {
        self.total_cogs = Some(value);
        self
    }

    /// Set the gross_profit field (chainable)
    pub fn with_gross_profit(mut self, value: Decimal) -> Self {
        self.gross_profit = Some(value);
        self
    }

    /// Set the gross_profit_margin field (chainable)
    pub fn with_gross_profit_margin(mut self, value: Decimal) -> Self {
        self.gross_profit_margin = Some(value);
        self
    }

    /// Set the total_operating_expenses field (chainable)
    pub fn with_total_operating_expenses(mut self, value: Decimal) -> Self {
        self.total_operating_expenses = Some(value);
        self
    }

    /// Set the operating_income field (chainable)
    pub fn with_operating_income(mut self, value: Decimal) -> Self {
        self.operating_income = Some(value);
        self
    }

    /// Set the operating_margin field (chainable)
    pub fn with_operating_margin(mut self, value: Decimal) -> Self {
        self.operating_margin = Some(value);
        self
    }

    /// Set the total_other_income field (chainable)
    pub fn with_total_other_income(mut self, value: Decimal) -> Self {
        self.total_other_income = Some(value);
        self
    }

    /// Set the total_other_expenses field (chainable)
    pub fn with_total_other_expenses(mut self, value: Decimal) -> Self {
        self.total_other_expenses = Some(value);
        self
    }

    /// Set the income_before_tax field (chainable)
    pub fn with_income_before_tax(mut self, value: Decimal) -> Self {
        self.income_before_tax = Some(value);
        self
    }

    /// Set the tax_expense field (chainable)
    pub fn with_tax_expense(mut self, value: Decimal) -> Self {
        self.tax_expense = Some(value);
        self
    }

    /// Set the net_income field (chainable)
    pub fn with_net_income(mut self, value: Decimal) -> Self {
        self.net_income = Some(value);
        self
    }

    /// Set the net_profit_margin field (chainable)
    pub fn with_net_profit_margin(mut self, value: Decimal) -> Self {
        self.net_profit_margin = Some(value);
        self
    }

    /// Set the cash_from_operations field (chainable)
    pub fn with_cash_from_operations(mut self, value: Decimal) -> Self {
        self.cash_from_operations = Some(value);
        self
    }

    /// Set the cash_from_investing field (chainable)
    pub fn with_cash_from_investing(mut self, value: Decimal) -> Self {
        self.cash_from_investing = Some(value);
        self
    }

    /// Set the cash_from_financing field (chainable)
    pub fn with_cash_from_financing(mut self, value: Decimal) -> Self {
        self.cash_from_financing = Some(value);
        self
    }

    /// Set the net_cash_change field (chainable)
    pub fn with_net_cash_change(mut self, value: Decimal) -> Self {
        self.net_cash_change = Some(value);
        self
    }

    /// Set the beginning_cash field (chainable)
    pub fn with_beginning_cash(mut self, value: Decimal) -> Self {
        self.beginning_cash = Some(value);
        self
    }

    /// Set the ending_cash field (chainable)
    pub fn with_ending_cash(mut self, value: Decimal) -> Self {
        self.ending_cash = Some(value);
        self
    }

    /// Set the total_debits field (chainable)
    pub fn with_total_debits(mut self, value: Decimal) -> Self {
        self.total_debits = Some(value);
        self
    }

    /// Set the total_credits field (chainable)
    pub fn with_total_credits(mut self, value: Decimal) -> Self {
        self.total_credits = Some(value);
        self
    }

    /// Set the management_notes field (chainable)
    pub fn with_management_notes(mut self, value: String) -> Self {
        self.management_notes = Some(value);
        self
    }

    /// Set the generated_at field (chainable)
    pub fn with_generated_at(mut self, value: DateTime<Utc>) -> Self {
        self.generated_at = Some(value);
        self
    }

    /// Set the generated_by field (chainable)
    pub fn with_generated_by(mut self, value: Uuid) -> Self {
        self.generated_by = Some(value);
        self
    }

    /// Set the reviewed_at field (chainable)
    pub fn with_reviewed_at(mut self, value: DateTime<Utc>) -> Self {
        self.reviewed_at = Some(value);
        self
    }

    /// Set the reviewed_by field (chainable)
    pub fn with_reviewed_by(mut self, value: Uuid) -> Self {
        self.reviewed_by = Some(value);
        self
    }

    /// Set the approved_at field (chainable)
    pub fn with_approved_at(mut self, value: DateTime<Utc>) -> Self {
        self.approved_at = Some(value);
        self
    }

    /// Set the approved_by field (chainable)
    pub fn with_approved_by(mut self, value: Uuid) -> Self {
        self.approved_by = Some(value);
        self
    }

    /// Set the published_at field (chainable)
    pub fn with_published_at(mut self, value: DateTime<Utc>) -> Self {
        self.published_at = Some(value);
        self
    }

    /// Set the pdf_url field (chainable)
    pub fn with_pdf_url(mut self, value: String) -> Self {
        self.pdf_url = Some(value);
        self
    }

    /// Set the excel_url field (chainable)
    pub fn with_excel_url(mut self, value: String) -> Self {
        self.excel_url = Some(value);
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
                "statement_number" => {
                    if let Ok(v) = serde_json::from_value(value) { self.statement_number = v; }
                }
                "statement_type" => {
                    if let Ok(v) = serde_json::from_value(value) { self.statement_type = v; }
                }
                "name" => {
                    if let Ok(v) = serde_json::from_value(value) { self.name = v; }
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
                "as_of_date" => {
                    if let Ok(v) = serde_json::from_value(value) { self.as_of_date = v; }
                }
                "period_start" => {
                    if let Ok(v) = serde_json::from_value(value) { self.period_start = v; }
                }
                "period_end" => {
                    if let Ok(v) = serde_json::from_value(value) { self.period_end = v; }
                }
                "is_comparative" => {
                    if let Ok(v) = serde_json::from_value(value) { self.is_comparative = v; }
                }
                "comparative_period_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.comparative_period_id = v; }
                }
                "comparative_as_of_date" => {
                    if let Ok(v) = serde_json::from_value(value) { self.comparative_as_of_date = v; }
                }
                "status" => {
                    if let Ok(v) = serde_json::from_value(value) { self.status = v; }
                }
                "total_current_assets" => {
                    if let Ok(v) = serde_json::from_value(value) { self.total_current_assets = v; }
                }
                "total_non_current_assets" => {
                    if let Ok(v) = serde_json::from_value(value) { self.total_non_current_assets = v; }
                }
                "total_assets" => {
                    if let Ok(v) = serde_json::from_value(value) { self.total_assets = v; }
                }
                "total_current_liabilities" => {
                    if let Ok(v) = serde_json::from_value(value) { self.total_current_liabilities = v; }
                }
                "total_non_current_liabilities" => {
                    if let Ok(v) = serde_json::from_value(value) { self.total_non_current_liabilities = v; }
                }
                "total_liabilities" => {
                    if let Ok(v) = serde_json::from_value(value) { self.total_liabilities = v; }
                }
                "total_equity" => {
                    if let Ok(v) = serde_json::from_value(value) { self.total_equity = v; }
                }
                "balance_check" => {
                    if let Ok(v) = serde_json::from_value(value) { self.balance_check = v; }
                }
                "balance_difference" => {
                    if let Ok(v) = serde_json::from_value(value) { self.balance_difference = v; }
                }
                "total_revenue" => {
                    if let Ok(v) = serde_json::from_value(value) { self.total_revenue = v; }
                }
                "total_cogs" => {
                    if let Ok(v) = serde_json::from_value(value) { self.total_cogs = v; }
                }
                "gross_profit" => {
                    if let Ok(v) = serde_json::from_value(value) { self.gross_profit = v; }
                }
                "gross_profit_margin" => {
                    if let Ok(v) = serde_json::from_value(value) { self.gross_profit_margin = v; }
                }
                "total_operating_expenses" => {
                    if let Ok(v) = serde_json::from_value(value) { self.total_operating_expenses = v; }
                }
                "operating_income" => {
                    if let Ok(v) = serde_json::from_value(value) { self.operating_income = v; }
                }
                "operating_margin" => {
                    if let Ok(v) = serde_json::from_value(value) { self.operating_margin = v; }
                }
                "total_other_income" => {
                    if let Ok(v) = serde_json::from_value(value) { self.total_other_income = v; }
                }
                "total_other_expenses" => {
                    if let Ok(v) = serde_json::from_value(value) { self.total_other_expenses = v; }
                }
                "income_before_tax" => {
                    if let Ok(v) = serde_json::from_value(value) { self.income_before_tax = v; }
                }
                "tax_expense" => {
                    if let Ok(v) = serde_json::from_value(value) { self.tax_expense = v; }
                }
                "net_income" => {
                    if let Ok(v) = serde_json::from_value(value) { self.net_income = v; }
                }
                "net_profit_margin" => {
                    if let Ok(v) = serde_json::from_value(value) { self.net_profit_margin = v; }
                }
                "cash_from_operations" => {
                    if let Ok(v) = serde_json::from_value(value) { self.cash_from_operations = v; }
                }
                "cash_from_investing" => {
                    if let Ok(v) = serde_json::from_value(value) { self.cash_from_investing = v; }
                }
                "cash_from_financing" => {
                    if let Ok(v) = serde_json::from_value(value) { self.cash_from_financing = v; }
                }
                "net_cash_change" => {
                    if let Ok(v) = serde_json::from_value(value) { self.net_cash_change = v; }
                }
                "beginning_cash" => {
                    if let Ok(v) = serde_json::from_value(value) { self.beginning_cash = v; }
                }
                "ending_cash" => {
                    if let Ok(v) = serde_json::from_value(value) { self.ending_cash = v; }
                }
                "total_debits" => {
                    if let Ok(v) = serde_json::from_value(value) { self.total_debits = v; }
                }
                "total_credits" => {
                    if let Ok(v) = serde_json::from_value(value) { self.total_credits = v; }
                }
                "trial_balance_check" => {
                    if let Ok(v) = serde_json::from_value(value) { self.trial_balance_check = v; }
                }
                "line_items" => {
                    if let Ok(v) = serde_json::from_value(value) { self.line_items = v; }
                }
                "comparative_data" => {
                    if let Ok(v) = serde_json::from_value(value) { self.comparative_data = v; }
                }
                "variance_data" => {
                    if let Ok(v) = serde_json::from_value(value) { self.variance_data = v; }
                }
                "notes" => {
                    if let Ok(v) = serde_json::from_value(value) { self.notes = v; }
                }
                "management_notes" => {
                    if let Ok(v) = serde_json::from_value(value) { self.management_notes = v; }
                }
                "generated_at" => {
                    if let Ok(v) = serde_json::from_value(value) { self.generated_at = v; }
                }
                "generated_by" => {
                    if let Ok(v) = serde_json::from_value(value) { self.generated_by = v; }
                }
                "generation_parameters" => {
                    if let Ok(v) = serde_json::from_value(value) { self.generation_parameters = v; }
                }
                "reviewed_at" => {
                    if let Ok(v) = serde_json::from_value(value) { self.reviewed_at = v; }
                }
                "reviewed_by" => {
                    if let Ok(v) = serde_json::from_value(value) { self.reviewed_by = v; }
                }
                "approved_at" => {
                    if let Ok(v) = serde_json::from_value(value) { self.approved_at = v; }
                }
                "approved_by" => {
                    if let Ok(v) = serde_json::from_value(value) { self.approved_by = v; }
                }
                "published_at" => {
                    if let Ok(v) = serde_json::from_value(value) { self.published_at = v; }
                }
                "pdf_url" => {
                    if let Ok(v) = serde_json::from_value(value) { self.pdf_url = v; }
                }
                "excel_url" => {
                    if let Ok(v) = serde_json::from_value(value) { self.excel_url = v; }
                }
                "currency" => {
                    if let Ok(v) = serde_json::from_value(value) { self.currency = v; }
                }
                _ => {} // ignore unknown fields
            }
        }
    }

    // <<< CUSTOM METHODS START >>>
    // <<< CUSTOM METHODS END >>>
}

impl super::Entity for FinancialStatement {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "FinancialStatement"
    }
}

impl backbone_core::PersistentEntity for FinancialStatement {
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

impl backbone_orm::EntityRepoMeta for FinancialStatement {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("company_id".to_string(), "uuid".to_string());
        m.insert("fiscal_period_id".to_string(), "uuid".to_string());
        m.insert("comparative_period_id".to_string(), "uuid".to_string());
        m.insert("statement_type".to_string(), "statement_type".to_string());
        m.insert("status".to_string(), "statement_status".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &["statement_number", "name", "currency"]
    }
}

/// Builder for FinancialStatement entity
///
/// Provides a fluent API for constructing FinancialStatement instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct FinancialStatementBuilder {
    company_id: Option<Uuid>,
    statement_number: Option<String>,
    statement_type: Option<StatementType>,
    name: Option<String>,
    fiscal_period_id: Option<Uuid>,
    fiscal_year: Option<i32>,
    fiscal_month: Option<i32>,
    as_of_date: Option<NaiveDate>,
    period_start: Option<NaiveDate>,
    period_end: Option<NaiveDate>,
    is_comparative: Option<bool>,
    comparative_period_id: Option<Uuid>,
    comparative_as_of_date: Option<NaiveDate>,
    status: Option<StatementStatus>,
    total_current_assets: Option<Decimal>,
    total_non_current_assets: Option<Decimal>,
    total_assets: Option<Decimal>,
    total_current_liabilities: Option<Decimal>,
    total_non_current_liabilities: Option<Decimal>,
    total_liabilities: Option<Decimal>,
    total_equity: Option<Decimal>,
    balance_check: Option<bool>,
    balance_difference: Option<Decimal>,
    total_revenue: Option<Decimal>,
    total_cogs: Option<Decimal>,
    gross_profit: Option<Decimal>,
    gross_profit_margin: Option<Decimal>,
    total_operating_expenses: Option<Decimal>,
    operating_income: Option<Decimal>,
    operating_margin: Option<Decimal>,
    total_other_income: Option<Decimal>,
    total_other_expenses: Option<Decimal>,
    income_before_tax: Option<Decimal>,
    tax_expense: Option<Decimal>,
    net_income: Option<Decimal>,
    net_profit_margin: Option<Decimal>,
    cash_from_operations: Option<Decimal>,
    cash_from_investing: Option<Decimal>,
    cash_from_financing: Option<Decimal>,
    net_cash_change: Option<Decimal>,
    beginning_cash: Option<Decimal>,
    ending_cash: Option<Decimal>,
    total_debits: Option<Decimal>,
    total_credits: Option<Decimal>,
    trial_balance_check: Option<bool>,
    line_items: Option<serde_json::Value>,
    comparative_data: Option<serde_json::Value>,
    variance_data: Option<serde_json::Value>,
    notes: Option<serde_json::Value>,
    management_notes: Option<String>,
    generated_at: Option<DateTime<Utc>>,
    generated_by: Option<Uuid>,
    generation_parameters: Option<serde_json::Value>,
    reviewed_at: Option<DateTime<Utc>>,
    reviewed_by: Option<Uuid>,
    approved_at: Option<DateTime<Utc>>,
    approved_by: Option<Uuid>,
    published_at: Option<DateTime<Utc>>,
    pdf_url: Option<String>,
    excel_url: Option<String>,
    currency: Option<String>,
}

impl FinancialStatementBuilder {
    /// Set the company_id field (required)
    pub fn company_id(mut self, value: Uuid) -> Self {
        self.company_id = Some(value);
        self
    }

    /// Set the statement_number field (required)
    pub fn statement_number(mut self, value: String) -> Self {
        self.statement_number = Some(value);
        self
    }

    /// Set the statement_type field (required)
    pub fn statement_type(mut self, value: StatementType) -> Self {
        self.statement_type = Some(value);
        self
    }

    /// Set the name field (required)
    pub fn name(mut self, value: String) -> Self {
        self.name = Some(value);
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

    /// Set the fiscal_month field (optional)
    pub fn fiscal_month(mut self, value: i32) -> Self {
        self.fiscal_month = Some(value);
        self
    }

    /// Set the as_of_date field (required)
    pub fn as_of_date(mut self, value: NaiveDate) -> Self {
        self.as_of_date = Some(value);
        self
    }

    /// Set the period_start field (optional)
    pub fn period_start(mut self, value: NaiveDate) -> Self {
        self.period_start = Some(value);
        self
    }

    /// Set the period_end field (optional)
    pub fn period_end(mut self, value: NaiveDate) -> Self {
        self.period_end = Some(value);
        self
    }

    /// Set the is_comparative field (default: `false`)
    pub fn is_comparative(mut self, value: bool) -> Self {
        self.is_comparative = Some(value);
        self
    }

    /// Set the comparative_period_id field (optional)
    pub fn comparative_period_id(mut self, value: Uuid) -> Self {
        self.comparative_period_id = Some(value);
        self
    }

    /// Set the comparative_as_of_date field (optional)
    pub fn comparative_as_of_date(mut self, value: NaiveDate) -> Self {
        self.comparative_as_of_date = Some(value);
        self
    }

    /// Set the status field (default: `StatementStatus::default()`)
    pub fn status(mut self, value: StatementStatus) -> Self {
        self.status = Some(value);
        self
    }

    /// Set the total_current_assets field (optional)
    pub fn total_current_assets(mut self, value: Decimal) -> Self {
        self.total_current_assets = Some(value);
        self
    }

    /// Set the total_non_current_assets field (optional)
    pub fn total_non_current_assets(mut self, value: Decimal) -> Self {
        self.total_non_current_assets = Some(value);
        self
    }

    /// Set the total_assets field (optional)
    pub fn total_assets(mut self, value: Decimal) -> Self {
        self.total_assets = Some(value);
        self
    }

    /// Set the total_current_liabilities field (optional)
    pub fn total_current_liabilities(mut self, value: Decimal) -> Self {
        self.total_current_liabilities = Some(value);
        self
    }

    /// Set the total_non_current_liabilities field (optional)
    pub fn total_non_current_liabilities(mut self, value: Decimal) -> Self {
        self.total_non_current_liabilities = Some(value);
        self
    }

    /// Set the total_liabilities field (optional)
    pub fn total_liabilities(mut self, value: Decimal) -> Self {
        self.total_liabilities = Some(value);
        self
    }

    /// Set the total_equity field (optional)
    pub fn total_equity(mut self, value: Decimal) -> Self {
        self.total_equity = Some(value);
        self
    }

    /// Set the balance_check field (default: `false`)
    pub fn balance_check(mut self, value: bool) -> Self {
        self.balance_check = Some(value);
        self
    }

    /// Set the balance_difference field (optional)
    pub fn balance_difference(mut self, value: Decimal) -> Self {
        self.balance_difference = Some(value);
        self
    }

    /// Set the total_revenue field (optional)
    pub fn total_revenue(mut self, value: Decimal) -> Self {
        self.total_revenue = Some(value);
        self
    }

    /// Set the total_cogs field (optional)
    pub fn total_cogs(mut self, value: Decimal) -> Self {
        self.total_cogs = Some(value);
        self
    }

    /// Set the gross_profit field (optional)
    pub fn gross_profit(mut self, value: Decimal) -> Self {
        self.gross_profit = Some(value);
        self
    }

    /// Set the gross_profit_margin field (optional)
    pub fn gross_profit_margin(mut self, value: Decimal) -> Self {
        self.gross_profit_margin = Some(value);
        self
    }

    /// Set the total_operating_expenses field (optional)
    pub fn total_operating_expenses(mut self, value: Decimal) -> Self {
        self.total_operating_expenses = Some(value);
        self
    }

    /// Set the operating_income field (optional)
    pub fn operating_income(mut self, value: Decimal) -> Self {
        self.operating_income = Some(value);
        self
    }

    /// Set the operating_margin field (optional)
    pub fn operating_margin(mut self, value: Decimal) -> Self {
        self.operating_margin = Some(value);
        self
    }

    /// Set the total_other_income field (optional)
    pub fn total_other_income(mut self, value: Decimal) -> Self {
        self.total_other_income = Some(value);
        self
    }

    /// Set the total_other_expenses field (optional)
    pub fn total_other_expenses(mut self, value: Decimal) -> Self {
        self.total_other_expenses = Some(value);
        self
    }

    /// Set the income_before_tax field (optional)
    pub fn income_before_tax(mut self, value: Decimal) -> Self {
        self.income_before_tax = Some(value);
        self
    }

    /// Set the tax_expense field (optional)
    pub fn tax_expense(mut self, value: Decimal) -> Self {
        self.tax_expense = Some(value);
        self
    }

    /// Set the net_income field (optional)
    pub fn net_income(mut self, value: Decimal) -> Self {
        self.net_income = Some(value);
        self
    }

    /// Set the net_profit_margin field (optional)
    pub fn net_profit_margin(mut self, value: Decimal) -> Self {
        self.net_profit_margin = Some(value);
        self
    }

    /// Set the cash_from_operations field (optional)
    pub fn cash_from_operations(mut self, value: Decimal) -> Self {
        self.cash_from_operations = Some(value);
        self
    }

    /// Set the cash_from_investing field (optional)
    pub fn cash_from_investing(mut self, value: Decimal) -> Self {
        self.cash_from_investing = Some(value);
        self
    }

    /// Set the cash_from_financing field (optional)
    pub fn cash_from_financing(mut self, value: Decimal) -> Self {
        self.cash_from_financing = Some(value);
        self
    }

    /// Set the net_cash_change field (optional)
    pub fn net_cash_change(mut self, value: Decimal) -> Self {
        self.net_cash_change = Some(value);
        self
    }

    /// Set the beginning_cash field (optional)
    pub fn beginning_cash(mut self, value: Decimal) -> Self {
        self.beginning_cash = Some(value);
        self
    }

    /// Set the ending_cash field (optional)
    pub fn ending_cash(mut self, value: Decimal) -> Self {
        self.ending_cash = Some(value);
        self
    }

    /// Set the total_debits field (optional)
    pub fn total_debits(mut self, value: Decimal) -> Self {
        self.total_debits = Some(value);
        self
    }

    /// Set the total_credits field (optional)
    pub fn total_credits(mut self, value: Decimal) -> Self {
        self.total_credits = Some(value);
        self
    }

    /// Set the trial_balance_check field (default: `false`)
    pub fn trial_balance_check(mut self, value: bool) -> Self {
        self.trial_balance_check = Some(value);
        self
    }

    /// Set the line_items field (default: `serde_json::json!([])`)
    pub fn line_items(mut self, value: serde_json::Value) -> Self {
        self.line_items = Some(value);
        self
    }

    /// Set the comparative_data field (default: `serde_json::json!({})`)
    pub fn comparative_data(mut self, value: serde_json::Value) -> Self {
        self.comparative_data = Some(value);
        self
    }

    /// Set the variance_data field (default: `serde_json::json!({})`)
    pub fn variance_data(mut self, value: serde_json::Value) -> Self {
        self.variance_data = Some(value);
        self
    }

    /// Set the notes field (default: `serde_json::json!([])`)
    pub fn notes(mut self, value: serde_json::Value) -> Self {
        self.notes = Some(value);
        self
    }

    /// Set the management_notes field (optional)
    pub fn management_notes(mut self, value: String) -> Self {
        self.management_notes = Some(value);
        self
    }

    /// Set the generated_at field (optional)
    pub fn generated_at(mut self, value: DateTime<Utc>) -> Self {
        self.generated_at = Some(value);
        self
    }

    /// Set the generated_by field (optional)
    pub fn generated_by(mut self, value: Uuid) -> Self {
        self.generated_by = Some(value);
        self
    }

    /// Set the generation_parameters field (default: `serde_json::json!({})`)
    pub fn generation_parameters(mut self, value: serde_json::Value) -> Self {
        self.generation_parameters = Some(value);
        self
    }

    /// Set the reviewed_at field (optional)
    pub fn reviewed_at(mut self, value: DateTime<Utc>) -> Self {
        self.reviewed_at = Some(value);
        self
    }

    /// Set the reviewed_by field (optional)
    pub fn reviewed_by(mut self, value: Uuid) -> Self {
        self.reviewed_by = Some(value);
        self
    }

    /// Set the approved_at field (optional)
    pub fn approved_at(mut self, value: DateTime<Utc>) -> Self {
        self.approved_at = Some(value);
        self
    }

    /// Set the approved_by field (optional)
    pub fn approved_by(mut self, value: Uuid) -> Self {
        self.approved_by = Some(value);
        self
    }

    /// Set the published_at field (optional)
    pub fn published_at(mut self, value: DateTime<Utc>) -> Self {
        self.published_at = Some(value);
        self
    }

    /// Set the pdf_url field (optional)
    pub fn pdf_url(mut self, value: String) -> Self {
        self.pdf_url = Some(value);
        self
    }

    /// Set the excel_url field (optional)
    pub fn excel_url(mut self, value: String) -> Self {
        self.excel_url = Some(value);
        self
    }

    /// Set the currency field (default: `"IDR".to_string()`)
    pub fn currency(mut self, value: String) -> Self {
        self.currency = Some(value);
        self
    }

    /// Build the FinancialStatement entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<FinancialStatement, String> {
        let company_id = self.company_id.ok_or_else(|| "company_id is required".to_string())?;
        let statement_number = self.statement_number.ok_or_else(|| "statement_number is required".to_string())?;
        let statement_type = self.statement_type.ok_or_else(|| "statement_type is required".to_string())?;
        let name = self.name.ok_or_else(|| "name is required".to_string())?;
        let fiscal_year = self.fiscal_year.ok_or_else(|| "fiscal_year is required".to_string())?;
        let as_of_date = self.as_of_date.ok_or_else(|| "as_of_date is required".to_string())?;

        Ok(FinancialStatement {
            id: Uuid::new_v4(),
            company_id,
            statement_number,
            statement_type,
            name,
            fiscal_period_id: self.fiscal_period_id,
            fiscal_year,
            fiscal_month: self.fiscal_month,
            as_of_date,
            period_start: self.period_start,
            period_end: self.period_end,
            is_comparative: self.is_comparative.unwrap_or(false),
            comparative_period_id: self.comparative_period_id,
            comparative_as_of_date: self.comparative_as_of_date,
            status: self.status.unwrap_or(StatementStatus::default()),
            total_current_assets: self.total_current_assets,
            total_non_current_assets: self.total_non_current_assets,
            total_assets: self.total_assets,
            total_current_liabilities: self.total_current_liabilities,
            total_non_current_liabilities: self.total_non_current_liabilities,
            total_liabilities: self.total_liabilities,
            total_equity: self.total_equity,
            balance_check: self.balance_check.unwrap_or(false),
            balance_difference: self.balance_difference,
            total_revenue: self.total_revenue,
            total_cogs: self.total_cogs,
            gross_profit: self.gross_profit,
            gross_profit_margin: self.gross_profit_margin,
            total_operating_expenses: self.total_operating_expenses,
            operating_income: self.operating_income,
            operating_margin: self.operating_margin,
            total_other_income: self.total_other_income,
            total_other_expenses: self.total_other_expenses,
            income_before_tax: self.income_before_tax,
            tax_expense: self.tax_expense,
            net_income: self.net_income,
            net_profit_margin: self.net_profit_margin,
            cash_from_operations: self.cash_from_operations,
            cash_from_investing: self.cash_from_investing,
            cash_from_financing: self.cash_from_financing,
            net_cash_change: self.net_cash_change,
            beginning_cash: self.beginning_cash,
            ending_cash: self.ending_cash,
            total_debits: self.total_debits,
            total_credits: self.total_credits,
            trial_balance_check: self.trial_balance_check.unwrap_or(false),
            line_items: self.line_items.unwrap_or(serde_json::json!([])),
            comparative_data: self.comparative_data.unwrap_or(serde_json::json!({})),
            variance_data: self.variance_data.unwrap_or(serde_json::json!({})),
            notes: self.notes.unwrap_or(serde_json::json!([])),
            management_notes: self.management_notes,
            generated_at: self.generated_at,
            generated_by: self.generated_by,
            generation_parameters: self.generation_parameters.unwrap_or(serde_json::json!({})),
            reviewed_at: self.reviewed_at,
            reviewed_by: self.reviewed_by,
            approved_at: self.approved_at,
            approved_by: self.approved_by,
            published_at: self.published_at,
            pdf_url: self.pdf_url,
            excel_url: self.excel_url,
            currency: self.currency.unwrap_or("IDR".to_string()),
            metadata: AuditMetadata::default(),
        })
    }
}
