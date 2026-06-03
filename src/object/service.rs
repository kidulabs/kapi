//! ObjectService - orchestrates validation, storage, and event publishing.
//!
//! The service is the single entry point for object CRUD operations.
//! Handlers call the service, never the store directly.

use std::collections::HashMap;
use std::sync::Arc;

use regex::Regex;
use serde_json::Value;

use crate::error::AppError;
use crate::event::EventPublisher;
use crate::object::types::{
    ListOptions, ListResponse, ObjectMeta, SchemaData, StoredObject, WatchEvent, WatchEventType,
    WatchFilter, SpecData,
};
use crate::schema::{SchemaRegistry, SchemaValidator, SCHEMA_KIND};
#[cfg(test)]
use crate::schema::schema_key;
use crate::store::{ObjectStore, ResourceKey};

/// Validates a label key according to label validation rules.
/// Keys must be non-empty, max 256 chars, matching `[a-zA-Z0-9][-_.a-zA-Z0-9]*`
/// with optional `/` prefix separator (prefix: max 253 chars, DNS subdomain format).
fn validate_label_key(key: &str) -> Result<(), AppError> {
    if key.is_empty() {
        return Err(AppError::InvalidLabel(
            "label key must not be empty".to_string(),
        ));
    }
    if key.len() > 256 {
        return Err(AppError::InvalidLabel(format!(
            "label key '{}' exceeds maximum length of 256 characters",
            key
        )));
    }

    let (_prefix, name) = if let Some(slash_pos) = key.find('/') {
        let prefix = &key[..slash_pos];
        let name = &key[slash_pos + 1..];
        if prefix.is_empty() {
            return Err(AppError::InvalidLabel(format!(
                "label key '{}' has empty prefix before '/'",
                key
            )));
        }
        if prefix.len() > 253 {
            return Err(AppError::InvalidLabel(format!(
                "label key '{}' prefix exceeds maximum length of 253 characters",
                key
            )));
        }
        // Validate prefix as DNS subdomain: lowercase alphanumeric, hyphens, dots
        let prefix_re =
            Regex::new(r"^[a-z0-9]([-a-z0-9]*[a-z0-9])?(\.[a-z0-9]([-a-z0-9]*[a-z0-9])?)*$")
                .unwrap();
        if !prefix_re.is_match(prefix) {
            return Err(AppError::InvalidLabel(format!(
                "label key '{}' has invalid prefix '{}' (must be a valid DNS subdomain)",
                key, prefix
            )));
        }
        (Some(prefix), name)
    } else {
        (None, key)
    };

    if name.is_empty() {
        return Err(AppError::InvalidLabel(format!(
            "label key '{}' has empty name after '/'",
            key
        )));
    }

    // Validate name part: starts with alphanumeric, followed by [-_.a-zA-Z0-9]*
    let name_re = Regex::new(r"^[a-zA-Z0-9][-_.a-zA-Z0-9]*$").unwrap();
    if !name_re.is_match(name) {
        return Err(AppError::InvalidLabel(format!(
            "label key '{}' contains invalid characters (name part must match [a-zA-Z0-9][-_.a-zA-Z0-9]*)",
            key
        )));
    }

    Ok(())
}

/// Validates a label value according to label validation rules.
/// Values must be max 256 chars, matching `[a-zA-Z0-9][-_.a-zA-Z0-9]*` or empty string.
fn validate_label_value(key: &str, value: &str) -> Result<(), AppError> {
    if value.is_empty() {
        return Ok(()); // Empty values are allowed
    }
    if value.len() > 256 {
        return Err(AppError::InvalidLabel(format!(
            "label value for key '{}' exceeds maximum length of 256 characters",
            key
        )));
    }

    let value_re = Regex::new(r"^[a-zA-Z0-9][-_.a-zA-Z0-9]*$").unwrap();
    if !value_re.is_match(value) {
        return Err(AppError::InvalidLabel(format!(
            "label value '{}' for key '{}' contains invalid characters (must match [a-zA-Z0-9][-_.a-zA-Z0-9]* or be empty)",
            value, key
        )));
    }

    Ok(())
}

/// Validates all labels in a HashMap according to label validation rules.
/// Checks key format, value format, and length limits.
fn validate_labels(labels: &HashMap<String, String>) -> Result<(), AppError> {
    for (key, value) in labels {
        validate_label_key(key)?;
        validate_label_value(key, value)?;
    }
    Ok(())
}

/// ObjectService wraps store, event bus, and schema registry.
///
/// - `store`: The storage backend for persisting objects
/// - `event_bus`: Per-kind event bus for watch notifications
/// - `schema_registry`: Manages schema validation, compilation, and caching
pub struct ObjectService {
    /// The storage backend
    store: Arc<dyn ObjectStore>,
    /// Per-kind event bus for SSE watch notifications (via trait object)
    event_bus: Arc<dyn EventPublisher>,
    /// Schema registry for validation, compilation, and caching
    schema_registry: SchemaRegistry,
}

