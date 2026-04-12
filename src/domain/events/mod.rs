// Domain Events Module
// All domain events for the Backbone bounded context

pub mod backbone_events;

pub use backbone_events::{
    BackboneCreated, BackboneDeleted, BackboneMetadataChanged, BackboneStatusChanged,
    BackboneTagsChanged, BackboneUpdated, DomainEvent, EventStore,
};