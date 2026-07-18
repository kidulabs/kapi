//! HTTP client library for the kapi API server.
//!
//! Provides [`KapiClient`] for performing CRUD operations, watching resources,
//! and managing schemas against a kapi server.
//!
//! For type-safe operations on generated wrapper structs, see [`TypedClient`].

pub mod client;
pub mod error;
pub mod typed;

// Re-export all shared kapi-core types for convenience.
pub use kapi_core::{
    ContinueToken, CoreError, FieldSelector, LabelRequirement, LabelSelector, ListOptions,
    ListResponse, ObjectMeta, ResourceKey, SchemaData, StoredObject, SystemMetadata,
    ValidationError, WatchEvent, WatchEventType, WatchFilter,
};

// Re-export typed client types for convenience.
pub use typed::{TypedClient, TypedError, TypedResource};
