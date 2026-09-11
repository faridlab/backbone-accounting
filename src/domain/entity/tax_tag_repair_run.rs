use chrono::{DateTime, Utc, NaiveDate};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// Strongly-typed ID for TaxTagRepairRun
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TaxTagRepairRunId(pub Uuid);

impl TaxTagRepairRunId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for TaxTagRepairRunId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for TaxTagRepairRunId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for TaxTagRepairRunId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<TaxTagRepairRunId> for Uuid {
    fn from(id: TaxTagRepairRunId) -> Self { id.0 }
}

impl AsRef<Uuid> for TaxTagRepairRunId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for TaxTagRepairRunId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct TaxTagRepairRun {
    pub id: Uuid,
    pub date_from: NaiveDate,
    pub date_to: NaiveDate,
    pub rules: serde_json::Value,
    pub lines_examined: i64,
    pub lines_retagged: i64,
    pub dry_run: bool,
    pub closed_periods_overridden: serde_json::Value,
    pub actor: Option<Uuid>,
    pub reason: String,
    pub ran_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub metadata: serde_json::Value,
}

impl TaxTagRepairRun {
    /// Create a builder for TaxTagRepairRun
    pub fn builder() -> TaxTagRepairRunBuilder {
        <TaxTagRepairRunBuilder as Default>::default()
    }

    /// Create a new TaxTagRepairRun with required fields
    pub fn new(date_from: NaiveDate, date_to: NaiveDate, rules: serde_json::Value, lines_examined: i64, lines_retagged: i64, dry_run: bool, closed_periods_overridden: serde_json::Value, reason: String, ran_at: DateTime<Utc>, metadata: serde_json::Value) -> Self {
        Self {
            id: Uuid::new_v4(),
            date_from,
            date_to,
            rules,
            lines_examined,
            lines_retagged,
            dry_run,
            closed_periods_overridden,
            actor: None,
            reason,
            ran_at,
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
    pub fn typed_id(&self) -> TaxTagRepairRunId {
        TaxTagRepairRunId(self.id)
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

    /// Set the actor field (chainable)
    pub fn with_actor(mut self, value: Uuid) -> Self {
        self.actor = Some(value);
        self
    }

    // ==========================================================
    // Partial Update
    // ==========================================================

    /// Apply partial updates from a map of field name to JSON value
    pub fn apply_patch(&mut self, fields: std::collections::HashMap<String, serde_json::Value>) {
        for (key, value) in fields {
            match key.as_str() {
                "date_from" => {
                    if let Ok(v) = serde_json::from_value(value) { self.date_from = v; }
                }
                "date_to" => {
                    if let Ok(v) = serde_json::from_value(value) { self.date_to = v; }
                }
                "rules" => {
                    if let Ok(v) = serde_json::from_value(value) { self.rules = v; }
                }
                "lines_examined" => {
                    if let Ok(v) = serde_json::from_value(value) { self.lines_examined = v; }
                }
                "lines_retagged" => {
                    if let Ok(v) = serde_json::from_value(value) { self.lines_retagged = v; }
                }
                "dry_run" => {
                    if let Ok(v) = serde_json::from_value(value) { self.dry_run = v; }
                }
                "closed_periods_overridden" => {
                    if let Ok(v) = serde_json::from_value(value) { self.closed_periods_overridden = v; }
                }
                "actor" => {
                    if let Ok(v) = serde_json::from_value(value) { self.actor = v; }
                }
                "reason" => {
                    if let Ok(v) = serde_json::from_value(value) { self.reason = v; }
                }
                "ran_at" => {
                    if let Ok(v) = serde_json::from_value(value) { self.ran_at = v; }
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

impl super::Entity for TaxTagRepairRun {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "TaxTagRepairRun"
    }
}

impl backbone_core::PersistentEntity for TaxTagRepairRun {
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

impl backbone_orm::EntityRepoMeta for TaxTagRepairRun {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &["reason"]
    }
}

/// Builder for TaxTagRepairRun entity
///
/// Provides a fluent API for constructing TaxTagRepairRun instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct TaxTagRepairRunBuilder {
    date_from: Option<NaiveDate>,
    date_to: Option<NaiveDate>,
    rules: Option<serde_json::Value>,
    lines_examined: Option<i64>,
    lines_retagged: Option<i64>,
    dry_run: Option<bool>,
    closed_periods_overridden: Option<serde_json::Value>,
    actor: Option<Uuid>,
    reason: Option<String>,
    ran_at: Option<DateTime<Utc>>,
    metadata: Option<serde_json::Value>,
}

impl TaxTagRepairRunBuilder {
    /// Set the date_from field (required)
    pub fn date_from(mut self, value: NaiveDate) -> Self {
        self.date_from = Some(value);
        self
    }

    /// Set the date_to field (required)
    pub fn date_to(mut self, value: NaiveDate) -> Self {
        self.date_to = Some(value);
        self
    }

    /// Set the rules field (required)
    pub fn rules(mut self, value: serde_json::Value) -> Self {
        self.rules = Some(value);
        self
    }

    /// Set the lines_examined field (required)
    pub fn lines_examined(mut self, value: i64) -> Self {
        self.lines_examined = Some(value);
        self
    }

    /// Set the lines_retagged field (required)
    pub fn lines_retagged(mut self, value: i64) -> Self {
        self.lines_retagged = Some(value);
        self
    }

    /// Set the dry_run field (default: `false`)
    pub fn dry_run(mut self, value: bool) -> Self {
        self.dry_run = Some(value);
        self
    }

    /// Set the closed_periods_overridden field (default: `serde_json::json!([])`)
    pub fn closed_periods_overridden(mut self, value: serde_json::Value) -> Self {
        self.closed_periods_overridden = Some(value);
        self
    }

    /// Set the actor field (optional)
    pub fn actor(mut self, value: Uuid) -> Self {
        self.actor = Some(value);
        self
    }

    /// Set the reason field (required)
    pub fn reason(mut self, value: String) -> Self {
        self.reason = Some(value);
        self
    }

    /// Set the ran_at field (default: `Utc::now()`)
    pub fn ran_at(mut self, value: DateTime<Utc>) -> Self {
        self.ran_at = Some(value);
        self
    }

    /// Set the metadata field (default: `serde_json::json!({})`)
    pub fn metadata(mut self, value: serde_json::Value) -> Self {
        self.metadata = Some(value);
        self
    }

    /// Build the TaxTagRepairRun entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<TaxTagRepairRun, String> {
        let date_from = self.date_from.ok_or_else(|| "date_from is required".to_string())?;
        let date_to = self.date_to.ok_or_else(|| "date_to is required".to_string())?;
        let rules = self.rules.ok_or_else(|| "rules is required".to_string())?;
        let lines_examined = self.lines_examined.ok_or_else(|| "lines_examined is required".to_string())?;
        let lines_retagged = self.lines_retagged.ok_or_else(|| "lines_retagged is required".to_string())?;
        let reason = self.reason.ok_or_else(|| "reason is required".to_string())?;

        Ok(TaxTagRepairRun {
            id: Uuid::new_v4(),
            date_from,
            date_to,
            rules,
            lines_examined,
            lines_retagged,
            dry_run: self.dry_run.unwrap_or(false),
            closed_periods_overridden: self.closed_periods_overridden.unwrap_or(serde_json::json!([])),
            actor: self.actor,
            reason,
            ran_at: self.ran_at.unwrap_or(Utc::now()),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            metadata: self.metadata.unwrap_or(serde_json::json!({})),
        })
    }
}
