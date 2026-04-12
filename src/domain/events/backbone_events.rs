// Backbone Domain Events
// Domain events for the Backbone aggregate

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::domain::value_objects::{BackboneId, BackboneStatus, Metadata};

// Base Domain Event Trait
pub trait DomainEvent {
    fn event_id(&self) -> &str;
    fn aggregate_id(&self) -> &BackboneId;
    fn event_type(&self) -> &'static str;
    fn occurred_at(&self) -> DateTime<Utc>;
    fn version(&self) -> i64;
}

// BackboneCreated Event
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BackboneCreated {
    pub event_id: String,
    pub backbone_id: BackboneId,
    pub name: String,
    pub description: String,
    pub status: BackboneStatus,
    pub tags: Vec<String>,
    pub metadata: Metadata,
    pub created_by: String,
    pub occurred_at: DateTime<Utc>,
    pub version: i64,
}

impl BackboneCreated {
    pub fn new(
        backbone_id: BackboneId,
        name: String,
        description: String,
        status: BackboneStatus,
        tags: Vec<String>,
        metadata: Metadata,
        created_by: String,
    ) -> Self {
        Self {
            event_id: uuid::Uuid::new_v4().to_string(),
            backbone_id,
            name,
            description,
            status,
            tags,
            metadata,
            created_by,
            occurred_at: Utc::now(),
            version: 1,
        }
    }
}

impl DomainEvent for BackboneCreated {
    fn event_id(&self) -> &str {
        &self.event_id
    }

    fn aggregate_id(&self) -> &BackboneId {
        &self.backbone_id
    }

    fn event_type(&self) -> &'static str {
        "BackboneCreated"
    }

    fn occurred_at(&self) -> DateTime<Utc> {
        self.occurred_at
    }

    fn version(&self) -> i64 {
        self.version
    }
}

// BackboneUpdated Event
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BackboneUpdated {
    pub event_id: String,
    pub backbone_id: BackboneId,
    pub changes: HashMap<String, String>,
    pub previous_version: i64,
    pub new_version: i64,
    pub updated_by: String,
    pub occurred_at: DateTime<Utc>,
}

impl BackboneUpdated {
    pub fn new(
        backbone_id: BackboneId,
        changes: HashMap<String, String>,
        previous_version: i64,
        new_version: i64,
        updated_by: String,
    ) -> Self {
        Self {
            event_id: uuid::Uuid::new_v4().to_string(),
            backbone_id,
            changes,
            previous_version,
            new_version,
            updated_by,
            occurred_at: Utc::now(),
        }
    }

    pub fn add_change(&mut self, field: String, old_value: String, new_value: String) {
        self.changes.insert(field, format!("{} -> {}", old_value, new_value));
    }

    pub fn has_changes(&self) -> bool {
        !self.changes.is_empty()
    }
}

impl DomainEvent for BackboneUpdated {
    fn event_id(&self) -> &str {
        &self.event_id
    }

    fn aggregate_id(&self) -> &BackboneId {
        &self.backbone_id
    }

    fn event_type(&self) -> &'static str {
        "BackboneUpdated"
    }

    fn occurred_at(&self) -> DateTime<Utc> {
        self.occurred_at
    }

    fn version(&self) -> i64 {
        self.new_version
    }
}

// BackboneStatusChanged Event
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BackboneStatusChanged {
    pub event_id: String,
    pub backbone_id: BackboneId,
    pub previous_status: BackboneStatus,
    pub new_status: BackboneStatus,
    pub reason: String,
    pub changed_by: String,
    pub occurred_at: DateTime<Utc>,
    pub version: i64,
}

impl BackboneStatusChanged {
    pub fn new(
        backbone_id: BackboneId,
        previous_status: BackboneStatus,
        new_status: BackboneStatus,
        reason: String,
        changed_by: String,
        version: i64,
    ) -> Self {
        Self {
            event_id: uuid::Uuid::new_v4().to_string(),
            backbone_id,
            previous_status,
            new_status,
            reason,
            changed_by,
            occurred_at: Utc::now(),
            version,
        }
    }

