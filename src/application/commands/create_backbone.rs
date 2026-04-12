// Create Backbone Command
// Command handler for creating Backbone entities

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::domain::entities::Backbone;
use crate::domain::repositories::BackboneRepository;
use crate::domain::services::BackboneValidationService;
use crate::domain::value_objects::{BackboneName, Metadata};
use crate::domain::{DomainError, DomainResult};

// Command DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateBackboneCommand {
    pub name: String,
    pub description: String,
    pub tags: Vec<String>,
    pub metadata: HashMap<String, String>,
    pub created_by: String,
}

impl CreateBackboneCommand {
    pub fn new(
        name: String,
        description: String,
        tags: Vec<String>,
        metadata: HashMap<String, String>,
        created_by: String,
    ) -> Self {
        Self {
            name,
            description,
            tags,
            metadata,
            created_by,
        }
    }

    pub fn validate(&self) -> DomainResult<()> {
        if self.name.trim().is_empty() {
            return Err(DomainError::ValidationError {
                message: "Name cannot be empty".to_string(),
            });
        }

        if self.name.len() > 100 {
            return Err(DomainError::ValidationError {
                message: "Name cannot exceed 100 characters".to_string(),
            });
        }

        if self.created_by.trim().is_empty() {
            return Err(DomainError::ValidationError {
                message: "Created by cannot be empty".to_string(),
            });
        }

        // Validate tags
        if self.tags.len() > 50 {
            return Err(DomainError::ValidationError {
                message: "Cannot have more than 50 tags".to_string(),
            });
        }

        for tag in &self.tags {
            if tag.trim().is_empty() {
                return Err(DomainError::ValidationError {
                    message: "Tags cannot be empty".to_string(),
                });
            }
            if tag.len() > 50 {
                return Err(DomainError::ValidationError {
                    message: "Tag cannot exceed 50 characters".to_string(),
                });
            }
        }

        // Validate metadata keys and values
        for (key, value) in &self.metadata {
            if key.is_empty() {
                return Err(DomainError::ValidationError {
                    message: "Metadata keys cannot be empty".to_string(),
                });
            }
            if key.len() > 50 {
                return Err(DomainError::ValidationError {
                    message: "Metadata key cannot exceed 50 characters".to_string(),
                });
            }
            if value.len() > 500 {
                return Err(DomainError::ValidationError {
                    message: "Metadata value cannot exceed 500 characters".to_string(),
                });
            }
        }

        Ok(())
    }
}

// Command Response DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateBackboneResponse {
    pub success: bool,
    pub backbone_id: Option<String>,
    pub message: String,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    pub warnings: Vec<String>,
}

impl CreateBackboneResponse {
    pub fn success(backbone_id: String, created_at: chrono::DateTime<chrono::Utc>) -> Self {
        Self {
            success: true,
            backbone_id: Some(backbone_id),
            message: "Backbone created successfully".to_string(),
            created_at: Some(created_at),
            warnings: Vec::new(),
        }
    }

    pub fn success_with_warnings(
        backbone_id: String,
        created_at: chrono::DateTime<chrono::Utc>,
        warnings: Vec<String>,
    ) -> Self {
        Self {
            success: true,
            backbone_id: Some(backbone_id),
            message: "Backbone created successfully with warnings".to_string(),
            created_at: Some(created_at),
            warnings,
        }
    }

    pub fn error(message: String) -> Self {
        Self {
            success: false,
            backbone_id: None,
            message,
            created_at: None,
            warnings: Vec::new(),
        }
    }
}

// Command Handler Trait
#[async_trait]
pub trait CreateBackboneHandler: Send + Sync {
    async fn handle(&self, command: CreateBackboneCommand) -> DomainResult<CreateBackboneResponse>;
}

// Default Command Handler Implementation
pub struct DefaultCreateBackboneHandler {
    repository: Box<dyn BackboneRepository>,
    validation_service: Option<Box<dyn BackboneValidationService>>,
}

impl DefaultCreateBackboneHandler {
    pub fn new(repository: Box<dyn BackboneRepository>) -> Self {
        Self {
            repository,
            validation_service: None,
        }
    }

