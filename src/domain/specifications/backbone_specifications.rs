// Backbone Specifications
// Business rules that can be combined and reused for Backbone validation

use std::collections::HashMap;
use std::fmt;

use crate::domain::entities::Backbone;
use crate::domain::value_objects::{BackboneStatus, BackboneTimestamp};

// Specification Trait
pub trait Specification {
    type Error;

    fn is_satisfied_by(&self, candidate: &Backbone) -> Result<bool, Self::Error>;
    fn and<S>(self, other: S) -> AndSpecification<Self, S>
    where
        Self: Sized,
        S: Specification,
    {
        AndSpecification::new(self, other)
    }

    fn or<S>(self, other: S) -> OrSpecification<Self, S>
    where
        Self: Sized,
        S: Specification,
    {
        OrSpecification::new(self, other)
    }

    fn not(self) -> NotSpecification<Self>
    where
        Self: Sized,
    {
        NotSpecification::new(self)
    }
}

// Specification Result
#[derive(Debug, Clone)]
pub struct SpecificationResult {
    pub satisfied: bool,
    pub specification_name: String,
    pub message: String,
    pub details: HashMap<String, String>,
    pub evaluated_at: BackboneTimestamp,
}

impl SpecificationResult {
    pub fn satisfied(name: String, message: String) -> Self {
        Self {
            satisfied: true,
            specification_name: name,
            message,
            details: HashMap::new(),
            evaluated_at: BackboneTimestamp::now(),
        }
    }

    pub fn unsatisfied(name: String, message: String) -> Self {
        Self {
            satisfied: false,
            specification_name: name,
            message,
            details: HashMap::new(),
            evaluated_at: BackboneTimestamp::now(),
        }
    }

    pub fn with_details(mut self, key: String, value: String) -> Self {
        self.details.insert(key, value);
        self
    }
}

// Composite Specification Operators
#[derive(Debug, Clone)]
pub struct AndSpecification<T, U> {
    left: T,
    right: U,
}

impl<T, U> AndSpecification<T, U> {
    pub fn new(left: T, right: U) -> Self {
        Self { left, right }
    }
}

impl<T, U> Specification for AndSpecification<T, U>
where
    T: Specification,
    U: Specification,
{
    type Error = String;

    fn is_satisfied_by(&self, candidate: &Backbone) -> Result<bool, Self::Error> {
        let left_result = self.left.is_satisfied_by(candidate)
            .map_err(|e| format!("Left specification failed: {}", e))?;
        let right_result = self.right.is_satisfied_by(candidate)
            .map_err(|e| format!("Right specification failed: {}", e))?;

        Ok(left_result && right_result)
    }
}

#[derive(Debug, Clone)]
pub struct OrSpecification<T, U> {
    left: T,
    right: U,
}

impl<T, U> OrSpecification<T, U> {
    pub fn new(left: T, right: U) -> Self {
        Self { left, right }
    }
}

impl<T, U> Specification for OrSpecification<T, U>
where
    T: Specification,
    U: Specification,
{
    type Error = String;

    fn is_satisfied_by(&self, candidate: &Backbone) -> Result<bool, Self::Error> {
        let left_result = self.left.is_satisfied_by(candidate)
            .map_err(|e| format!("Left specification failed: {}", e))?;

        if left_result {
            return Ok(true);
        }

        self.right.is_satisfied_by(candidate)
            .map_err(|e| format!("Right specification failed: {}", e))
    }
}

#[derive(Debug, Clone)]
pub struct NotSpecification<T> {
    spec: T,
}

impl<T> NotSpecification<T> {
    pub fn new(spec: T) -> Self {
        Self { spec }
    }
}

impl<T> Specification for NotSpecification<T>
where
    T: Specification,
{
    type Error = String;

    fn is_satisfied_by(&self, candidate: &Backbone) -> Result<bool, Self::Error> {
        let result = self.spec.is_satisfied_by(candidate)
            .map_err(|e| format!("Inner specification failed: {}", e))?;
        Ok(!result)
    }
}

