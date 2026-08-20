//! HTTP client library for the kapi API server.
//!
//! Provides [`KapiClient`] for performing CRUD operations, watching resources,
//! and managing schemas against a kapi server.

pub mod client;
pub mod error;
pub mod typed;

// Re-export the typed client for convenience.
pub use typed::{TypedClient, TypedResource};

// Re-export all shared kapi-core types for convenience.
pub use kapi_core::{
    ApiError, ContinueToken, FieldSelector, LabelRequirement, LabelSelector, ListOptions,
    ListResponse, ObjectMeta, ResourceKey, SchemaData, StoredObject, SystemMetadata,
    ValidationError, WatchEvent, WatchEventType, WatchFilter,
};
