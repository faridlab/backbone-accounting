// Backbone Repository Trait
// Repository interface for Backbone aggregate persistence

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::collections::HashMap;

use crate::domain::entities::Backbone;
use crate::domain::value_objects::{BackboneId, BackboneStatus};

// Repository Result Type
pub type RepositoryResult<T> = Result<T, RepositoryError>;

// Repository Error Types
#[derive(Debug, thiserror::Error)]
pub enum RepositoryError {
    #[error("Backbone not found: {id}")]
    NotFound { id: String },

    #[error("Backbone already exists: {id}")]
    AlreadyExists { id: String },

    #[error("Database connection error: {message}")]
    DatabaseError { message: String },

    #[error("Validation error: {message}")]
    ValidationError { message: String },

    #[error("Concurrency conflict: Backbone has been modified")]
    ConcurrencyConflict,

    #[error("Unknown repository error: {message}")]
    Unknown { message: String },
}

// Pagination Parameters
#[derive(Debug, Clone)]
pub struct PaginationParams {
    pub page: usize,
    pub page_size: usize,
}

impl PaginationParams {
    pub fn new(page: usize, page_size: usize) -> Self {
        Self {
            page: page.max(1) - 1, // Convert to 0-based
            page_size: page_size.min(100).max(1).clamp(1, 100),
        }
    }

    pub fn offset(&self) -> usize {
        self.page * self.page_size
    }

    pub fn limit(&self) -> usize {
        self.page_size
    }
}

impl Default for PaginationParams {
    fn default() -> Self {
        Self::new(1, 20)
    }
}

// Filter Parameters
#[derive(Debug, Clone, Default)]
pub struct BackboneFilters {
    pub status: Option<BackboneStatus>,
    pub tags: Option<Vec<String>>,
    pub created_after: Option<DateTime<Utc>>,
    pub created_before: Option<DateTime<Utc>>,
    pub updated_after: Option<DateTime<Utc>>,
    pub updated_before: Option<DateTime<Utc>>,
    pub search_query: Option<String>,
    pub created_by: Option<String>,
    pub metadata: Option<HashMap<String, String>>,
}

impl BackboneFilters {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_status(mut self, status: BackboneStatus) -> Self {
        self.status = Some(status);
        self
    }

    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = Some(tags);
        self
    }

    pub fn with_search(mut self, query: String) -> Self {
        self.search_query = Some(query);
        self
    }

    pub fn with_date_range(
        mut self,
        created_after: Option<DateTime<Utc>>,
        created_before: Option<DateTime<Utc>>,
    ) -> Self {
        self.created_after = created_after;
        self.created_before = created_before;
        self
    }

    pub fn with_metadata(mut self, metadata: HashMap<String, String>) -> Self {
        self.metadata = Some(metadata);
        self
    }

    pub fn has_filters(&self) -> bool {
        self.status.is_some()
            || self.tags.as_ref().map_or(false, |t| !t.is_empty())
            || self.created_after.is_some()
            || self.created_before.is_some()
            || self.updated_after.is_some()
            || self.updated_before.is_some()
            || self.search_query.as_ref().map_or(false, |q| !q.is_empty())
            || self.created_by.as_ref().map_or(false, |u| !u.is_empty())
            || self.metadata.as_ref().map_or(false, |m| !m.is_empty())
    }
}