// Simple Specifications

#[derive(Debug, Clone)]
pub struct BackboneNameMustBeValidSpecification;

impl BackboneNameMustBeValidSpecification {
    pub fn new() -> Self {
        Self
    }
}

impl Specification for BackboneNameMustBeValidSpecification {
    type Error = String;

    fn is_satisfied_by(&self, candidate: &Backbone) -> Result<bool, Self::Error> {
        let name = candidate.name();

        // Name must be between 1 and 100 characters
        if name.is_empty() {
            return Ok(false);
        }

        if name.len() > 100 {
            return Ok(false);
        }

        // Name must contain only alphanumeric characters, spaces, hyphens, and underscores
        let valid_chars = name.chars().all(|c| c.is_alphanumeric() || c.is_whitespace() || c == '-' || c == '_');
        if !valid_chars {
            return Ok(false);
        }

        // Name must not be empty or contain only whitespace
        if name.trim().is_empty() {
            return Ok(false);
        }

        Ok(true)
    }
}

#[derive(Debug, Clone)]
pub struct BackboneStatusMustBeValidSpecification;

impl BackboneStatusMustBeValidSpecification {
    pub fn new() -> Self {
        Self
    }
}

impl Specification for BackboneStatusMustBeValidSpecification {
    type Error = String;

    fn is_satisfied_by(&self, candidate: &Backbone) -> Result<bool, Self::Error> {
        // Status must be one of the defined enum values (this is always true with Rust enums)
        // Status transitions must follow valid state machine
        matches!(
            candidate.status(),
            BackboneStatus::Active | BackboneStatus::Inactive | BackboneStatus::Suspended | BackboneStatus::Archived
        )
    }
}

#[derive(Debug, Clone)]
pub struct BackboneTagsMustBeUniqueSpecification;

impl BackboneTagsMustBeUniqueSpecification {
    pub fn new() -> Self {
        Self
    }
}

impl Specification for BackboneTagsMustBeUniqueSpecification {
    type Error = String;

    fn is_satisfied_by(&self, candidate: &Backbone) -> Result<bool, Self::Error> {
        let tags = candidate.tags();
        let mut seen = std::collections::HashSet::new();

        for tag in tags {
            if seen.contains(tag) {
                return Ok(false); // Duplicate found
            }
            seen.insert(tag);
        }

        // Each tag must be between 1 and 50 characters
        for tag in tags {
            if tag.is_empty() || tag.len() > 50 {
                return Ok(false);
            }
        }

        // Maximum 50 tags per backbone
        if tags.len() > 50 {
            return Ok(false);
        }

        Ok(true)
    }
}

#[derive(Debug, Clone)]
pub struct BackboneMustHaveMetadataSpecification;

impl BackboneMustHaveMetadataSpecification {
    pub fn new() -> Self {
        Self
    }
}

impl Specification for BackboneMustHaveMetadataSpecification {
    type Error = String;

    fn is_satisfied_by(&self, candidate: &Backbone) -> Result<bool, Self::Error> {
        let metadata = candidate.metadata();

        // Metadata must not be empty
        if metadata.is_empty() {
            return Ok(false);
        }

        // All metadata keys must be strings (always true with Rust)
        // All metadata values must be strings (always true with Rust)

        Ok(true)
    }
}

// Composite Specifications

#[derive(Debug, Clone)]
pub struct BackboneIsActiveSpecification;

impl BackboneIsActiveSpecification {
    pub fn new() -> Self {
        Self
    }
}

impl Specification for BackboneIsActiveSpecification {
    type Error = String;

    fn is_satisfied_by(&self, candidate: &Backbone) -> Result<bool, Self::Error> {
        // Backbone status must be ACTIVE
        if !candidate.status().is_active() {
            return Ok(false);
        }

        // Backbone must not be deleted
        if candidate.is_deleted() {
            return Ok(false);
        }

        // Backbone must be valid (combine with other specifications)
        let name_spec = BackboneNameMustBeValidSpecification::new();
        name_spec.is_satisfied_by(candidate)
    }
}

