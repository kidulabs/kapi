//! Shared helper functions for object CRUD operations.
//!
//! These pure functions were extracted from `ObjectService` private methods to
//! enable reuse across multiple services (`ObjectService`, `SchemaService`).
//! They encapsulate metadata management, event publishing, and error mapping.

use chrono::Utc;

use crate::event::EventPublisher;
use crate::object::types::{StoredObject, ValidationError, WatchEvent, WatchEventType};
use crate::schema::SchemaValidationError;
use crate::store::{ResourceKey, TransactionOp};

/// Wraps a transaction callback to automatically manage system metadata.
///
/// This is the single place where resource_version increment, generation
/// bumping (on spec change), timestamp updates, and created_at preservation
/// happen. Callbacks focus purely on domain changes (spec, metadata, status).
///
/// This eliminates the "update_status landmine" — generation is automatically
/// preserved when the spec doesn't change, regardless of what the callback does.
///
/// # Usage
///
/// ```ignore
/// store.transaction(&key, &name, Box::new(|existing| {
///     helpers::apply_with_metadata(existing, |existing| {
///         let mut updated = existing.clone();
///         // ... apply domain changes ...
///         updated
///     })
/// }))
/// ```
pub(crate) fn apply_with_metadata<F>(existing: &StoredObject, mutator: F) -> TransactionOp
where
    F: FnOnce(&StoredObject) -> StoredObject,
{
    let mut new_obj = mutator(existing);
    // Bump resource_version on every mutation (enables CAS)
    new_obj.system.resource_version = existing.system.resource_version + 1;
    // Update the timestamp
    new_obj.system.updated_at = Utc::now();
    // Preserve the original creation timestamp
    new_obj.system.created_at = existing.system.created_at;
    // Preserve deletion_timestamp (server-managed)
    new_obj.system.deletion_timestamp = existing.system.deletion_timestamp;
    // Bump generation only if spec changed, otherwise preserve it
    if new_obj.spec != existing.spec {
        new_obj.system.generation = existing.system.generation + 1;
    } else {
        new_obj.system.generation = existing.system.generation;
    }
    TransactionOp::Apply(new_obj)
}

/// Publishes a watch event via the event bus.
///
/// Constructs a [`WatchEvent`] with the given type and object, then
/// delegates to [`EventPublisher::publish`].
pub(crate) fn publish_event(
    event_bus: &dyn EventPublisher,
    key: &ResourceKey,
    event_type: WatchEventType,
    object: &StoredObject,
) {
    event_bus.publish(key, WatchEvent { event_type, object: object.clone() });
}

/// Maps schema validation errors to domain validation errors.
///
/// Converts each [`SchemaValidationError`] (which contains `instance_path`
/// and `message`) into a [`ValidationError`] with the same fields,
/// for use in [`AppError::SchemaValidation`].
pub(crate) fn map_validation_errors(errors: Vec<SchemaValidationError>) -> Vec<ValidationError> {
    errors
        .into_iter()
        .map(|e| ValidationError { path: e.instance_path, message: e.message })
        .collect()
}