    pub fn with_validation_service(
        mut self,
        validation_service: Box<dyn BackboneValidationService>,
    ) -> Self {
        self.validation_service = Some(validation_service);
        self
    }

    async fn check_name_uniqueness(&self, name: &str) -> DomainResult<bool> {
        // Note: In a real implementation, you might want to check if a name is unique
        // For now, we'll assume names don't need to be unique at the repository level
        Ok(true)
    }

    async fn validate_business_rules(
        &self,
        backbone: &Backbone,
    ) -> DomainResult<Vec<String>> {
        let mut warnings = Vec::new();

        // Check if validation service is available
        if let Some(validation_service) = &self.validation_service {
            match validation_service
                .check_business_rules(backbone, "create")
                .await
            {
                Ok(report) => {
                    if !report.allowed {
                        return Err(DomainError::BusinessRuleViolation {
                            message: report.recommendation,
                        });
                    }

                    // Add any warnings from violated rules
                    for rule in &report.violated_rules {
                        if matches!(rule.severity, crate::domain::services::RuleSeverity::Low | crate::domain::services::RuleSeverity::Info) {
                            warnings.push(format!("{}: {}", rule.name, rule.message));
                        }
                    }
                }
                Err(e) => {
                    // Log error but don't fail creation if validation service fails
                    eprintln!("Validation service error: {:?}", e);
                }
            }
        }

        // Business rule checks
        if backbone.tags().is_empty() {
            warnings.push("No tags provided - consider adding tags for better organization".to_string());
        }

        if backbone.metadata().is_empty() {
            warnings.push("No metadata provided - consider adding metadata for better context".to_string());
        }

        if backbone.description().trim().is_empty() {
            warnings.push("No description provided - consider adding a description for better documentation".to_string());
        }

        Ok(warnings)
    }
}

#[async_trait]
impl CreateBackboneHandler for DefaultCreateBackboneHandler {
    async fn handle(&self, command: CreateBackboneCommand) -> DomainResult<CreateBackboneResponse> {
        // Validate command
        command.validate()?;

        // Check name uniqueness (optional business rule)
        if !self.check_name_uniqueness(&command.name).await? {
            return Err(DomainError::BusinessRuleViolation {
                message: "A Backbone with this name already exists".to_string(),
            });
        }

        // Create value objects
        let backbone_name = BackboneName::new(&command.name)
            .map_err(|e| DomainError::ValidationError { message: e.to_string() })?;

        let metadata = if command.metadata.is_empty() {
            Metadata::new()
        } else {
            Metadata::from_map(command.metadata.clone())
                .map_err(|e| DomainError::ValidationError { message: e.to_string() })?
        };

        // Create the Backbone aggregate
        let mut backbone = Backbone::create(
            backbone_name,
            command.description,
            command.tags.clone(),
            metadata,
            command.created_by,
        ).map_err(|e| DomainError::ValidationError { message: e.to_string() })?;

        // Validate business rules
        let warnings = self.validate_business_rules(&backbone).await?;

        // Additional validation if service is available
        if let Some(validation_service) = &self.validation_service {
            match validation_service.validate_backbone_integrity(&backbone).await {
                Ok(report) => {
                    if !report.valid {
                        return Err(DomainError::ValidationError {
                            message: format!("Backbone validation failed: {}", report.summary),
                        });
                    }
                }
                Err(e) => {
                    eprintln!("Validation service error during creation: {:?}", e);
                }
            }
        }

        // Save to repository
        self.repository.save(&backbone).await
            .map_err(|e| DomainError::from(e))?;

        // Get domain events and publish them (in a real implementation, you'd have an event bus)
        let events = backbone.pending_events();
        for event in events {
            // TODO: Publish events to event bus
            println!("Publishing event: {:?}", event.event_type());
        }
        backbone.mark_events_as_committed();

        // Return response
        let backbone_id = backbone.id().value().to_string();
        let created_at = *backbone.created_at();

        if warnings.is_empty() {
            Ok(CreateBackboneResponse::success(backbone_id, created_at))
        } else {
            Ok(CreateBackboneResponse::success_with_warnings(
                backbone_id,
                created_at,
                warnings,
            ))
        }
    }
}