    pub fn is_activation(&self) -> bool {
        self.new_status.is_active() && !self.previous_status.is_active()
    }

    pub fn is_deactivation(&self) -> bool {
        !self.new_status.is_active() && self.previous_status.is_active()
    }

    pub fn is_suspension(&self) -> bool {
        self.new_status.is_suspended() && !self.previous_status.is_suspended()
    }

    pub fn is_archival(&self) -> bool {
        self.new_status.is_archived() && !self.previous_status.is_archived()
    }
}

impl DomainEvent for BackboneStatusChanged {
    fn event_id(&self) -> &str {
        &self.event_id
    }

    fn aggregate_id(&self) -> &BackboneId {
        &self.backbone_id
    }

    fn event_type(&self) -> &'static str {
        "BackboneStatusChanged"
    }

    fn occurred_at(&self) -> DateTime<Utc> {
        self.occurred_at
    }

    fn version(&self) -> i64 {
        self.version
    }
}

// BackboneTagsChanged Event
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BackboneTagsChanged {
    pub event_id: String,
    pub backbone_id: BackboneId,
    pub added_tags: Vec<String>,
    pub removed_tags: Vec<String>,
    pub changed_by: String,
    pub occurred_at: DateTime<Utc>,
    pub version: i64,
}

impl BackboneTagsChanged {
    pub fn new(
        backbone_id: BackboneId,
        added_tags: Vec<String>,
        removed_tags: Vec<String>,
        changed_by: String,
        version: i64,
    ) -> Self {
        Self {
            event_id: uuid::Uuid::new_v4().to_string(),
            backbone_id,
            added_tags,
            removed_tags,
            changed_by,
            occurred_at: Utc::now(),
            version,
        }
    }

    pub fn has_changes(&self) -> bool {
        !self.added_tags.is_empty() || !self.removed_tags.is_empty()
    }

    pub fn total_changes(&self) -> usize {
        self.added_tags.len() + self.removed_tags.len()
    }
}

impl DomainEvent for BackboneTagsChanged {
    fn event_id(&self) -> &str {
        &self.event_id
    }

    fn aggregate_id(&self) -> &BackboneId {
        &self.backbone_id
    }

    fn event_type(&self) -> &'static str {
        "BackboneTagsChanged"
    }

    fn occurred_at(&self) -> DateTime<Utc> {
        self.occurred_at
    }

    fn version(&self) -> i64 {
        self.version
    }
}

// BackboneMetadataChanged Event
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BackboneMetadataChanged {
    pub event_id: String,
    pub backbone_id: BackboneId,
    pub previous_metadata: Metadata,
    pub new_metadata: Metadata,
    pub changed_by: String,
    pub occurred_at: DateTime<Utc>,
    pub version: i64,
}

impl BackboneMetadataChanged {
    pub fn new(
        backbone_id: BackboneId,
        previous_metadata: Metadata,
        new_metadata: Metadata,
        changed_by: String,
        version: i64,
    ) -> Self {
        Self {
            event_id: uuid::Uuid::new_v4().to_string(),
            backbone_id,
            previous_metadata,
            new_metadata,
            changed_by,
            occurred_at: Utc::now(),
            version,
        }
    }

    pub fn has_changes(&self) -> bool {
        self.previous_metadata != self.new_metadata
    }

    pub fn get_added_keys(&self) -> Vec<String> {
        self.new_metadata
            .keys()
            .filter(|&k| !self.previous_metadata.contains_key(k))
            .cloned()
            .collect()
    }

    pub fn get_removed_keys(&self) -> Vec<String> {
        self.previous_metadata
            .keys()
            .filter(|&k| !self.new_metadata.contains_key(k))
            .cloned()
            .collect()
    }