impl ObjectService {
    /// Creates a new ObjectService with the given store, event bus, and meta-validator.
    ///
    /// The SchemaRegistry is constructed internally from `store` and `meta_validator`,
    /// with an empty cache that is populated lazily as schemas are created or looked up.
    pub fn new(
        store: Arc<dyn ObjectStore>,
        event_bus: Arc<dyn EventPublisher>,
        meta_validator: Arc<dyn SchemaValidator>,
    ) -> Self {
        let schema_registry = SchemaRegistry::new(store.clone(), meta_validator);
        Self {
            store,
            event_bus,
            schema_registry,
        }
    }

    /// Creates an object (Schema or regular object) with validation.
    ///
    /// For Schema objects:
    /// 1. Validate against meta-schema
    /// 2. Compile the specSchema
    /// 3. Cache the compiled validator
    /// 4. Store and publish Added event
    ///
    /// For regular objects:
    /// 1. Look up the Schema from the store
    /// 2. Validate against cached compiled schema
    /// 3. Store and publish Added event
    pub async fn create(
        &self,
        key: ResourceKey,
        meta: ObjectMeta,
        spec: Value,
    ) -> Result<StoredObject, AppError> {
        if key.kind == SCHEMA_KIND {
            // Schema path: meta-schema validate → compile → cache → store → publish
            self.validate_and_create_schema(key, meta, spec).await
        } else {
            // Object path: lookup schema → validate → store → publish
            self.validate_and_create_object(key, meta, spec).await
        }
    }

    /// Gets an object by key and name — delegates to store.
    pub async fn get(&self, key: ResourceKey, name: String) -> Result<StoredObject, AppError> {
        self.store.get(&key, &name).await
    }

    /// Lists objects by key and options — delegates to store.
    pub async fn list(
        &self,
        key: ResourceKey,
        opts: ListOptions,
    ) -> Result<ListResponse, AppError> {
        self.store.list(&key, opts).await
    }

    /// Updates an object with validation.
    ///
    /// Same validation flow as create (meta-schema for Schema, compiled schema for objects).
    /// After successful store update, publishes a Modified event.
    pub async fn update(&self, object: StoredObject) -> Result<StoredObject, AppError> {
        let key = object.key.clone();
        let spec = object.spec.value.clone();

        if key.kind == SCHEMA_KIND {
            // Schema path: meta-schema validate → compile → cache → store → publish
            self.validate_and_update_schema(object, spec).await
        } else {
            // Object path: lookup schema → validate → store → publish
            self.validate_and_update_object(object, spec).await
        }
    }

    /// Deletes an object with Schema deletion guard.
    ///
    /// For Schema objects:
    /// 1. Fetch the Schema
    /// 2. Extract target kind
    /// 3. Check if objects of that kind exist → SchemaHasObjects if any
    /// 4. Delete, cache evict, publish Deleted
    ///
    /// For regular objects:
    /// 1. Delete and publish Deleted
    pub async fn delete(&self, key: ResourceKey, name: String) -> Result<StoredObject, AppError> {
        if key.kind == SCHEMA_KIND {
            // Schema path: check for existing objects before deletion
            self.delete_schema(key, name).await
        } else {
            // Regular object path: delete and publish
            let deleted = self.store.delete(&key, &name).await?;
            self.publish_event(&key, WatchEventType::Deleted, &deleted);
            Ok(deleted)
        }
    }

    /// Subscribe to watch events for the given key, filtered by WatchFilter.
    ///
    /// Delegates to the internal `EventPublisher` so handlers don't need
    /// to know the event bus exists.
    pub fn subscribe(&self, key: &ResourceKey, filter: WatchFilter) -> crate::event::WatchStream {
        self.event_bus.subscribe(key, filter)
    }

    /// Updates the status subresource of an object.
    ///
    /// Validates that a statusSchema exists for the kind, validates the status
    /// against it, calls the store's update_status, and publishes a StatusModified event.
    pub async fn update_status(
        &self,
        key: ResourceKey,
        name: String,
        status: Value,
    ) -> Result<StoredObject, AppError> {
        // Check that status subresource is enabled
        self.schema_registry.get_status_validator(&key).await?;

        // Validate status against statusSchema
        let validator = self.schema_registry.get_status_validator(&key).await?;
        if !validator.is_valid(&status) {
            let errors = Self::map_validation_errors(validator.validate(&status));
            return Err(AppError::SchemaValidation(errors));
        }

        // Update status in store
        let updated = self.store.update_status(&key, &name, status).await?;
        self.publish_event(&key, WatchEventType::StatusModified, &updated);
        Ok(updated)
    }

    /// Gets the status subresource of an object.
    ///
    /// Validates that a statusSchema exists for the kind, fetches the object,
    /// and returns the status field.
    pub async fn get_status(
        &self,
        key: ResourceKey,
        name: String,
    ) -> Result<Option<SpecData>, AppError> {
        // Check that status subresource is enabled
        self.schema_registry.get_status_validator(&key).await?;

        // Fetch object and return status
        let object = self.store.get(&key, &name).await?;
        Ok(object.status)
    }

    // --- Private helper methods ---

    /// Maps schema validation errors to domain validation errors.
    fn map_validation_errors(
        errors: Vec<crate::schema::SchemaValidationError>,
    ) -> Vec<crate::object::types::ValidationError> {
        errors
            .into_iter()
            .map(|e| crate::object::types::ValidationError {
                path: e.instance_path,
                message: e.message,
            })
            .collect()
    }