#[derive(Debug, Clone)]
pub struct BackboneCanDeactivateSpecification;

impl BackboneCanDeactivateSpecification {
    pub fn new() -> Self {
        Self
    }
}

impl Specification for BackboneCanDeactivateSpecification {
    type Error = String;

    fn is_satisfied_by(&self, candidate: &Backbone) -> Result<bool, Self::Error> {
        // Backbone must currently be ACTIVE
        if !candidate.status().is_active() {
            return Ok(false);
        }

        // Backbone must not be in SUSPENDED state
        if candidate.status().is_suspended() {
            return Ok(false);
        }

        // Note: Deactivation reason should be provided at the application layer
        Ok(true)
    }
}

#[derive(Debug, Clone)]
pub struct BackboneCanSuspendSpecification;

impl BackboneCanSuspendSpecification {
    pub fn new() -> Self {
        Self
    }
}

impl Specification for BackboneCanSuspendSpecification {
    type Error = String;

    fn is_satisfied_by(&self, candidate: &Backbone) -> Result<bool, Self::Error> {
        // Backbone must be ACTIVE or INACTIVE
        if !candidate.status().is_active() && !candidate.status().is_inactive() {
            return Ok(false);
        }

        // Note: Suspension reason should be provided at the application layer
        // Note: Suspension period should be reasonable (check at application layer)
        Ok(true)
    }
}

#[derive(Debug, Clone)]
pub struct BackboneCanArchiveSpecification;

impl BackboneCanArchiveSpecification {
    pub fn new() -> Self {
        Self
    }
}

impl Specification for BackboneCanArchiveSpecification {
    type Error = String;

    fn is_satisfied_by(&self, candidate: &Backbone) -> Result<bool, Self::Error> {
        // Backbone must be INACTIVE
        if !candidate.status().is_inactive() {
            return Ok(false);
        }

        // Must be inactive for at least 30 days (simplified check - actual implementation would check timestamps)
        let thirty_days_ago = BackboneTimestamp::now().add_days(-30);
        if candidate.updated_at() > &thirty_days_ago {
            return Ok(false);
        }

        // No pending operations (simplified - actual implementation would check operation status)
        Ok(true)
    }
}

// Temporal Specifications

#[derive(Debug, Clone)]
pub struct BackboneMustBeRecentSpecification {
    days: i64,
}

impl BackboneMustBeRecentSpecification {
    pub fn new(days: i64) -> Self {
        Self { days }
    }
}

impl Specification for BackboneMustBeRecentSpecification {
    type Error = String;

    fn is_satisfied_by(&self, candidate: &Backbone) -> Result<bool, Self::Error> {
        let cutoff = BackboneTimestamp::now().add_days(-self.days);
        Ok(candidate.created_at() >= &cutoff)
    }
}

#[derive(Debug, Clone)]
pub struct BackboneMustNotBeOlderThanSpecification {
    max_age_days: i64,
}

impl BackboneMustNotBeOlderThanSpecification {
    pub fn new(max_age_days: i64) -> Self {
        Self { max_age_days }
    }
}

impl Specification for BackboneMustNotBeOlderThanSpecification {
    type Error = String;

    fn is_satisfied_by(&self, candidate: &Backbone) -> Result<bool, Self::Error> {
        let cutoff = BackboneTimestamp::now().add_days(-self.max_age_days);
        Ok(candidate.created_at() >= &cutoff)
    }
}

// Parameterized Specifications

#[derive(Debug, Clone)]
pub struct BackboneTaggedWithSpecification {
    required_tags: Vec<String>,
    match_all: bool,
}

impl BackboneTaggedWithSpecification {
    pub fn new(required_tags: Vec<String>, match_all: bool) -> Self {
        Self {
            required_tags,
            match_all,
        }
    }
}

impl Specification for BackboneTaggedWithSpecification {
    type Error = String;