    pub fn get_modified_keys(&self) -> Vec<String> {
        self.new_metadata
            .keys()
            .filter(|&k| {
                self.previous_metadata.contains_key(k)
                    && self.previous_metadata.get(k) != self.new_metadata.get(k)
            })
            .cloned()
            .collect()
    }
}

impl DomainEvent for BackboneMetadataChanged {
    fn event_id(&self) -> &str {
        &self.event_id
    }

    fn aggregate_id(&self) -> &BackboneId {
        &self.backbone_id
    }

    fn event_type(&self) -> &'static str {
        "BackboneMetadataChanged"
    }

    fn occurred_at(&self) -> DateTime<Utc> {
        self.occurred_at
    }

    fn version(&self) -> i64 {
        self.version
    }
}

// BackboneDeleted Event
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BackboneDeleted {
    pub event_id: String,
    pub backbone_id: BackboneId,
    pub hard_delete: bool,
    pub reason: String,
    pub deleted_by: String,
    pub occurred_at: DateTime<Utc>,
    pub version: i64,
}

impl BackboneDeleted {
    pub fn new(
        backbone_id: BackboneId,
        hard_delete: bool,
        reason: String,
        deleted_by: String,
        version: i64,
    ) -> Self {
        Self {
            event_id: uuid::Uuid::new_v4().to_string(),
            backbone_id,
            hard_delete,
            reason,
            deleted_by,
            occurred_at: Utc::now(),
            version,
        }
    }

    pub fn is_soft_delete(&self) -> bool {
        !self.hard_delete
    }

    pub fn is_hard_delete(&self) -> bool {
        self.hard_delete
    }
}

impl DomainEvent for BackboneDeleted {
    fn event_id(&self) -> &str {
        &self.event_id
    }

    fn aggregate_id(&self) -> &BackboneId {
        &self.backbone_id
    }

    fn event_type(&self) -> &'static str {
        "BackboneDeleted"
    }

    fn occurred_at(&self) -> DateTime<Utc> {
        self.occurred_at
    }

    fn version(&self) -> i64 {
        self.version
    }
}

// Event Store for managing domain events
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EventStore {
    pub events: Vec<Box<dyn DomainEvent>>,
}

impl EventStore {
    pub fn new() -> Self {
        Self { events: Vec::new() }
    }

    pub fn add_event<E: DomainEvent + 'static>(&mut self, event: E) {
        self.events.push(Box::new(event));
    }

    pub fn get_events(&self) -> &[Box<dyn DomainEvent>] {
        &self.events
    }

    pub fn get_events_by_type(&self, event_type: &str) -> Vec<&Box<dyn DomainEvent>> {
        self.events
            .iter()
            .filter(|e| e.event_type() == event_type)
            .collect()
    }

    pub fn clear(&mut self) {
        self.events.clear();
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    pub fn get_latest_version(&self) -> i64 {
        self.events
            .iter()
            .map(|e| e.version())
            .max()
            .unwrap_or(0)
    }
}