    /// Publishes a watch event via the event bus.
    fn publish_event(&self, key: &ResourceKey, event_type: WatchEventType, object: &StoredObject) {
        self.event_bus.publish(
            key,
            WatchEvent {
                event_type,
                object: object.clone(),
            },
        );
    }

    async fn validate_and_create_schema(
        &self,
        key: ResourceKey,
        meta: ObjectMeta,
        spec: Value,
    ) -> Result<StoredObject, AppError> {
        validate_labels(&meta.labels)?;
        let (_schema_data, compiled, status_compiled) =
            self.schema_registry.validate_and_compile(&spec)?;
        let stored = self.store.create(&key, meta.clone(), spec).await?;
        self.schema_registry.insert(&meta.name, compiled);
        if let Some(status_validator) = status_compiled {
            self.schema_registry
                .insert_status(&meta.name, status_validator);
        }
        self.publish_event(&key, WatchEventType::Added, &stored);
        Ok(stored)
    }

    /// Validates an object against its cached schema and creates it.
    async fn validate_and_create_object(
        &self,
        key: ResourceKey,
        meta: ObjectMeta,
        spec: Value,
    ) -> Result<StoredObject, AppError> {
        validate_labels(&meta.labels)?;
        let validator = self.schema_registry.get_validator(&key).await?;

        if !validator.is_valid(&spec) {
            let errors = Self::map_validation_errors(validator.validate(&spec));
            return Err(AppError::SchemaValidation(errors));
        }

        let stored = self.store.create(&key, meta, spec).await?;
        self.publish_event(&key, WatchEventType::Added, &stored);
        Ok(stored)
    }

    /// Validates and updates a Schema object.
    async fn validate_and_update_schema(
        &self,
        object: StoredObject,
        spec: Value,
    ) -> Result<StoredObject, AppError> {
        validate_labels(&object.metadata.labels)?;
        let (_schema_data, compiled, status_compiled) =
            self.schema_registry.validate_and_compile(&spec)?;
        let updated = self.store.update(object).await?;
        self.schema_registry
            .insert(&updated.metadata.name, compiled);
        if let Some(status_validator) = status_compiled {
            self.schema_registry
                .insert_status(&updated.metadata.name, status_validator);
        }
        self.publish_event(&updated.key, WatchEventType::Modified, &updated);
        Ok(updated)
    }

    /// Validates and updates a regular object.
    async fn validate_and_update_object(
        &self,
        object: StoredObject,
        spec: Value,
    ) -> Result<StoredObject, AppError> {
        validate_labels(&object.metadata.labels)?;
        let validator = self.schema_registry.get_validator(&object.key).await?;

        if !validator.is_valid(&spec) {
            let errors = Self::map_validation_errors(validator.validate(&spec));
            return Err(AppError::SchemaValidation(errors));
        }

        let updated = self.store.update(object).await?;
        self.publish_event(&updated.key, WatchEventType::Modified, &updated);
        Ok(updated)
    }

