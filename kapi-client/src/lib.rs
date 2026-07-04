//! HTTP client library for the kapi API server.
//!
//! Provides [`KapiClient`] for performing CRUD operations, watching resources,
//! and managing schemas against a kapi server.

pub mod client;
pub mod error;

// Re-export all shared kapi-core types for convenience.
pub use kapi_core::{
    ContinueToken, CoreError, FieldSelector, LabelRequirement, LabelSelector, ListOptions,
    ListResponse, ObjectMeta, ResourceKey, SchemaData, StoredObject, SystemMetadata,
    ValidationError, WatchEvent, WatchEventType, WatchFilter,
};