// Command Handler Factory
pub struct CreateBackboneHandlerFactory;

impl CreateBackboneHandlerFactory {
    pub fn create_handler(
        repository: Box<dyn BackboneRepository>,
        validation_service: Option<Box<dyn BackboneValidationService>>,
    ) -> Box<dyn CreateBackboneHandler> {
        let handler = DefaultCreateBackboneHandler::new(repository);

        if let Some(vs) = validation_service {
            Box::new(handler.with_validation_service(vs))
        } else {
            Box::new(handler)
        }
    }

    pub fn create_simple_handler(
        repository: Box<dyn BackboneRepository>,
    ) -> Box<dyn CreateBackboneHandler> {
        Box::new(DefaultCreateBackboneHandler::new(repository))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::value_objects::{BackboneId, BackboneStatus};
    use async_trait::async_trait;
    use std::collections::HashMap;

    // Mock repository for testing
    struct MockRepository {
        should_fail: bool,
        name_exists: bool,
    }

    impl MockRepository {
        fn new() -> Self {
            Self {
                should_fail: false,
                name_exists: false,
            }
        }

        fn with_failure(mut self) -> Self {
            self.should_fail = true;
            self
        }

        fn with_name_exists(mut self) -> Self {
            self.name_exists = true;
            self
        }
    }

    #[async_trait]
    impl BackboneRepository for MockRepository {
        async fn save(&self, _backbone: &Backbone) -> crate::domain::repositories::RepositoryResult<()> {
            if self.should_fail {
                Err(crate::domain::repositories::RepositoryError::DatabaseError {
                    message: "Database error".to_string(),
                })
            } else {
                Ok(())
            }
        }

        async fn find_by_id(&self, _id: &BackboneId) -> crate::domain::repositories::RepositoryResult<Option<Backbone>> {
            Ok(None)
        }

        async fn delete(&self, _id: &BackboneId, _hard_delete: bool) -> crate::domain::repositories::RepositoryResult<()> {
            Ok(())
        }

        async fn find_all(
            &self,
            _pagination: crate::domain::repositories::PaginationParams,
            _sort: crate::domain::repositories::SortParams,
        ) -> crate::domain::repositories::RepositoryResult<crate::domain::repositories::PaginatedResult<Backbone>> {
            Ok(crate::domain::repositories::PaginatedResult::empty(1, 20))
        }

        async fn find_with_filters(
            &self,
            _filters: crate::domain::repositories::BackboneFilters,
            _pagination: crate::domain::repositories::PaginationParams,
            _sort: crate::domain::repositories::SortParams,
        ) -> crate::domain::repositories::RepositoryResult<crate::domain::repositories::PaginatedResult<Backbone>> {
            Ok(crate::domain::repositories::PaginatedResult::empty(1, 20))
        }

        async fn find_by_status(
            &self,
            _status: BackboneStatus,
            _pagination: crate::domain::repositories::PaginationParams,
            _sort: crate::domain::repositories::SortParams,
        ) -> crate::domain::repositories::RepositoryResult<crate::domain::repositories::PaginatedResult<Backbone>> {
            Ok(crate::domain::repositories::PaginatedResult::empty(1, 20))
        }

        async fn find_by_tags(
            &self,
            _tags: Vec<String>,
            _pagination: crate::domain::repositories::PaginationParams,
            _sort: crate::domain::repositories::SortParams,
        ) -> crate::domain::repositories::RepositoryResult<crate::domain::repositories::PaginatedResult<Backbone>> {
            Ok(crate::domain::repositories::PaginatedResult::empty(1, 20))
        }

        async fn find_by_created_by(
            &self,
            _created_by: &str,
            _pagination: crate::domain::repositories::PaginationParams,
            _sort: crate::domain::repositories::SortParams,
        ) -> crate::domain::repositories::RepositoryResult<crate::domain::repositories::PaginatedResult<Backbone>> {
            Ok(crate::domain::repositories::PaginatedResult::empty(1, 20))
        }

        async fn search(
            &self,
            _query: &str,
            _pagination: crate::domain::repositories::PaginationParams,
            _sort: crate::domain::repositories::SortParams,
        ) -> crate::domain::repositories::RepositoryResult<crate::domain::repositories::PaginatedResult<Backbone>> {
            Ok(crate::domain::repositories::PaginatedResult::empty(1, 20))
        }

        async fn save_batch(&self, _backbones: &[Backbone]) -> crate::domain::repositories::RepositoryResult<()> {
            Ok(())
        }

        async fn delete_batch(&self, _ids: &[BackboneId], _hard_delete: bool) -> crate::domain::repositories::RepositoryResult<()> {
            Ok(())
        }

        async fn exists(&self, _id: &BackboneId) -> crate::domain::repositories::RepositoryResult<bool> {
            Ok(false)
        }

        async fn count(&self, _filters: Option<crate::domain::repositories::BackboneFilters>) -> crate::domain::repositories::RepositoryResult<u64> {
            Ok(0)
        }

        async fn find_deleted(
            &self,
            _pagination: crate::domain::repositories::PaginationParams,
            _sort: crate::domain::repositories::SortParams,
        ) -> crate::domain::repositories::RepositoryResult<crate::domain::repositories::PaginatedResult<Backbone>> {
            Ok(crate::domain::repositories::PaginatedResult::empty(1, 20))
        }

        async fn restore(&self, _id: &BackboneId) -> crate::domain::repositories::RepositoryResult<()> {
            Ok(())
        }

        async fn find_by_metadata(
            &self,
            _metadata_key: &str,
            _metadata_value: Option<&str>,
            _pagination: crate::domain::repositories::PaginationParams,
            _sort: crate::domain::repositories::SortParams,
        ) -> crate::domain::repositories::RepositoryResult<crate::domain::repositories::PaginatedResult<Backbone>> {
            Ok(crate::domain::repositories::PaginatedResult::empty(1, 20))
        }

        async fn find_by_date_range(
            &self,
            _start_date: chrono::DateTime<chrono::Utc>,
            _end_date: chrono::DateTime<chrono::Utc>,
            _date_field: crate::domain::repositories::SortField,
            _pagination: crate::domain::repositories::PaginationParams,
            _sort: crate::domain::repositories::SortParams,
        ) -> crate::domain::repositories::RepositoryResult<crate::domain::repositories::PaginatedResult<Backbone>> {
            Ok(crate::domain::repositories::PaginatedResult::empty(1, 20))
        }

        async fn get_status_counts(&self) -> crate::domain::repositories::RepositoryResult<std::collections::HashMap<BackboneStatus, u64>> {
            Ok(std::collections::HashMap::new())
        }

        async fn get_tag_counts(&self) -> crate::domain::repositories::RepositoryResult<std::collections::HashMap<String, u64>> {
            Ok(std::collections::HashMap::new())
        }

        async fn get_recently_created(&self, _days: i64, _limit: Option<usize>) -> crate::domain::repositories::RepositoryResult<Vec<Backbone>> {
            Ok(Vec::new())
        }

        async fn health_check(&self) -> crate::domain::repositories::RepositoryResult<bool> {
            Ok(true)
        }

        async fn connection_pool_status(&self) -> crate::domain::repositories::RepositoryResult<std::collections::HashMap<String, serde_json::Value>> {
            Ok(std::collections::HashMap::new())
        }
    }

    // Mock validation service
    struct MockValidationService {
        should_fail: bool,
    }

    impl MockValidationService {
        fn new() -> Self {
            Self { should_fail: false }
        }

        fn with_failure(mut self) -> Self {
            self.should_fail = true;
            self
        }
    }

    #[async_trait]
    impl BackboneValidationService for MockValidationService {
        async fn validate_backbone_integrity(&self, _backbone: &Backbone) -> crate::domain::services::ServiceResult<crate::domain::services::ValidationReport> {
            if self.should_fail {
                Err(crate::domain::services::ServiceError::ValidationError {
                    message: "Validation failed".to_string(),
                })
            } else {
                Ok(crate::domain::services::ValidationReport::new(BackboneId::generate()))
            }
        }

        async fn check_business_rules(&self, _backbone: &Backbone, _operation: &str) -> crate::domain::services::ServiceResult<crate::domain::services::BusinessRuleReport> {
            Ok(crate::domain::services::BusinessRuleReport::new())
        }

        async fn validate_backbone_configuration(&self, _backbone: &Backbone) -> crate::domain::services::ServiceResult<crate::domain::services::ConfigurationReport> {
            Ok(crate::domain::services::ConfigurationReport::new("test".to_string()))
        }
    }

    #[tokio::test]
    async fn test_create_backbone_command_validation() {
        // Valid command
        let valid_command = CreateBackboneCommand::new(
            "Test Backbone".to_string(),
            "Test Description".to_string(),
            vec!["test".to_string()],
            HashMap::new(),
            "test_user".to_string(),
        );

        assert!(valid_command.validate().is_ok());

        // Invalid command - empty name
        let invalid_command = CreateBackboneCommand::new(
            "".to_string(),
            "Test Description".to_string(),
            vec![],
            HashMap::new(),
            "test_user".to_string(),
        );

        assert!(invalid_command.validate().is_err());

        // Invalid command - name too long
        let long_name = "a".repeat(101);
        let invalid_command = CreateBackboneCommand::new(
            long_name,
            "Test Description".to_string(),
            vec![],
            HashMap::new(),
            "test_user".to_string(),
        );

        assert!(invalid_command.validate().is_err());
    }

    #[tokio::test]
    async fn test_create_backbone_handler_success() {
        let repository = Box::new(MockRepository::new());
        let handler = DefaultCreateBackboneHandler::new(repository);

        let command = CreateBackboneCommand::new(
            "Test Backbone".to_string(),
            "Test Description".to_string(),
            vec!["test".to_string()],
            HashMap::new(),
            "test_user".to_string(),
        );

        let result = handler.handle(command).await;
        assert!(result.is_ok());

        let response = result.unwrap();
        assert!(response.success);
        assert!(response.backbone_id.is_some());
        assert!(response.created_at.is_some());
    }

    #[tokio::test]
    async fn test_create_backbone_handler_with_warnings() {
        let repository = Box::new(MockRepository::new());
        let handler = DefaultCreateBackboneHandler::new(repository);

        let command = CreateBackboneCommand::new(
            "Test Backbone".to_string(),
            "".to_string(), // Empty description should trigger warning
            vec![], // Empty tags should trigger warning
            HashMap::new(), // Empty metadata should trigger warning
            "test_user".to_string(),
        );

        let result = handler.handle(command).await;
        assert!(result.is_ok());

        let response = result.unwrap();
        assert!(response.success);
        assert!(!response.warnings.is_empty());
        assert!(response.warnings.len() >= 3); // At least 3 warnings
    }

    #[tokio::test]
    async fn test_create_backbone_handler_repository_error() {
        let repository = Box::new(MockRepository::new().with_failure());
        let handler = DefaultCreateBackboneHandler::new(repository);

        let command = CreateBackboneCommand::new(
            "Test Backbone".to_string(),
            "Test Description".to_string(),
            vec![],
            HashMap::new(),
            "test_user".to_string(),
        );

        let result = handler.handle(command).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_create_backbone_handler_with_validation_service() {
        let repository = Box::new(MockRepository::new());
        let validation_service = Box::new(MockValidationService::new());
        let handler = DefaultCreateBackboneHandler::new(repository)
            .with_validation_service(validation_service);

        let command = CreateBackboneCommand::new(
            "Test Backbone".to_string(),
            "Test Description".to_string(),
            vec!["test".to_string()],
            {
                let mut metadata = HashMap::new();
                metadata.insert("env".to_string(), "test".to_string());
                metadata
            },
            "test_user".to_string(),
        );

        let result = handler.handle(command).await;
        assert!(result.is_ok());

        let response = result.unwrap();
        assert!(response.success);
    }
}