    fn is_satisfied_by(&self, candidate: &Backbone) -> Result<bool, Self::Error> {
        if self.required_tags.is_empty() {
            return Ok(true);
        }

        let backbone_tags = candidate.tags();

        if self.match_all {
            // All required tags must be present
            Ok(self.required_tags.iter().all(|tag| backbone_tags.contains(tag)))
        } else {
            // At least one required tag must be present
            Ok(self.required_tags.iter().any(|tag| backbone_tags.contains(tag)))
        }
    }
}

#[derive(Debug, Clone)]
pub struct BackboneInDateRangeSpecification {
    start_date: BackboneTimestamp,
    end_date: BackboneTimestamp,
    include_start: bool,
    include_end: bool,
}

impl BackboneInDateRangeSpecification {
    pub fn new(
        start_date: BackboneTimestamp,
        end_date: BackboneTimestamp,
        include_start: bool,
        include_end: bool,
    ) -> Self {
        Self {
            start_date,
            end_date,
            include_start,
            include_end,
        }
    }
}

impl Specification for BackboneInDateRangeSpecification {
    type Error = String;

    fn is_satisfied_by(&self, candidate: &Backbone) -> Result<bool, Self::Error> {
        let created_at = candidate.created_at();

        let after_start = if self.include_start {
            created_at >= &self.start_date
        } else {
            created_at > &self.start_date
        };

        let before_end = if self.include_end {
            created_at <= &self.end_date
        } else {
            created_at < &self.end_date
        };

        Ok(after_start && before_end)
    }
}

#[derive(Debug, Clone)]
pub struct BackboneWithMetadataKeySpecification {
    key: String,
    value: Option<String>,
}

impl BackboneWithMetadataKeySpecification {
    pub fn new(key: String, value: Option<String>) -> Self {
        Self { key, value }
    }
}

impl Specification for BackboneWithMetadataKeySpecification {
    type Error = String;

    fn is_satisfied_by(&self, candidate: &Backbone) -> Result<bool, Self::Error> {
        let metadata = candidate.metadata();

        match &self.value {
            Some(expected_value) => {
                // Check if key exists and has specific value
                metadata.get(&self.key).map_or(false, |v| v == expected_value)
            }
            None => {
                // Just check if key exists
                metadata.contains_key(&self.key)
            }
        }
    }
}

// Specification Evaluator
pub struct SpecificationEvaluator;

impl SpecificationEvaluator {
    pub fn evaluate<S: Specification>(
        specification: &S,
        candidate: &Backbone,
    ) -> Result<SpecificationResult, S::Error> {
        let satisfied = specification.is_satisfied_by(candidate)?;
        let spec_name = std::any::type_name::<S>().split("::").last().unwrap_or("Unknown");

        let result = if satisfied {
            SpecificationResult::satisfied(
                spec_name.to_string(),
                format!("Specification '{}' is satisfied", spec_name),
            )
        } else {
            SpecificationResult::unsatisfied(
                spec_name.to_string(),
                format!("Specification '{}' is not satisfied", spec_name),
            )
        };

        Ok(result)
    }