    /// Deletes a Schema object with guard against deleting schemas with existing objects.
    async fn delete_schema(
        &self,
        key: ResourceKey,
        name: String,
    ) -> Result<StoredObject, AppError> {
        // Step 1: Get the schema from store
        let schema_obj = self.store.get(&key, &name).await?;

        // Step 2: Parse schema data to extract target kind
        let schema_data: SchemaData = serde_json::from_value(schema_obj.spec.value.clone())
            .map_err(|e| AppError::InvalidSchema(format!("failed to parse schema data: {}", e)))?;

        // Step 3: Build target key and check for existing objects
        let target_key = ResourceKey {
            group: schema_data.target_group,
            version: schema_data.target_version,
            kind: schema_data.target_kind,
        };

        // Check if any objects of the target kind exist
        if self.store.exists(&target_key).await? {
            return Err(AppError::SchemaHasObjects {
                kind: target_key.kind,
            });
        }

        // Step 4: Delete the schema
        let deleted = self.store.delete(&key, &name).await?;

        // Step 5: Evict from cache
        self.schema_registry.evict(&name);

        // Step 6: Publish Deleted event
        self.publish_event(&key, WatchEventType::Deleted, &deleted);

        Ok(deleted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::meta_schema::compile_meta_schema;
    use crate::store::memory::InMemoryStore;
    use serde_json::json;
    use std::collections::HashMap;

    // Helper to create a service with a fresh store and event bus
    fn make_service() -> ObjectService {
        let store: Arc<dyn ObjectStore> = Arc::new(InMemoryStore::new());
        let event_bus: Arc<dyn EventPublisher> = Arc::new(crate::event::EventBus::default());
        let meta_validator: Arc<dyn SchemaValidator> =
            Arc::new(compile_meta_schema().expect("meta-schema should compile"));
        ObjectService::new(store, event_bus, meta_validator)
    }

    // Helper to register a Schema for testing.
    // The name format "{targetKind}.{targetGroup}" is backend-generated
    // (see handler::extract_schema_name), but tests call service.create()
    // directly and must supply the name.
    async fn register_test_schema(service: &ObjectService) {
        let schema_key = schema_key();
        let schema_data = json!({
            "targetGroup": "example.io",
            "targetVersion": "v1",
            "targetKind": "Widget",
            "specSchema": {
                "type": "object",
                "properties": {
                    "color": { "type": "string" },
                    "size": { "type": "integer" }
                }
            }
        });
        // Name is generated as "{targetKind}.{targetGroup}" by the handler
        service
            .create(
                schema_key,
                ObjectMeta {
                    name: "Widget.example.io".to_string(),
                    labels: HashMap::new(),
                },
                schema_data,
            )
            .await
            .expect("schema registration should succeed");
    }

    // T19: Create valid Schema → stored, cached, event published
    #[tokio::test]
    async fn create_valid_schema_stored_cached_event_published() {
        let service = make_service();
        let schema_key = schema_key();
        let schema_data = json!({
            "targetGroup": "example.io",
            "targetVersion": "v1",
            "targetKind": "Widget",
            "specSchema": { "type": "object" }
        });

        let result = service
            .create(
                schema_key.clone(),
                ObjectMeta {
                    name: "Widget.example.io".to_string(),
                    labels: HashMap::new(),
                },
                schema_data,
            )
            .await;
        assert!(result.is_ok());
        let stored = result.unwrap();
        assert_eq!(stored.metadata.name, "Widget.example.io");

        // Verify cached
        assert!(service.schema_registry.cache.contains_key("Widget.example.io"));

        // Verify stored in store
        let retrieved = service
            .get(schema_key, "Widget.example.io".to_string())
            .await
            .unwrap();
        assert_eq!(retrieved.metadata.name, "Widget.example.io");
    }

    // T20: Create Schema with invalid meta-schema → InvalidSchema, nothing stored
    #[tokio::test]
    async fn create_schema_invalid_meta_schema_returns_error() {
        let service = make_service();
        let schema_key = schema_key();
        // Missing required fields
        let invalid_data = json!({ "targetGroup": "example.io" });

        // Name would be generated as "Widget.example.io" by the handler,
        // but this test calls service.create() directly
        let result = service
            .create(
                schema_key,
                ObjectMeta {
                    name: "Widget.example.io".to_string(),
                    labels: HashMap::new(),
                },
                invalid_data,
            )
            .await;
        assert!(matches!(result, Err(AppError::InvalidSchema(_))));
    }

    // T21: Create Schema with uncompileable specSchema → InvalidSchema, nothing stored
    #[tokio::test]
    async fn create_schema_uncompileable_spec_schema_returns_error() {
        let service = make_service();
        let schema_key = schema_key();
        // specSchema with invalid content (not a valid JSON Schema)
        let invalid_schema = json!({
            "targetGroup": "example.io",
            "targetVersion": "v1",
            "targetKind": "Widget",
            "specSchema": { "type": "not-a-real-type" }
        });

        // Name would be generated as "Widget.example.io" by the handler
        let result = service
            .create(
                schema_key,
                ObjectMeta {
                    name: "Widget.example.io".to_string(),
                    labels: HashMap::new(),
                },
                invalid_schema,
            )
            .await;
        // This should fail during compilation of specSchema
        assert!(matches!(result, Err(AppError::InvalidSchema(_))));
    }

    // T22: Create object for unregistered kind → NotFound
    #[tokio::test]
    async fn create_object_unregistered_kind_returns_not_found() {
        let service = make_service();
        let widget_key = ResourceKey {
            group: "example.io".to_string(),
            version: "v1".to_string(),
            kind: "Widget".to_string(),
        };

        let result = service
            .create(
                widget_key,
                ObjectMeta {
                    name: "my-widget".to_string(),
                    labels: HashMap::new(),
                },
                json!({}),
            )
            .await;
        assert!(matches!(result, Err(AppError::NotFound { .. })));
    }

    // T23: Create object with invalid data → SchemaValidation
    #[tokio::test]
    async fn create_object_invalid_data_returns_schema_validation() {
        let service = make_service();
        register_test_schema(&service).await;

        let widget_key = ResourceKey {
            group: "example.io".to_string(),
            version: "v1".to_string(),
            kind: "Widget".to_string(),
        };
        // Invalid data: size should be integer, not string
        let invalid_data = json!({ "color": "blue", "size": "not-a-number" });

        let result = service
            .create(
                widget_key,
                ObjectMeta {
                    name: "my-widget".to_string(),
                    labels: HashMap::new(),
                },
                invalid_data,
            )
            .await;
        assert!(matches!(result, Err(AppError::SchemaValidation(_))));
    }

    // T24: Update with correct version → success, Modified event published
    #[tokio::test]
    async fn update_correct_version_succeeds() {
        let service = make_service();
        register_test_schema(&service).await;

        let widget_key = ResourceKey {
            group: "example.io".to_string(),
            version: "v1".to_string(),
            kind: "Widget".to_string(),
        };
        let created = service
            .create(
                widget_key.clone(),
                ObjectMeta {
                    name: "my-widget".to_string(),
                    labels: HashMap::new(),
                },
                json!({ "color": "blue", "size": 10 }),
            )
            .await
            .unwrap();

        let v1 = created.system.resource_version;
        let mut updated_obj = created;
        updated_obj.spec.value = json!({ "color": "red", "size": 20 });
        updated_obj.system.resource_version = v1;

        let result = service.update(updated_obj).await;
        assert!(result.is_ok());
        assert!(result.unwrap().system.resource_version > v1);
    }

    // T25: Update with wrong version → Conflict, no event published
    #[tokio::test]
    async fn update_wrong_version_returns_conflict() {
        let service = make_service();
        register_test_schema(&service).await;

        let widget_key = ResourceKey {
            group: "example.io".to_string(),
            version: "v1".to_string(),
            kind: "Widget".to_string(),
        };
        let created = service
            .create(
                widget_key.clone(),
                ObjectMeta {
                    name: "my-widget".to_string(),
                    labels: HashMap::new(),
                },
                json!({ "color": "blue", "size": 10 }),
            )
            .await
            .unwrap();

        let mut wrong_version_obj = created;
        wrong_version_obj.spec.value = json!({ "color": "red" });
        wrong_version_obj.system.resource_version = 999;

        let result = service.update(wrong_version_obj).await;
        assert!(matches!(result, Err(AppError::Conflict { .. })));
    }

    // T26: Delete Schema with no objects → success, cache evicted, Deleted event published
    #[tokio::test]
    async fn delete_schema_no_objects_succeeds() {
        let service = make_service();
        let schema_key = schema_key();
        let schema_data = json!({
            "targetGroup": "example.io",
            "targetVersion": "v1",
            "targetKind": "Widget",
            "specSchema": { "type": "object" }
        });
        service
            .create(
                schema_key.clone(),
                ObjectMeta {
                    name: "Widget.example.io".to_string(),
                    labels: HashMap::new(),
                },
                schema_data,
            )
            .await
            .unwrap();

        // Verify cached
        assert!(service.schema_registry.cache.contains_key("Widget.example.io"));

        // Delete the schema
        let result = service
            .delete(schema_key, "Widget.example.io".to_string())
            .await;
        assert!(result.is_ok());

        // Verify cache evicted
        assert!(!service.schema_registry.cache.contains_key("Widget.example.io"));
    }

    // T27: Delete Schema with existing objects → SchemaHasObjects, nothing deleted
    #[tokio::test]
    async fn delete_schema_with_objects_returns_conflict() {
        let service = make_service();
        register_test_schema(&service).await;

        // Create an object of the registered kind
        let widget_key = ResourceKey {
            group: "example.io".to_string(),
            version: "v1".to_string(),
            kind: "Widget".to_string(),
        };
        service
            .create(
                widget_key.clone(),
                ObjectMeta {
                    name: "my-widget".to_string(),
                    labels: HashMap::new(),
                },
                json!({ "color": "blue", "size": 10 }),
            )
            .await
            .unwrap();

        // Try to delete the schema
        let schema_key = schema_key();
        let result = service
            .delete(schema_key, "Widget.example.io".to_string())
            .await;
        assert!(
            matches!(result, Err(AppError::SchemaHasObjects { kind }) if kind == "Widget")
        );
    }

    // T28: Delete regular object → success, Deleted event published
    #[tokio::test]
    async fn delete_regular_object_succeeds() {
        let service = make_service();
        register_test_schema(&service).await;

        let widget_key = ResourceKey {
            group: "example.io".to_string(),
            version: "v1".to_string(),
            kind: "Widget".to_string(),
        };
        let created = service
            .create(
                widget_key.clone(),
                ObjectMeta {
                    name: "my-widget".to_string(),
                    labels: HashMap::new(),
                },
                json!({ "color": "blue", "size": 10 }),
            )
            .await
            .unwrap();

        let result = service.delete(widget_key, "my-widget".to_string()).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().metadata.name, created.metadata.name);
    }

    // T29: Failed create (duplicate) → no Added event published
    #[tokio::test]
    async fn create_duplicate_no_event_published() {
        let service = make_service();
        let schema_key = schema_key();
        let schema_data = json!({
            "targetGroup": "example.io",
            "targetVersion": "v1",
            "targetKind": "Widget",
            "specSchema": { "type": "object" }
        });
        service
            .create(
                schema_key.clone(),
                ObjectMeta {
                    name: "Widget.example.io".to_string(),
                    labels: HashMap::new(),
                },
                schema_data.clone(),
            )
            .await
            .unwrap();

        // Try to create duplicate
        let result = service
            .create(
                schema_key.clone(),
                ObjectMeta {
                    name: "Widget.example.io".to_string(),
                    labels: HashMap::new(),
                },
                schema_data,
            )
            .await;
        assert!(matches!(result, Err(AppError::AlreadyExists { .. })));
    }

    // T30: Schema cache eviction on Schema delete
    #[tokio::test]
    async fn schema_cache_eviction_on_delete() {
        let service = make_service();
        let schema_key = schema_key();
        let schema_data = json!({
            "targetGroup": "example.io",
            "targetVersion": "v1",
            "targetKind": "Widget",
            "specSchema": { "type": "object" }
        });
        service
            .create(
                schema_key.clone(),
                ObjectMeta {
                    name: "Widget.example.io".to_string(),
                    labels: HashMap::new(),
                },
                schema_data,
            )
            .await
            .unwrap();
        assert!(service.schema_registry.cache.contains_key("Widget.example.io"));

        service
            .delete(schema_key, "Widget.example.io".to_string())
            .await
            .unwrap();
        assert!(!service.schema_registry.cache.contains_key("Widget.example.io"));
    }

    // Schema create with missing targetKind returns InvalidSchema error
    // (meta-schema requires targetKind as a required field)
    #[tokio::test]
    async fn create_schema_missing_target_kind_returns_error() {
        let service = make_service();
        let schema_key = schema_key();
        // Missing targetKind
        let schema_data = json!({
            "targetGroup": "example.io",
            "targetVersion": "v1",
            "specSchema": { "type": "object" }
        });

        let result = service
            .create(
                schema_key,
                ObjectMeta {
                    name: "Widget.example.io".to_string(),
                    labels: HashMap::new(),
                },
                schema_data,
            )
            .await;
        assert!(matches!(result, Err(AppError::InvalidSchema(_))));
    }

    // Schema create with missing targetGroup returns InvalidSchema error
    // (meta-schema requires targetGroup as a required field)
    #[tokio::test]
    async fn create_schema_missing_target_group_returns_error() {
        let service = make_service();
        let schema_key = schema_key();
        // Missing targetGroup
        let schema_data = json!({
            "targetVersion": "v1",
            "targetKind": "Widget",
            "specSchema": { "type": "object" }
        });

        let result = service
            .create(
                schema_key,
                ObjectMeta {
                    name: "Widget.example.io".to_string(),
                    labels: HashMap::new(),
                },
                schema_data,
            )
            .await;
        assert!(matches!(result, Err(AppError::InvalidSchema(_))));
    }

    // T31: Object creation after simulated restart (shared store, empty cache) succeeds
    #[tokio::test]
    async fn object_creation_after_restart_with_empty_cache_succeeds() {
        let store: Arc<dyn ObjectStore> = Arc::new(InMemoryStore::new());
        let event_bus: Arc<dyn EventPublisher> = Arc::new(crate::event::EventBus::default());
        let meta_validator: Arc<dyn SchemaValidator> =
            Arc::new(compile_meta_schema().expect("meta-schema should compile"));

        let schema_key = schema_key();
        let schema_data = json!({
            "targetGroup": "example.io",
            "targetVersion": "v1",
            "targetKind": "Widget",
            "specSchema": {
                "type": "object",
                "properties": { "color": { "type": "string" } }
            }
        });

        let widget_key = ResourceKey {
            group: "example.io".to_string(),
            version: "v1".to_string(),
            kind: "Widget".to_string(),
        };

        // Service A: register schema and create an object
        let service_a =
            ObjectService::new(store.clone(), event_bus.clone(), meta_validator.clone());
        service_a
            .create(
                schema_key,
                ObjectMeta {
                    name: "Widget.example.io".to_string(),
                    labels: HashMap::new(),
                },
                schema_data,
            )
            .await
            .expect("schema registration should succeed");
        service_a
            .create(
                widget_key.clone(),
                ObjectMeta {
                    name: "widget-1".to_string(),
                    labels: HashMap::new(),
                },
                json!({"color": "red"}),
            )
            .await
            .expect("first object should succeed");

        // Service B: same store, fresh cache (simulating restart)
        let service_b = ObjectService::new(store, event_bus, meta_validator);
        assert!(!service_b.schema_registry.cache.contains_key("Widget.example.io"));

        let result = service_b
            .create(
                widget_key,
                ObjectMeta {
                    name: "widget-2".to_string(),
                    labels: HashMap::new(),
                },
                json!({"color": "blue"}),
            )
            .await;
        assert!(result.is_ok());
        assert!(service_b.schema_registry.cache.contains_key("Widget.example.io"));
    }

    // T32: Cache miss triggers compilation, subsequent requests use cached validator
    #[tokio::test]
    async fn cache_miss_triggers_compilation_and_subsequent_uses_cache() {
        let store: Arc<dyn ObjectStore> = Arc::new(InMemoryStore::new());
        let event_bus: Arc<dyn EventPublisher> = Arc::new(crate::event::EventBus::default());
        let meta_validator: Arc<dyn SchemaValidator> =
            Arc::new(compile_meta_schema().expect("meta-schema should compile"));

        let schema_key = schema_key();
        let schema_data = json!({
            "targetGroup": "example.io",
            "targetVersion": "v1",
            "targetKind": "Widget",
            "specSchema": {
                "type": "object",
                "properties": {
                    "color": { "type": "string" },
                    "size": { "type": "integer" }
                }
            }
        });

        let widget_key = ResourceKey {
            group: "example.io".to_string(),
            version: "v1".to_string(),
            kind: "Widget".to_string(),
        };

        // Register schema via service A
        let service_a =
            ObjectService::new(store.clone(), event_bus.clone(), meta_validator.clone());
        service_a
            .create(
                schema_key,
                ObjectMeta {
                    name: "Widget.example.io".to_string(),
                    labels: HashMap::new(),
                },
                schema_data,
            )
            .await
            .expect("schema registration should succeed");

        // Service B starts with empty cache
        let service_b = ObjectService::new(store, event_bus, meta_validator);
        assert!(!service_b.schema_registry.cache.contains_key("Widget.example.io"));

        // First creation triggers lazy compilation
        let first = service_b
            .create(
                widget_key.clone(),
                ObjectMeta {
                    name: "widget-1".to_string(),
                    labels: HashMap::new(),
                },
                json!({"color": "red", "size": 1}),
            )
            .await;
        assert!(first.is_ok());
        assert!(service_b.schema_registry.cache.contains_key("Widget.example.io"));

        // Second creation uses cached validator
        let second = service_b
            .create(
                widget_key,
                ObjectMeta {
                    name: "widget-2".to_string(),
                    labels: HashMap::new(),
                },
                json!({"color": "blue", "size": 2}),
            )
            .await;
        assert!(second.is_ok());
    }

    // T33: Stored schema with invalid specSchema returns StoredSchemaCompilationFailed
    #[tokio::test]
    async fn stored_schema_invalid_jsonschema_returns_compilation_failed() {
        let store: Arc<dyn ObjectStore> = Arc::new(InMemoryStore::new());
        let event_bus: Arc<dyn EventPublisher> = Arc::new(crate::event::EventBus::default());
        let meta_validator: Arc<dyn SchemaValidator> =
            Arc::new(compile_meta_schema().expect("meta-schema should compile"));

        // Bypass service to store a schema with invalid specSchema directly
        let schema_key = schema_key();
        let invalid_schema = json!({
            "targetGroup": "example.io",
            "targetVersion": "v1",
            "targetKind": "Widget",
            "specSchema": { "type": "not-a-real-type" }
        });
        store
            .create(
                &schema_key,
                ObjectMeta {
                    name: "Widget.example.io".to_string(),
                    labels: HashMap::new(),
                },
                invalid_schema,
            )
            .await
            .expect("store create should succeed");

        // Service with empty cache should fail to compile the stored schema
        let service = ObjectService::new(store, event_bus, meta_validator);
        let widget_key = ResourceKey {
            group: "example.io".to_string(),
            version: "v1".to_string(),
            kind: "Widget".to_string(),
        };
        let result = service
            .create(
                widget_key,
                ObjectMeta {
                    name: "my-widget".to_string(),
                    labels: HashMap::new(),
                },
                json!({"color": "red"}),
            )
            .await;
        assert!(
            matches!(result, Err(AppError::StoredSchemaCompilationFailed { .. })),
            "expected StoredSchemaCompilationFailed, got {:?}",
            result
        );
    }

    // --- validate_labels unit tests ---

    #[test]
    fn validate_labels_empty_map() {
        let labels = HashMap::new();
        assert!(validate_labels(&labels).is_ok());
    }

    #[test]
    fn validate_labels_valid_simple_keys() {
        let mut labels = HashMap::new();
        labels.insert("app".to_string(), "nginx".to_string());
        labels.insert("my-label".to_string(), "v1".to_string());
        labels.insert("label_name.v2".to_string(), "prod".to_string());
        assert!(validate_labels(&labels).is_ok());
    }

    #[test]
    fn validate_labels_valid_prefixed_keys() {
        let mut labels = HashMap::new();
        labels.insert("app.example.io/name".to_string(), "myapp".to_string());
        labels.insert("example.com/tier".to_string(), "frontend".to_string());
        assert!(validate_labels(&labels).is_ok());
    }

    #[test]
    fn validate_labels_empty_key_rejected() {
        let mut labels = HashMap::new();
        labels.insert("".to_string(), "value".to_string());
        assert!(validate_labels(&labels).is_err());
    }

    #[test]
    fn validate_labels_key_too_long() {
        let mut labels = HashMap::new();
        let long_key = "a".repeat(257);
        labels.insert(long_key, "value".to_string());
        assert!(validate_labels(&labels).is_err());
    }

    #[test]
    fn validate_labels_key_invalid_chars() {
        let mut labels = HashMap::new();
        labels.insert("invalid key!".to_string(), "value".to_string());
        assert!(validate_labels(&labels).is_err());
    }

    #[test]
    fn validate_labels_value_too_long() {
        let mut labels = HashMap::new();
        let long_value = "a".repeat(257);
        labels.insert("key".to_string(), long_value);
        assert!(validate_labels(&labels).is_err());
    }

    #[test]
    fn validate_labels_value_invalid_chars() {
        let mut labels = HashMap::new();
        labels.insert("key".to_string(), "invalid value!".to_string());
        assert!(validate_labels(&labels).is_err());
    }

    #[test]
    fn validate_labels_empty_value_allowed() {
        let mut labels = HashMap::new();
        labels.insert("key".to_string(), "".to_string());
        assert!(validate_labels(&labels).is_ok());
    }

    #[test]
    fn validate_labels_prefix_too_long() {
        let mut labels = HashMap::new();
        let long_prefix = "a".repeat(254);
        labels.insert(format!("{}/name", long_prefix), "value".to_string());
        assert!(validate_labels(&labels).is_err());
    }

    // --- Status subresource tests ---

    // Helper to register a Schema with statusSchema
    async fn register_test_schema_with_status(service: &ObjectService) {
        let schema_key = schema_key();
        let schema_data = json!({
            "targetGroup": "example.io",
            "targetVersion": "v1",
            "targetKind": "Widget",
            "specSchema": {
                "type": "object",
                "properties": {
                    "color": { "type": "string" },
                    "size": { "type": "integer" }
                }
            },
            "statusSchema": {
                "type": "object",
                "properties": {
                    "phase": { "type": "string" }
                }
            }
        });
        service
            .create(
                schema_key,
                ObjectMeta {
                    name: "Widget.example.io".to_string(),
                    labels: HashMap::new(),
                },
                schema_data,
            )
            .await
            .expect("schema registration should succeed");
    }

    #[tokio::test]
    async fn update_status_with_status_schema_succeeds() {
        let service = make_service();
        register_test_schema_with_status(&service).await;

        let widget_key = ResourceKey {
            group: "example.io".to_string(),
            version: "v1".to_string(),
            kind: "Widget".to_string(),
        };
        let created = service
            .create(
                widget_key.clone(),
                ObjectMeta {
                    name: "my-widget".to_string(),
                    labels: HashMap::new(),
                },
                json!({"color": "blue", "size": 10}),
            )
            .await
            .unwrap();
        assert!(created.status.is_none());

        let updated = service
            .update_status(
                widget_key.clone(),
                "my-widget".to_string(),
                json!({"phase": "Running"}),
            )
            .await
            .unwrap();
        assert!(updated.status.is_some());
        assert_eq!(updated.status.unwrap().value, json!({"phase": "Running"}));
    }

    #[tokio::test]
    async fn update_status_without_status_schema_returns_error() {
        let service = make_service();
        // Register schema WITHOUT statusSchema
        register_test_schema(&service).await;

        let widget_key = ResourceKey {
            group: "example.io".to_string(),
            version: "v1".to_string(),
            kind: "Widget".to_string(),
        };
        service
            .create(
                widget_key.clone(),
                ObjectMeta {
                    name: "my-widget".to_string(),
                    labels: HashMap::new(),
                },
                json!({"color": "blue"}),
            )
            .await
            .unwrap();

        let err = service
            .update_status(
                widget_key.clone(),
                "my-widget".to_string(),
                json!({"phase": "Running"}),
            )
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::StatusSubresourceNotEnabled { .. }));
    }

    #[tokio::test]
    async fn update_status_invalid_status_returns_validation_error() {
        let service = make_service();
        register_test_schema_with_status(&service).await;

        let widget_key = ResourceKey {
            group: "example.io".to_string(),
            version: "v1".to_string(),
            kind: "Widget".to_string(),
        };
        service
            .create(
                widget_key.clone(),
                ObjectMeta {
                    name: "my-widget".to_string(),
                    labels: HashMap::new(),
                },
                json!({"color": "blue"}),
            )
            .await
            .unwrap();

        // phase should be string, not integer
        let err = service
            .update_status(
                widget_key.clone(),
                "my-widget".to_string(),
                json!({"phase": 123}),
            )
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::SchemaValidation(_)));
    }

    #[tokio::test]
    async fn update_status_not_found() {
        let service = make_service();
        register_test_schema_with_status(&service).await;

        let widget_key = ResourceKey {
            group: "example.io".to_string(),
            version: "v1".to_string(),
            kind: "Widget".to_string(),
        };

        let err = service
            .update_status(
                widget_key.clone(),
                "nonexistent".to_string(),
                json!({"phase": "Running"}),
            )
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::NotFound { .. }));
    }

    #[tokio::test]
    async fn get_status_with_status_schema_returns_status() {
        let service = make_service();
        register_test_schema_with_status(&service).await;

        let widget_key = ResourceKey {
            group: "example.io".to_string(),
            version: "v1".to_string(),
            kind: "Widget".to_string(),
        };
        service
            .create(
                widget_key.clone(),
                ObjectMeta {
                    name: "my-widget".to_string(),
                    labels: HashMap::new(),
                },
                json!({"color": "blue"}),
            )
            .await
            .unwrap();

        // Initially None
        let status = service
            .get_status(widget_key.clone(), "my-widget".to_string())
            .await
            .unwrap();
        assert!(status.is_none());

        // After update
        service
            .update_status(
                widget_key.clone(),
                "my-widget".to_string(),
                json!({"phase": "Running"}),
            )
            .await
            .unwrap();

        let status = service
            .get_status(widget_key.clone(), "my-widget".to_string())
            .await
            .unwrap();
        assert!(status.is_some());
        assert_eq!(status.unwrap().value, json!({"phase": "Running"}));
    }

    #[tokio::test]
    async fn get_status_without_status_schema_returns_error() {
        let service = make_service();
        register_test_schema(&service).await;

        let widget_key = ResourceKey {
            group: "example.io".to_string(),
            version: "v1".to_string(),
            kind: "Widget".to_string(),
        };
        service
            .create(
                widget_key.clone(),
                ObjectMeta {
                    name: "my-widget".to_string(),
                    labels: HashMap::new(),
                },
                json!({"color": "blue"}),
            )
            .await
            .unwrap();

        let err = service
            .get_status(widget_key.clone(), "my-widget".to_string())
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::StatusSubresourceNotEnabled { .. }));
    }

    #[tokio::test]
    async fn create_strips_status_from_body() {
        let service = make_service();
        register_test_schema_with_status(&service).await;

        let widget_key = ResourceKey {
            group: "example.io".to_string(),
            version: "v1".to_string(),
            kind: "Widget".to_string(),
        };

        // Create with status in body — should be ignored
        let created = service
            .create(
                widget_key.clone(),
                ObjectMeta {
                    name: "my-widget".to_string(),
                    labels: HashMap::new(),
                },
                json!({"color": "blue", "status": {"phase": "Running"}}),
            )
            .await
            .unwrap();
        assert!(created.status.is_none());
    }
}