// Sort Parameters
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortField {
    Id,
    Name,
    Status,
    CreatedAt,
    UpdatedAt,
    CreatedBy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDirection {
    Ascending,
    Descending,
}

#[derive(Debug, Clone)]
pub struct SortParams {
    pub field: SortField,
    pub direction: SortDirection,
}

impl SortParams {
    pub fn new(field: SortField, direction: SortDirection) -> Self {
        Self { field, direction }
    }

    pub fn ascending(field: SortField) -> Self {
        Self::new(field, SortDirection::Ascending)
    }

    pub fn descending(field: SortField) -> Self {
        Self::new(field, SortDirection::Descending)
    }
}

impl Default for SortParams {
    fn default() -> Self {
        Self::ascending(SortField::CreatedAt)
    }
}

// Paginated Result
#[derive(Debug, Clone)]
pub struct PaginatedResult<T> {
    pub items: Vec<T>,
    pub total: u64,
    pub page: usize,
    pub page_size: usize,
    pub total_pages: usize,
    pub has_next: bool,
    pub has_previous: bool,
}

impl<T> PaginatedResult<T> {
    pub fn new(
        items: Vec<T>,
        total: u64,
        page: usize,
        page_size: usize,
    ) -> Self {
        let total_pages = ((total as f64) / (page_size as f64)).ceil() as usize;
        let has_next = page < total_pages;
        let has_previous = page > 1;

        Self {
            items,
            total,
            page,
            page_size,
            total_pages,
            has_next,
            has_previous,
        }
    }

    pub fn empty(page: usize, page_size: usize) -> Self {
        Self::new(Vec::new(), 0, page, page_size)
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }
}

// Repository Trait for Backbone aggregate
#[async_trait]
pub trait BackboneRepository: Send + Sync {
    // Basic CRUD operations
    async fn save(&self, backbone: &Backbone) -> RepositoryResult<()>;
    async fn find_by_id(&self, id: &BackboneId) -> RepositoryResult<Option<Backbone>>;
    async fn delete(&self, id: &BackboneId, hard_delete: bool) -> RepositoryResult<()>;

    // Query operations
    async fn find_all(
        &self,
        pagination: PaginationParams,
        sort: SortParams,
    ) -> RepositoryResult<PaginatedResult<Backbone>>;

    async fn find_with_filters(
        &self,
        filters: BackboneFilters,
        pagination: PaginationParams,
        sort: SortParams,
    ) -> RepositoryResult<PaginatedResult<Backbone>>;

    async fn find_by_status(
        &self,
        status: BackboneStatus,
        pagination: PaginationParams,
        sort: SortParams,
    ) -> RepositoryResult<PaginatedResult<Backbone>>;

    async fn find_by_tags(
        &self,
        tags: Vec<String>,
        pagination: PaginationParams,
        sort: SortParams,
    ) -> RepositoryResult<PaginatedResult<Backbone>>;

    async fn find_by_created_by(
        &self,
        created_by: &str,
        pagination: PaginationParams,
        sort: SortParams,
    ) -> RepositoryResult<PaginatedResult<Backbone>>;

    async fn search(
        &self,
        query: &str,
        pagination: PaginationParams,
        sort: SortParams,
    ) -> RepositoryResult<PaginatedResult<Backbone>>;

    // Bulk operations
    async fn save_batch(&self, backbones: &[Backbone]) -> RepositoryResult<()>;
    async fn delete_batch(&self, ids: &[BackboneId], hard_delete: bool) -> RepositoryResult<()>;

    // Existence checks
    async fn exists(&self, id: &BackboneId) -> RepositoryResult<bool>;
    async fn count(&self, filters: Option<BackboneFilters>) -> RepositoryResult<u64>;

    // Soft delete operations
    async fn find_deleted(
        &self,
        pagination: PaginationParams,
        sort: SortParams,
    ) -> RepositoryResult<PaginatedResult<Backbone>>;

    async fn restore(&self, id: &BackboneId) -> RepositoryResult<()>;

    // Metadata operations
    async fn find_by_metadata(
        &self,
        metadata_key: &str,
        metadata_value: Option<&str>,
        pagination: PaginationParams,
        sort: SortParams,
    ) -> RepositoryResult<PaginatedResult<Backbone>>;

    // Date range queries
    async fn find_by_date_range(
        &self,
        start_date: DateTime<Utc>,
        end_date: DateTime<Utc>,
        date_field: SortField, // CreatedAt or UpdatedAt
        pagination: PaginationParams,
        sort: SortParams,
    ) -> RepositoryResult<PaginatedResult<Backbone>>;

    // Statistics
    async fn get_status_counts(&self) -> RepositoryResult<HashMap<BackboneStatus, u64>>;
    async fn get_tag_counts(&self) -> RepositoryResult<HashMap<String, u64>>;
    async fn get_recently_created(
        &self,
        days: i64,
        limit: Option<usize>,
    ) -> RepositoryResult<Vec<Backbone>>;

    // Health and connection
    async fn health_check(&self) -> RepositoryResult<bool>;
    async fn connection_pool_status(&self) -> RepositoryResult<HashMap<String, serde_json::Value>>;
}

// Transaction Trait for operations that need to be atomic
#[async_trait]
pub trait BackboneTransaction: Send + Sync {
    async fn begin(&mut self) -> RepositoryResult<()>;
    async fn commit(&mut self) -> RepositoryResult<()>;
    async fn rollback(&mut self) -> RepositoryResult<()>;
    async fn save(&mut self, backbone: &Backbone) -> RepositoryResult<()>;
    async fn delete(&mut self, id: &BackboneId, hard_delete: bool) -> RepositoryResult<()>;
    async fn save_batch(&mut self, backbones: &[Backbone]) -> RepositoryResult<()>;
    async fn delete_batch(&mut self, ids: &[BackboneId], hard_delete: bool) -> RepositoryResult<()>;
}

// Repository Factory for creating repository instances
#[async_trait]
pub trait BackboneRepositoryFactory: Send + Sync {
    type Repository: BackboneRepository;
    type Transaction: BackboneTransaction;

    async fn create_repository(&self) -> RepositoryResult<Self::Repository>;
    async fn create_transaction(&self) -> RepositoryResult<Self::Transaction>;
}

// Cache-aware Repository (optional extension)
#[async_trait]
pub trait CacheableRepository: Send + Sync {
    async fn invalidate_cache(&self, id: &BackboneId) -> RepositoryResult<()>;
    async fn invalidate_cache_by_pattern(&self, pattern: &str) -> RepositoryResult<()>;
    async fn warm_cache(&self, ids: &[BackboneId]) -> RepositoryResult<()>;
}

// Audit Trail Repository (optional extension)
#[async_trait]
pub trait AuditableRepository: Send + Sync {
    async fn get_audit_history(
        &self,
        id: &BackboneId,
        pagination: PaginationParams,
    ) -> RepositoryResult<PaginatedResult<AuditEntry>>;

    async fn add_audit_entry(&self, entry: AuditEntry) -> RepositoryResult<()>;
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AuditEntry {
    pub id: String,
    pub backbone_id: String,
    pub action: String,
    pub previous_state: Option<serde_json::Value>,
    pub new_state: Option<serde_json::Value>,
    pub changed_by: String,
    pub timestamp: DateTime<Utc>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub metadata: Option<HashMap<String, String>>,
}

impl AuditEntry {
    pub fn new(
        backbone_id: String,
        action: String,
        changed_by: String,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            backbone_id,
            action,
            previous_state: None,
            new_state: None,
            changed_by,
            timestamp: Utc::now(),
            ip_address: None,
            user_agent: None,
            metadata: None,
        }
    }

    pub fn with_states(
        mut self,
        previous_state: Option<serde_json::Value>,
        new_state: Option<serde_json::Value>,
    ) -> Self {
        self.previous_state = previous_state;
        self.new_state = new_state;
        self
    }

    pub fn with_request_info(
        mut self,
        ip_address: Option<String>,
        user_agent: Option<String>,
    ) -> Self {
        self.ip_address = ip_address;
        self.user_agent = user_agent;
        self
    }

    pub fn with_metadata(mut self, metadata: HashMap<String, String>) -> Self {
        self.metadata = Some(metadata);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::value_objects::{BackboneId, BackboneName, BackboneTimestamp};

    #[test]
    fn test_pagination_params() {
        let params = PaginationParams::new(2, 25);
        assert_eq!(params.offset(), 25);
        assert_eq!(params.limit(), 25);

        let default_params = PaginationParams::default();
        assert_eq!(default_params.offset(), 0);
        assert_eq!(default_params.limit(), 20);
    }

    #[test]
    fn test_backbone_filters() {
        let filters = BackboneFilters::new()
            .with_status(BackboneStatus::Active)
            .with_tags(vec!["tag1".to_string(), "tag2".to_string()])
            .with_search("test".to_string());

        assert!(filters.has_filters());
        assert_eq!(filters.status, Some(BackboneStatus::Active));
        assert!(filters.tags.is_some());

        let empty_filters = BackboneFilters::new();
        assert!(!empty_filters.has_filters());
    }

    #[test]
    fn test_sort_params() {
        let sort = SortParams::ascending(SortField::Name);
        assert_eq!(sort.field, SortField::Name);
        assert_eq!(sort.direction, SortDirection::Ascending);

        let default_sort = SortParams::default();
        assert_eq!(default_sort.field, SortField::CreatedAt);
        assert_eq!(default_sort.direction, SortDirection::Ascending);
    }

    #[test]
    fn test_paginated_result() {
        let items = vec!["item1", "item2"];
        let result = PaginatedResult::new(items, 10, 1, 3);

        assert_eq!(result.len(), 2);
        assert_eq!(result.total, 10);
        assert_eq!(result.page, 1);
        assert_eq!(result.page_size, 3);
        assert_eq!(result.total_pages, 4);
        assert!(result.has_next);
        assert!(!result.has_previous);

        let empty_result = PaginatedResult::<String>::empty(1, 10);
        assert!(empty_result.is_empty());
        assert_eq!(empty_result.len(), 0);
    }

    #[test]
    fn test_audit_entry() {
        let entry = AuditEntry::new(
            "backbone-id".to_string(),
            "CREATE".to_string(),
            "user".to_string(),
        );

        assert_eq!(entry.backbone_id, "backbone-id");
        assert_eq!(entry.action, "CREATE");
        assert_eq!(entry.changed_by, "user");
        assert!(entry.previous_state.is_none());
        assert!(entry.new_state.is_none());

        let with_states = entry.with_states(
            Some(serde_json::json!({ "name": "old" })),
            Some(serde_json::json!({ "name": "new" })),
        );

        assert!(with_states.previous_state.is_some());
        assert!(with_states.new_state.is_some());
    }
}