impl Default for EventStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::value_objects::{BackboneName, BackboneVersion};

    #[test]
    fn test_backbone_created_event() {
        let backbone_id = BackboneId::new("123e4567-e89b-12d3-a456-426614174000").unwrap();
        let metadata = Metadata::from_map([("key".to_string(), "value".to_string())].into()).unwrap();

        let event = BackboneCreated::new(
            backbone_id.clone(),
            "Test Backbone".to_string(),
            "Test Description".to_string(),
            BackboneStatus::Active,
            vec!["test".to_string()],
            metadata,
            "test_user".to_string(),
        );

        assert_eq!(event.event_type(), "BackboneCreated");
        assert_eq!(event.aggregate_id(), &backbone_id);
        assert_eq!(event.name, "Test Backbone");
        assert!(event.occurred_at <= Utc::now());
    }

    #[test]
    fn test_backbone_updated_event() {
        let backbone_id = BackboneId::new("123e4567-e89b-12d3-a456-426614174000").unwrap();
        let mut changes = HashMap::new();
        changes.insert("name".to_string(), "Old -> New".to_string());

        let event = BackboneUpdated::new(
            backbone_id.clone(),
            changes,
            1,
            2,
            "test_user".to_string(),
        );

        assert_eq!(event.event_type(), "BackboneUpdated");
        assert_eq!(event.aggregate_id(), &backbone_id);
        assert!(event.has_changes());
        assert_eq!(event.new_version, 2);
    }

    #[test]
    fn test_backbone_status_changed_event() {
        let backbone_id = BackboneId::new("123e4567-e89b-12d3-a456-426614174000").unwrap();

        let event = BackboneStatusChanged::new(
            backbone_id.clone(),
            BackboneStatus::Inactive,
            BackboneStatus::Active,
            "User activation".to_string(),
            "admin".to_string(),
            2,
        );

        assert_eq!(event.event_type(), "BackboneStatusChanged");
        assert_eq!(event.aggregate_id(), &backbone_id);
        assert!(event.is_activation());
        assert!(!event.is_deactivation());
    }

    #[test]
    fn test_backbone_tags_changed_event() {
        let backbone_id = BackboneId::new("123e4567-e89b-12d3-a456-426614174000").unwrap();

        let event = BackboneTagsChanged::new(
            backbone_id.clone(),
            vec!["new_tag".to_string()],
            vec!["old_tag".to_string()],
            "user".to_string(),
            3,
        );

        assert_eq!(event.event_type(), "BackboneTagsChanged");
        assert_eq!(event.aggregate_id(), &backbone_id);
        assert!(event.has_changes());
        assert_eq!(event.total_changes(), 2);
    }

    #[test]
    fn test_backbone_metadata_changed_event() {
        let backbone_id = BackboneId::new("123e4567-e89b-12d3-a456-426614174000").unwrap();
        let old_metadata = Metadata::from_map([("old".to_string(), "value".to_string())].into()).unwrap();
        let new_metadata = Metadata::from_map([("new".to_string(), "value".to_string())].into()).unwrap();

        let event = BackboneMetadataChanged::new(
            backbone_id.clone(),
            old_metadata.clone(),
            new_metadata.clone(),
            "user".to_string(),
            4,
        );

        assert_eq!(event.event_type(), "BackboneMetadataChanged");
        assert_eq!(event.aggregate_id(), &backbone_id);
        assert!(event.has_changes());
    }

    #[test]
    fn test_backbone_deleted_event() {
        let backbone_id = BackboneId::new("123e4567-e89b-12d3-a456-426614174000").unwrap();

        let soft_event = BackboneDeleted::new(
            backbone_id.clone(),
            false,
            "Soft delete".to_string(),
            "user".to_string(),
            5,
        );

        assert!(soft_event.is_soft_delete());
        assert!(!soft_event.is_hard_delete());

        let hard_event = BackboneDeleted::new(
            backbone_id,
            true,
            "Hard delete".to_string(),
            "admin".to_string(),
            6,
        );

        assert!(!hard_event.is_soft_delete());
        assert!(hard_event.is_hard_delete());
    }

    #[test]
    fn test_event_store() {
        let mut store = EventStore::new();
        assert!(store.is_empty());

        let backbone_id = BackboneId::new("123e4567-e89b-12d3-a456-426614174000").unwrap();
        let event = BackboneCreated::new(
            backbone_id,
            "Test".to_string(),
            "Desc".to_string(),
            BackboneStatus::Active,
            vec![],
            Metadata::new(),
            "user".to_string(),
        );

        store.add_event(event);
        assert_eq!(store.len(), 1);
        assert_eq!(store.get_latest_version(), 1);

        let events = store.get_events_by_type("BackboneCreated");
        assert_eq!(events.len(), 1);

        store.clear();
        assert!(store.is_empty());
    }
}