    pub fn evaluate_batch<S: Specification>(
        specification: &S,
        candidates: &[Backbone],
    ) -> Vec<Result<SpecificationResult, S::Error>> {
        candidates
            .iter()
            .map(|candidate| Self::evaluate(specification, candidate))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::value_objects::{BackboneName, Metadata};

    fn create_test_backbone() -> Backbone {
        Backbone::create(
            BackboneName::new("Test Backbone").unwrap(),
            "Test Description".to_string(),
            vec!["test".to_string(), "production".to_string()],
            {
                let mut metadata = Metadata::new();
                metadata.insert("env".to_string(), "production".to_string()).unwrap();
                metadata
            },
            "test_user".to_string(),
        ).unwrap()
    }

    #[test]
    fn test_backbone_name_specification() {
        let spec = BackboneNameMustBeValidSpecification::new();
        let valid_backbone = create_test_backbone();

        assert!(spec.is_satisfied_by(&valid_backbone).unwrap());

        // Test with invalid name (empty)
        let invalid_backbone = Backbone::create(
            BackboneName::new("").unwrap(), // This would normally fail at creation
            "Test".to_string(),
            vec![],
            Metadata::new(),
            "user".to_string(),
        ).unwrap();

        // This test is conceptual - in practice, name validation happens at creation
    }

    #[test]
    fn test_tags_unique_specification() {
        let spec = BackboneTagsMustBeUniqueSpecification::new();
        let valid_backbone = create_test_backbone();

        assert!(spec.is_satisfied_by(&valid_backbone).unwrap());
    }

    #[test]
    fn test_is_active_specification() {
        let spec = BackboneIsActiveSpecification::new();
        let active_backbone = create_test_backbone();

        assert!(spec.is_satisfied_by(&active_backbone).unwrap());

        // Create inactive backbone
        let mut inactive_backbone = create_test_backbone();
        // Note: In a real implementation, you'd need to be able to change status
        // This is just for testing the specification logic
    }

    #[test]
    fn test_tagged_with_specification() {
        let spec_match_all = BackboneTaggedWithSpecification::new(
            vec!["test".to_string(), "production".to_string()],
            true,
        );

        let spec_match_any = BackboneTaggedWithSpecification::new(
            vec!["test".to_string(), "nonexistent".to_string()],
            false,
        );

        let backbone = create_test_backbone();

        assert!(spec_match_all.is_satisfied_by(&backbone).unwrap());
        assert!(spec_match_any.is_satisfied_by(&backbone).unwrap());
    }

    #[test]
    fn test_composite_specifications() {
        let name_spec = BackboneNameMustBeValidSpecification::new();
        let tags_spec = BackboneTagsMustBeUniqueSpecification::new();

        let combined_and = name_spec.and(tags_spec);
        let backbone = create_test_backbone();

        assert!(combined_and.is_satisfied_by(&backbone).unwrap());
    }

    #[test]
    fn test_not_specification() {
        let active_spec = BackboneIsActiveSpecification::new();
        let not_active = active_spec.not();

        // Create an inactive backbone (conceptual test)
        let active_backbone = create_test_backbone();
        assert!(active_backbone.status().is_active());

        // The not specification should return false for an active backbone
        assert!(!not_active.is_satisfied_by(&active_backbone).unwrap());
    }

    #[test]
    fn test_specification_evaluator() {
        let spec = BackboneNameMustBeValidSpecification::new();
        let backbone = create_test_backbone();

        let result = SpecificationEvaluator::evaluate(&spec, &backbone).unwrap();
        assert!(result.satisfied);
        assert!(result.specification_name.contains("BackboneNameMustBeValidSpecification"));
    }

    #[test]
    fn test_metadata_specification() {
        let spec_has_key = BackboneWithMetadataKeySpecification::new(
            "env".to_string(),
            None,
        );

        let spec_has_key_value = BackboneWithMetadataKeySpecification::new(
            "env".to_string(),
            Some("production".to_string()),
        );

        let spec_wrong_value = BackboneWithMetadataKeySpecification::new(
            "env".to_string(),
            Some("development".to_string()),
        );

        let backbone = create_test_backbone();

        assert!(spec_has_key.is_satisfied_by(&backbone).unwrap());
        assert!(spec_has_key_value.is_satisfied_by(&backbone).unwrap());
        assert!(!spec_wrong_value.is_satisfied_by(&backbone).unwrap());
    }

    #[test]
    fn test_temporal_specifications() {
        let recent_spec = BackboneMustBeRecentSpecification::new(30);
        let backbone = create_test_backbone();

        assert!(recent_spec.is_satisfied_by(&backbone).unwrap());

        let old_spec = BackboneMustNotBeOlderThanSpecification::new(1);
        assert!(old_spec.is_satisfied_by(&backbone).unwrap());
    }
}