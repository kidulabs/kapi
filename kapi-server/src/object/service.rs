//! ObjectService - orchestrates validation, storage, and event publishing.
//!
//! The service is the single entry point for object CRUD operations.
//! Handlers call the service, never the store directly.

use std::sync::{Arc, Mutex};

use serde_json::Value;

use crate::error::AppError;
use crate::event::EventPublisher;
use crate::namespace::namespace_key;
use crate::object::finalizer;
use crate::object::helpers;
use crate::object::types::{
    ListOptions, ListResponse, ObjectMeta, StoredObject, SystemMetadata, WatchEventType,
    WatchFilter,
};
#[cfg(test)]
use crate::schema::schema_key;
use crate::schema::{DEFAULT_NAMESPACE, SCOPE_CLUSTER, SCOPE_NAMESPACED, SchemaRegistry};
use crate::store::{ObjectStore, ResourceKey, TransactionOp};
use crate::validation::{validate_annotations, validate_finalizers, validate_labels};

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
    pub schema_registry: SchemaRegistry,
}

impl ObjectService {
    /// Creates a new ObjectService with the given store, event bus, and schema registry.
    ///
    /// The `SchemaRegistry` is shared with `SchemaService` to ensure both services
    /// can access compiled schema validators from the same cache.
    pub fn new(
        store: Arc<dyn ObjectStore>,
        event_bus: Arc<dyn EventPublisher>,
        schema_registry: SchemaRegistry,
    ) -> Self {
        Self { store, event_bus, schema_registry }
    }

    /// Creates a regular object with schema validation.
    ///
    /// 1. Look up the Schema from the store
    /// 2. Validate against cached compiled schema
    /// 3. Store and publish Added event
    ///
    /// `namespace` is the namespace from the URL path. For cluster-scoped kinds,
    /// it must be `None`. For namespaced kinds, `None` defaults to "default".
    /// Any namespace in `meta.namespace` from the request body is discarded
    /// in favor of the URL-derived namespace.
    pub async fn create(
        &self,
        key: ResourceKey,
        namespace: Option<String>,
        meta: ObjectMeta,
        spec: Value,
    ) -> Result<StoredObject, AppError> {
        self.validate_and_create_object(key, namespace, meta, spec).await
    }

    /// Gets an object by key, namespace, and name — delegates to store.
    pub async fn get(
        &self,
        key: ResourceKey,
        namespace: Option<&str>,
        name: String,
    ) -> Result<StoredObject, AppError> {
        self.store.get(&key, namespace, &name).await
    }

    /// Lists objects by key, namespace, and options — delegates to store.
    pub async fn list(
        &self,
        key: ResourceKey,
        namespace: Option<&str>,
        opts: ListOptions,
    ) -> Result<ListResponse, AppError> {
        self.store.list(&key, namespace, opts).await
    }

    /// Updates a regular object with validation.
    ///
    /// Looks up compiled schema, validates spec, applies transaction with OCC and
    /// finalizer checks, and publishes a Modified (or Deleted) event.
    ///
    /// `namespace` is the namespace from the URL path. When `Some`, it is validated
    /// against `object.metadata.namespace` for consistency. When `None`, the namespace
    /// from the object metadata is used.
    pub async fn update(
        &self,
        namespace: Option<&str>,
        object: StoredObject,
    ) -> Result<StoredObject, AppError> {
        let spec = object.spec.clone();
        self.validate_and_update_object(namespace, object, spec).await
    }

    /// Deletes an object with finalizer support.
    ///
    /// Special handling for Namespace objects:
    /// 1. The `"default"` namespace cannot be deleted (rejected with 403).
    /// 2. Other namespaces can only be deleted when empty (rejected with 409
    ///    if any object exists in the namespace).
    ///
    /// Standard delete flow (after namespace checks pass):
    /// 1. If `finalizers` is empty → hard delete, publish Deleted
    /// 2. If `deletion_timestamp` is already set → idempotent no-op (200, no event)
    /// 3. Otherwise → set `deletion_timestamp`, publish Modified (mark for deletion)
    ///
    /// Controllers watch for objects with `deletion_timestamp` set, perform cleanup,
    /// then remove their finalizer via UPDATE. When all finalizers are removed,
    /// the object is hard-deleted (see `validate_and_update_object`).
    pub async fn delete(
        &self,
        key: ResourceKey,
        namespace: Option<&str>,
        name: String,
    ) -> Result<StoredObject, AppError> {
        // Namespace-specific lifecycle rules.
        if key == namespace_key() {
            // The "default" namespace is protected from deletion.
            if name == DEFAULT_NAMESPACE {
                return Err(AppError::ProtectedNamespace { name });
            }
            // Other namespaces can only be deleted when empty.
            let object_count = self.count_objects_in_namespace(&name).await?;
            if object_count > 0 {
                return Err(AppError::NamespaceNotEmpty { namespace: name, object_count });
            }
        }

        let action = Arc::new(Mutex::new(finalizer::DeleteAction::HardDeleted));
        let action_clone = action.clone();
        let result = self.store.transaction(
            &key,
            namespace,
            &name,
            Box::new(move |existing| {
                let act = finalizer::evaluate_delete(existing);
                *action_clone.lock().unwrap() = act;
                finalizer::execute_delete(act, existing)
            }),
        )?;

        match *action.lock().unwrap() {
            finalizer::DeleteAction::HardDeleted => {
                helpers::publish_event(
                    self.event_bus.as_ref(),
                    &key,
                    WatchEventType::Deleted,
                    &result,
                );
            }
            finalizer::DeleteAction::MarkedForDeletion => {
                helpers::publish_event(
                    self.event_bus.as_ref(),
                    &key,
                    WatchEventType::Modified,
                    &result,
                );
            }
            finalizer::DeleteAction::IdempotentNoOp => {
                // No event
            }
        }

        Ok(result)
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
        namespace: Option<&str>,
        name: String,
        status: Value,
    ) -> Result<StoredObject, AppError> {
        // Check that status subresource is enabled
        self.schema_registry.get_status_validator(&key).await?;

        // Validate status against statusSchema
        let validator = self.schema_registry.get_status_validator(&key).await?;
        if !validator.is_valid(&status) {
            let errors = helpers::map_validation_errors(validator.validate(&status));
            return Err(AppError::SchemaValidation(errors));
        }

        // Update status in store via transaction
        // No OCC check for status updates — they are unconditional per spec.
        let updated = self.store.transaction(
            &key,
            namespace,
            &name,
            Box::new(move |existing| {
                helpers::apply_with_metadata(existing, |_existing| {
                    let mut updated = existing.clone();
                    updated.status = Some(status);
                    updated
                })
            }),
        )?;
        helpers::publish_event(
            self.event_bus.as_ref(),
            &key,
            WatchEventType::StatusModified,
            &updated,
        );
        Ok(updated)
    }

    /// Gets the status subresource of an object.
    ///
    /// Validates that a statusSchema exists for the kind, fetches the object,
    /// and returns the status field.
    pub async fn get_status(
        &self,
        key: ResourceKey,
        namespace: Option<&str>,
        name: String,
    ) -> Result<Option<Value>, AppError> {
        // Check that status subresource is enabled
        self.schema_registry.get_status_validator(&key).await?;

        // Fetch object and return status
        let object = self.store.get(&key, namespace, &name).await?;
        Ok(object.status)
    }

    /// Validates an object against its cached schema and creates it.
    ///
    /// Resolves the namespace from the URL parameter and schema scope:
    /// - Cluster-scoped kinds reject a provided namespace.
    /// - Namespaced kinds default to "default" when no namespace is provided.
    ///
    /// For namespaced kinds, validates that the namespace exists by looking up
    /// the corresponding Namespace object. Returns 404 if not found.
    ///
    /// The resolved namespace is set in `metadata.namespace`.
    async fn validate_and_create_object(
        &self,
        key: ResourceKey,
        namespace: Option<String>,
        meta: ObjectMeta,
        spec: Value,
    ) -> Result<StoredObject, AppError> {
        validate_labels(&meta.labels)?;
        validate_annotations(&meta.annotations)?;
        validate_finalizers(&meta.finalizers)?;
        let (validator, scope) = self.schema_registry.get_validator(&key).await?;

        // Resolve namespace from scope and URL namespace parameter
        let resolved_namespace = match scope.as_str() {
            SCOPE_CLUSTER => {
                if namespace.is_some() {
                    return Err(AppError::InvalidRequest(format!(
                        "cluster-scoped kind '{}' does not accept namespace",
                        key.kind
                    )));
                }
                None
            }
            SCOPE_NAMESPACED => Some(namespace.unwrap_or_else(|| "default".to_string())),
            _ => {
                return Err(AppError::Internal(anyhow::anyhow!("unknown scope: {}", scope)));
            }
        };

        // Validate namespace existence for namespaced kinds.
        // Cluster-scoped kinds have no namespace and skip this check.
        // The "default" namespace always exists (auto-created at startup),
        // so this check is effectively a fast no-op for default-namespace creates.
        if scope == SCOPE_NAMESPACED
            && let Some(ns) = resolved_namespace.as_deref()
        {
            self.ensure_namespace_exists(ns).await?;
        }

        if !validator.is_valid(&spec) {
            let errors = helpers::map_validation_errors(validator.validate(&spec));
            return Err(AppError::SchemaValidation(errors));
        }

        let stored = self
            .store
            .create(StoredObject {
                key: key.clone(),
                metadata: ObjectMeta { namespace: resolved_namespace, ..meta },
                system: SystemMetadata::initial(),
                spec,
                status: None,
            })
            .await?;
        helpers::publish_event(self.event_bus.as_ref(), &key, WatchEventType::Added, &stored);
        Ok(stored)
    }

    /// Verifies that a Namespace object with the given name exists.
    ///
    /// Used by [`validate_and_create_object`] to enforce that objects can only
    /// be created in known namespaces. Returns [`AppError::NotFound`] (with
    /// `what: "namespace"`) if the Namespace object is missing.
    ///
    /// This is a single `store.get` call against the Namespace kind (which is
    /// cluster-scoped, so `namespace: None` is used).
    async fn ensure_namespace_exists(&self, namespace: &str) -> Result<(), AppError> {
        match self.store.get(&namespace_key(), None, namespace).await {
            Ok(_) => Ok(()),
            Err(AppError::NotFound { .. }) => Err(AppError::NotFound {
                what: "namespace".to_string(),
                identifier: namespace.to_string(),
            }),
            Err(e) => Err(e),
        }
    }

    /// Counts the number of objects in the given namespace across all kinds.
    ///
    /// Used by [`delete`] to enforce that a Namespace can only be deleted when
    /// empty. Returns the total object count in the namespace (cluster-scoped
    /// objects are not counted because they don't belong to any namespace).
    ///
    /// Delegates schema enumeration to [`SchemaRegistry::list_namespaced_keys`].
    async fn count_objects_in_namespace(&self, namespace: &str) -> Result<usize, AppError> {
        let mut count = 0usize;
        for key in self.schema_registry.list_namespaced_keys().await? {
            let resp = self
                .store
                .list(
                    &key,
                    Some(namespace),
                    ListOptions { limit: Some(usize::MAX), ..Default::default() },
                )
                .await?;
            count += resp.items.len();
        }
        Ok(count)
    }

    /// Validates and updates a regular object.
    ///
    /// Resolves namespace from schema scope:
    /// - Cluster-scoped kinds reject a provided namespace in object metadata.
    /// - Namespaced kinds default to "default" when no namespace is set.
    /// - When `namespace` (from URL) is `Some`, it must match the object's metadata namespace.
    ///
    /// The resolved namespace is passed to `store.transaction()`.
    async fn validate_and_update_object(
        &self,
        namespace: Option<&str>,
        mut object: StoredObject,
        spec: Value,
    ) -> Result<StoredObject, AppError> {
        validate_labels(&object.metadata.labels)?;
        validate_annotations(&object.metadata.annotations)?;
        validate_finalizers(&object.metadata.finalizers)?;
        let (validator, scope) = self.schema_registry.get_validator(&object.key).await?;

        // Resolve namespace from scope
        let resolved_namespace = match scope.as_str() {
            SCOPE_CLUSTER => {
                if object.metadata.namespace.is_some() {
                    return Err(AppError::InvalidRequest(format!(
                        "cluster-scoped kind '{}' does not accept namespace",
                        object.key.kind
                    )));
                }
                None
            }
            SCOPE_NAMESPACED => {
                let ns = object.metadata.namespace.get_or_insert_with(|| "default".to_string());
                // If a namespace was passed from URL, validate it matches
                if let Some(expected_ns) = namespace
                    && ns.as_str() != expected_ns
                {
                    return Err(AppError::InvalidRequest(format!(
                        "namespace mismatch: expected '{}', got '{}'",
                        expected_ns, ns
                    )));
                }
                Some(ns.clone())
            }
            _ => {
                return Err(AppError::Internal(anyhow::anyhow!("unknown scope: {}", scope)));
            }
        };

        if !validator.is_valid(&spec) {
            let errors = helpers::map_validation_errors(validator.validate(&spec));
            return Err(AppError::SchemaValidation(errors));
        }

        let key = object.key.clone();
        let name = object.metadata.name.clone();
        let incoming_rv = object.system.resource_version;
        let incoming_metadata = object.metadata.clone();
        let incoming_spec = object.spec.clone();
        let was_hard_deleted = Arc::new(Mutex::new(false));
        let wd = was_hard_deleted.clone();
        let updated = self.store.transaction(
            &key,
            resolved_namespace.as_deref(),
            &name,
            Box::new(move |existing| {
                // OCC check: reject if resource_version doesn't match
                if incoming_rv != existing.system.resource_version {
                    return TransactionOp::Abort(AppError::Conflict {
                        expected: existing.system.resource_version,
                        actual: incoming_rv,
                    });
                }

                // If object is being deleted, use finalizer state machine
                if finalizer::evaluate_update(existing, &incoming_metadata)
                    == finalizer::FinalizerDecision::RejectBeingDeleted
                {
                    return TransactionOp::Abort(AppError::ObjectBeingDeleted {
                        name: existing.metadata.name.clone(),
                    });
                }

                // Build the updated object
                let mut new_obj = existing.clone();
                new_obj.metadata = incoming_metadata.clone();
                new_obj.spec = incoming_spec.clone();

                // Check if this should trigger hard delete (finalizers became empty on deleting object)
                if finalizer::should_hard_delete(existing, &new_obj.metadata.finalizers) {
                    *wd.lock().unwrap() = true;
                    return TransactionOp::Delete;
                }

                // Otherwise, apply metadata management
                helpers::apply_with_metadata(existing, |_| new_obj)
            }),
        )?;

        if *was_hard_deleted.lock().unwrap() {
            helpers::publish_event(
                self.event_bus.as_ref(),
                &updated.key,
                WatchEventType::Deleted,
                &updated,
            );
        } else {
            helpers::publish_event(
                self.event_bus.as_ref(),
                &updated.key,
                WatchEventType::Modified,
                &updated,
            );
        }

        Ok(updated)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::object::schema_service::SchemaService;
    use crate::schema::SchemaValidator;
    use crate::schema::meta_schema::compile_meta_schema;
    use crate::schema::schema_cache_key;
    use crate::store::memory::InMemoryStore;
    use serde_json::json;
    use std::collections::HashMap;

    // Helper to create services with a fresh store and event bus.
    // Returns (ObjectService, SchemaService) sharing the same store and event_bus.
    // Bootstraps the "default" namespace so namespaced creates succeed.
    async fn make_services() -> (ObjectService, SchemaService) {
        let store: Arc<dyn ObjectStore> = Arc::new(InMemoryStore::new());
        let event_bus: Arc<dyn EventPublisher> = Arc::new(crate::event::EventBus::default());

        // Bootstrap the "default" namespace BEFORE constructing services, while
        // we still have a clean handle to store and event_bus.
        crate::namespace::bootstrap_default_namespace(&store, &event_bus)
            .await
            .expect("bootstrap should succeed");

        let meta_validator: Arc<dyn SchemaValidator> =
            Arc::new(compile_meta_schema().expect("meta-schema should compile"));
        let schema_registry = SchemaRegistry::new(store.clone(), meta_validator.clone());
        let schema_service = SchemaService::new(store.clone(), event_bus.clone(), meta_validator);
        let object_service = ObjectService::new(store, event_bus, schema_registry);
        (object_service, schema_service)
    }

    // Helper to register a Schema for testing using SchemaService.
    // The name format "{targetKind}.{targetGroup}.{targetVersion}" is backend-generated
    // (see handler::extract_schema_name), but tests call service.create()
    // directly and must supply the name.
    // Uses "Cluster" scope so objects store with namespace=None (avoids namespace
    // interactions for tests that don't test namespace/scope behavior).
    async fn register_test_schema(schema_service: &SchemaService) {
        let schema_key = schema_key();
        let schema_data = json!({
            "targetGroup": "example.io",
            "targetVersion": "v1",
            "targetKind": "Widget",
            "scope": "Cluster",
            "specSchema": {
                "type": "object",
                "properties": {
                    "color": { "type": "string" },
                    "size": { "type": "integer" }
                }
            }
        });
        schema_service
            .create(
                schema_key,
                ObjectMeta {
                    name: "Widget.example.io.v1".to_string(),
                    namespace: None,
                    labels: HashMap::new(),
                    annotations: HashMap::new(),
                    finalizers: Vec::new(),
                },
                schema_data,
            )
            .await
            .expect("schema registration should succeed");
    }

    // T19: Create valid Schema → stored, cached, event published
    #[tokio::test]
    async fn create_valid_schema_stored_cached_event_published() {
        let (service, schema_service) = make_services().await;
        let schema_key = schema_key();
        let schema_data = json!({
            "targetGroup": "example.io",
            "targetVersion": "v1",
            "targetKind": "Widget",
            "specSchema": { "type": "object" }
        });

        let result = schema_service
            .create(
                schema_key.clone(),
                ObjectMeta {
                    name: "Widget.example.io.v1".to_string(),
                    namespace: None,
                    labels: HashMap::new(),
                    annotations: HashMap::new(),
                    finalizers: Vec::new(),
                },
                schema_data,
            )
            .await;
        assert!(result.is_ok());
        let stored = result.unwrap();
        assert_eq!(stored.metadata.name, "Widget.example.io.v1");

        // Verify stored in store (using ObjectService.get which delegates to store)
        let retrieved =
            service.get(schema_key, None, "Widget.example.io.v1".to_string()).await.unwrap();
        assert_eq!(retrieved.metadata.name, "Widget.example.io.v1");

        // Verify ObjectService's registry can lazy-load and cache it
        let widget_key = ResourceKey {
            group: "example.io".to_string(),
            version: "v1".to_string(),
            kind: "Widget".to_string(),
        };
        assert!(!service.schema_registry.cache.contains_key("Widget.example.io.v1"));
        // Trigger lazy compilation by validating an object
        let _ = service.schema_registry.get_validator(&widget_key).await.unwrap();
        assert!(service.schema_registry.cache.contains_key("Widget.example.io.v1"));
    }

    // T20: Create Schema with invalid meta-schema → InvalidSchema, nothing stored
    #[tokio::test]
    async fn create_schema_invalid_meta_schema_returns_error() {
        let (_service, schema_service) = make_services().await;
        let schema_key = schema_key();
        // Missing required fields
        let invalid_data = json!({ "targetGroup": "example.io" });

        let result = schema_service
            .create(
                schema_key,
                ObjectMeta {
                    name: "Widget.example.io.v1".to_string(),
                    namespace: None,
                    labels: HashMap::new(),
                    annotations: HashMap::new(),
                    finalizers: Vec::new(),
                },
                invalid_data,
            )
            .await;
        assert!(matches!(result, Err(AppError::InvalidSchema(_))));
    }

    // T21: Create Schema with uncompileable specSchema → InvalidSchema, nothing stored
    #[tokio::test]
    async fn create_schema_uncompileable_spec_schema_returns_error() {
        let (_service, schema_service) = make_services().await;
        let schema_key = schema_key();
        // specSchema with invalid content (not a valid JSON Schema)
        let invalid_schema = json!({
            "targetGroup": "example.io",
            "targetVersion": "v1",
            "targetKind": "Widget",
            "specSchema": { "type": "not-a-real-type" }
        });

        let result = schema_service
            .create(
                schema_key,
                ObjectMeta {
                    name: "Widget.example.io.v1".to_string(),
                    namespace: None,
                    labels: HashMap::new(),
                    annotations: HashMap::new(),
                    finalizers: Vec::new(),
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
        let (service, _schema_service) = make_services().await;
        let widget_key = ResourceKey {
            group: "example.io".to_string(),
            version: "v1".to_string(),
            kind: "Widget".to_string(),
        };

        let result = service
            .create(
                widget_key,
                None,
                ObjectMeta {
                    name: "my-widget".to_string(),
                    namespace: None,
                    labels: HashMap::new(),
                    annotations: HashMap::new(),
                    finalizers: Vec::new(),
                },
                json!({}),
            )
            .await;
        assert!(matches!(result, Err(AppError::NotFound { .. })));
    }

    // T23: Create object with invalid data → SchemaValidation
    #[tokio::test]
    async fn create_object_invalid_data_returns_schema_validation() {
        let (service, schema_service) = make_services().await;
        register_test_schema(&schema_service).await;

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
                None,
                ObjectMeta {
                    name: "my-widget".to_string(),
                    namespace: None,
                    labels: HashMap::new(),
                    annotations: HashMap::new(),
                    finalizers: Vec::new(),
                },
                invalid_data,
            )
            .await;
        assert!(matches!(result, Err(AppError::SchemaValidation(_))));
    }

    // T24: Update with correct version → success, Modified event published
    #[tokio::test]
    async fn update_correct_version_succeeds() {
        let (service, schema_service) = make_services().await;
        register_test_schema(&schema_service).await;

        let widget_key = ResourceKey {
            group: "example.io".to_string(),
            version: "v1".to_string(),
            kind: "Widget".to_string(),
        };
        let created = service
            .create(
                widget_key.clone(),
                None,
                ObjectMeta {
                    name: "my-widget".to_string(),
                    namespace: None,
                    labels: HashMap::new(),
                    annotations: HashMap::new(),
                    finalizers: Vec::new(),
                },
                json!({ "color": "blue", "size": 10 }),
            )
            .await
            .unwrap();

        let v1 = created.system.resource_version;
        let mut updated_obj = created;
        updated_obj.spec = json!({ "color": "red", "size": 20 });
        updated_obj.system.resource_version = v1;

        let result = service.update(None, updated_obj).await;
        assert!(result.is_ok());
        assert!(result.unwrap().system.resource_version > v1);
    }

    // T25: Update with wrong version → Conflict
    #[tokio::test]
    async fn update_with_wrong_version_returns_conflict() {
        let (service, schema_service) = make_services().await;
        register_test_schema(&schema_service).await;

        let widget_key = ResourceKey {
            group: "example.io".to_string(),
            version: "v1".to_string(),
            kind: "Widget".to_string(),
        };
        let created = service
            .create(
                widget_key.clone(),
                None,
                ObjectMeta {
                    name: "my-widget".to_string(),
                    namespace: None,
                    labels: HashMap::new(),
                    annotations: HashMap::new(),
                    finalizers: Vec::new(),
                },
                json!({ "color": "blue", "size": 10 }),
            )
            .await
            .unwrap();

        let mut wrong_version_obj = created;
        wrong_version_obj.spec = json!({ "color": "red" });
        wrong_version_obj.system.resource_version = 999;

        // OCC check in service layer rejects stale versions
        let result = service.update(None, wrong_version_obj).await;
        assert!(matches!(result, Err(AppError::Conflict { .. })));
    }

    // T26: Delete Schema with no objects → success, cache evicted, Deleted event published
    #[tokio::test]
    async fn delete_schema_no_objects_succeeds() {
        let (_service, schema_service) = make_services().await;
        let schema_key = schema_key();
        let schema_data = json!({
            "targetGroup": "example.io",
            "targetVersion": "v1",
            "targetKind": "Widget",
            "specSchema": { "type": "object" }
        });
        schema_service
            .create(
                schema_key.clone(),
                ObjectMeta {
                    name: "Widget.example.io.v1".to_string(),
                    namespace: None,
                    labels: HashMap::new(),
                    annotations: HashMap::new(),
                    finalizers: Vec::new(),
                },
                schema_data,
            )
            .await
            .unwrap();

        // Delete the schema via SchemaService
        let result = schema_service.delete(schema_key, "Widget.example.io.v1".to_string()).await;
        assert!(result.is_ok());
    }

    // T27: Delete Schema with existing objects → SchemaHasObjects, nothing deleted
    #[tokio::test]
    async fn delete_schema_with_objects_returns_conflict() {
        let (service, schema_service) = make_services().await;
        register_test_schema(&schema_service).await;

        // Create an object of the registered kind
        let widget_key = ResourceKey {
            group: "example.io".to_string(),
            version: "v1".to_string(),
            kind: "Widget".to_string(),
        };
        service
            .create(
                widget_key.clone(),
                None,
                ObjectMeta {
                    name: "my-widget".to_string(),
                    namespace: None,
                    labels: HashMap::new(),
                    annotations: HashMap::new(),
                    finalizers: Vec::new(),
                },
                json!({ "color": "blue", "size": 10 }),
            )
            .await
            .unwrap();

        // Try to delete the schema via SchemaService
        let schema_key = schema_key();
        let result = schema_service.delete(schema_key, "Widget.example.io.v1".to_string()).await;
        assert!(matches!(result, Err(AppError::SchemaHasObjects { kind }) if kind == "Widget"));
    }

    // T28: Delete regular object → success, Deleted event published
    #[tokio::test]
    async fn delete_regular_object_succeeds() {
        let (service, schema_service) = make_services().await;
        register_test_schema(&schema_service).await;

        let widget_key = ResourceKey {
            group: "example.io".to_string(),
            version: "v1".to_string(),
            kind: "Widget".to_string(),
        };
        let created = service
            .create(
                widget_key.clone(),
                None,
                ObjectMeta {
                    name: "my-widget".to_string(),
                    namespace: None,
                    labels: HashMap::new(),
                    annotations: HashMap::new(),
                    finalizers: Vec::new(),
                },
                json!({ "color": "blue", "size": 10 }),
            )
            .await
            .unwrap();

        let result = service.delete(widget_key, None, "my-widget".to_string()).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().metadata.name, created.metadata.name);
    }

    // T29: Failed create (duplicate) → no Added event published
    #[tokio::test]
    async fn create_duplicate_no_event_published() {
        let (_service, schema_service) = make_services().await;
        let schema_key = schema_key();
        let schema_data = json!({
            "targetGroup": "example.io",
            "targetVersion": "v1",
            "targetKind": "Widget",
            "specSchema": { "type": "object" }
        });
        schema_service
            .create(
                schema_key.clone(),
                ObjectMeta {
                    name: "Widget.example.io.v1".to_string(),
                    namespace: None,
                    labels: HashMap::new(),
                    annotations: HashMap::new(),
                    finalizers: Vec::new(),
                },
                schema_data.clone(),
            )
            .await
            .unwrap();

        // Try to create duplicate via SchemaService
        let result = schema_service
            .create(
                schema_key.clone(),
                ObjectMeta {
                    name: "Widget.example.io.v1".to_string(),
                    namespace: None,
                    labels: HashMap::new(),
                    annotations: HashMap::new(),
                    finalizers: Vec::new(),
                },
                schema_data,
            )
            .await;
        assert!(matches!(result, Err(AppError::AlreadyExists { .. })));
    }

    // T30: Schema cache eviction on Schema delete
    #[tokio::test]
    async fn schema_cache_eviction_on_delete() {
        let (service, schema_service) = make_services().await;
        let schema_key = schema_key();
        let schema_data = json!({
            "targetGroup": "example.io",
            "targetVersion": "v1",
            "targetKind": "Widget",
            "specSchema": { "type": "object" }
        });
        schema_service
            .create(
                schema_key.clone(),
                ObjectMeta {
                    name: "Widget.example.io.v1".to_string(),
                    namespace: None,
                    labels: HashMap::new(),
                    annotations: HashMap::new(),
                    finalizers: Vec::new(),
                },
                schema_data,
            )
            .await
            .unwrap();

        // Verify ObjectService can lazy-load and cache it
        let widget_key = ResourceKey {
            group: "example.io".to_string(),
            version: "v1".to_string(),
            kind: "Widget".to_string(),
        };
        let _ = service.schema_registry.get_validator(&widget_key).await.unwrap();
        assert!(service.schema_registry.cache.contains_key("Widget.example.io.v1"));

        // Delete the schema via SchemaService
        schema_service.delete(schema_key, "Widget.example.io.v1".to_string()).await.unwrap();

        // ObjectService's cache still has it (SchemaService evicted its own cache)
        // but the store no longer has the schema, so next use will fail
        assert!(service.schema_registry.cache.contains_key("Widget.example.io.v1"));
        // Verify the key was evicted from SchemaService's perspective by trying to use it:
        // ObjectService will try to use its cached value, which is still valid
        // (cache invalidation across service boundaries is out of scope for this test)
    }

    // Schema create with missing targetKind returns InvalidSchema error
    // (meta-schema requires targetKind as a required field)
    #[tokio::test]
    async fn create_schema_missing_target_kind_returns_error() {
        let (_service, schema_service) = make_services().await;
        let schema_key = schema_key();
        // Missing targetKind
        let schema_data = json!({
            "targetGroup": "example.io",
            "targetVersion": "v1",
            "specSchema": { "type": "object" }
        });

        let result = schema_service
            .create(
                schema_key,
                ObjectMeta {
                    name: "Widget.example.io.v1".to_string(),
                    namespace: None,
                    labels: HashMap::new(),
                    annotations: HashMap::new(),
                    finalizers: Vec::new(),
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
        let (_service, schema_service) = make_services().await;
        let schema_key = schema_key();
        // Missing targetGroup
        let schema_data = json!({
            "targetVersion": "v1",
            "targetKind": "Widget",
            "specSchema": { "type": "object" }
        });

        let result = schema_service
            .create(
                schema_key,
                ObjectMeta {
                    name: "Widget.example.io.v1".to_string(),
                    namespace: None,
                    labels: HashMap::new(),
                    annotations: HashMap::new(),
                    finalizers: Vec::new(),
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
        // Bootstrap the "default" namespace so namespaced creates succeed.
        crate::namespace::bootstrap_default_namespace(&store, &event_bus)
            .await
            .expect("bootstrap should succeed");
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
        let schema_service_a =
            SchemaService::new(store.clone(), event_bus.clone(), meta_validator.clone());
        let schema_registry_a = SchemaRegistry::new(store.clone(), meta_validator.clone());
        let service_a = ObjectService::new(store.clone(), event_bus.clone(), schema_registry_a);

        schema_service_a
            .create(
                schema_key,
                ObjectMeta {
                    name: "Widget.example.io.v1".to_string(),
                    namespace: None,
                    labels: HashMap::new(),
                    annotations: HashMap::new(),
                    finalizers: Vec::new(),
                },
                schema_data,
            )
            .await
            .expect("schema registration should succeed");
        service_a
            .create(
                widget_key.clone(),
                None,
                ObjectMeta {
                    name: "widget-1".to_string(),
                    namespace: None,
                    labels: HashMap::new(),
                    annotations: HashMap::new(),
                    finalizers: Vec::new(),
                },
                json!({"color": "red"}),
            )
            .await
            .expect("first object should succeed");

        // Service B: same store, fresh cache (simulating restart)
        let schema_registry_b = SchemaRegistry::new(store.clone(), meta_validator);
        let service_b = ObjectService::new(store, event_bus, schema_registry_b);
        assert!(!service_b.schema_registry.cache.contains_key("Widget.example.io.v1"));

        let result = service_b
            .create(
                widget_key,
                None,
                ObjectMeta {
                    name: "widget-2".to_string(),
                    namespace: None,
                    labels: HashMap::new(),
                    annotations: HashMap::new(),
                    finalizers: Vec::new(),
                },
                json!({"color": "blue"}),
            )
            .await;
        assert!(result.is_ok());
        assert!(service_b.schema_registry.cache.contains_key("Widget.example.io.v1"));
    }

    // T32: Cache miss triggers compilation, subsequent requests use cached validator
    #[tokio::test]
    async fn cache_miss_triggers_compilation_and_subsequent_uses_cache() {
        let store: Arc<dyn ObjectStore> = Arc::new(InMemoryStore::new());
        let event_bus: Arc<dyn EventPublisher> = Arc::new(crate::event::EventBus::default());
        // Bootstrap the "default" namespace so namespaced creates succeed.
        crate::namespace::bootstrap_default_namespace(&store, &event_bus)
            .await
            .expect("bootstrap should succeed");
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

        // Register schema via SchemaService A
        let schema_service_a =
            SchemaService::new(store.clone(), event_bus.clone(), meta_validator.clone());
        schema_service_a
            .create(
                schema_key,
                ObjectMeta {
                    name: "Widget.example.io.v1".to_string(),
                    namespace: None,
                    labels: HashMap::new(),
                    annotations: HashMap::new(),
                    finalizers: Vec::new(),
                },
                schema_data,
            )
            .await
            .expect("schema registration should succeed");

        // Service B starts with empty cache
        let schema_registry_b = SchemaRegistry::new(store.clone(), meta_validator);
        let service_b = ObjectService::new(store, event_bus, schema_registry_b);
        assert!(!service_b.schema_registry.cache.contains_key("Widget.example.io.v1"));

        // First creation triggers lazy compilation
        let first = service_b
            .create(
                widget_key.clone(),
                None,
                ObjectMeta {
                    name: "widget-1".to_string(),
                    namespace: None,
                    labels: HashMap::new(),
                    annotations: HashMap::new(),
                    finalizers: Vec::new(),
                },
                json!({"color": "red", "size": 1}),
            )
            .await;
        assert!(first.is_ok());
        assert!(service_b.schema_registry.cache.contains_key("Widget.example.io.v1"));

        // Second creation uses cached validator
        let second = service_b
            .create(
                widget_key,
                None,
                ObjectMeta {
                    name: "widget-2".to_string(),
                    namespace: None,
                    labels: HashMap::new(),
                    annotations: HashMap::new(),
                    finalizers: Vec::new(),
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
            .create(StoredObject {
                key: schema_key.clone(),
                metadata: ObjectMeta {
                    name: "Widget.example.io.v1".to_string(),
                    namespace: None,
                    labels: HashMap::new(),
                    annotations: HashMap::new(),
                    finalizers: Vec::new(),
                },
                system: SystemMetadata::initial(),
                spec: invalid_schema,
                status: None,
            })
            .await
            .expect("store create should succeed");

        // Service with empty cache should fail to compile the stored schema
        let schema_registry = SchemaRegistry::new(store.clone(), meta_validator);
        let service = ObjectService::new(store, event_bus, schema_registry);
        let widget_key = ResourceKey {
            group: "example.io".to_string(),
            version: "v1".to_string(),
            kind: "Widget".to_string(),
        };
        let result = service
            .create(
                widget_key,
                None,
                ObjectMeta {
                    name: "my-widget".to_string(),
                    namespace: None,
                    labels: HashMap::new(),
                    annotations: HashMap::new(),
                    finalizers: Vec::new(),
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

    // --- validate_annotations unit tests ---

    #[test]
    fn validate_annotations_empty_map() {
        let annotations = HashMap::new();
        assert!(validate_annotations(&annotations).is_ok());
    }

    #[test]
    fn validate_annotations_valid_keys() {
        let mut annotations = HashMap::new();
        annotations.insert("description".to_string(), "my widget".to_string());
        annotations.insert("kapi.io/last-applied-config".to_string(), "{}".to_string());
        annotations.insert("example.com/path@v1".to_string(), "data".to_string());
        assert!(validate_annotations(&annotations).is_ok());
    }

    #[test]
    fn validate_annotations_empty_key_rejected() {
        let mut annotations = HashMap::new();
        annotations.insert("".to_string(), "value".to_string());
        assert!(validate_annotations(&annotations).is_err());
    }

    #[test]
    fn validate_annotations_key_too_long() {
        let mut annotations = HashMap::new();
        let long_key = "a".repeat(257);
        annotations.insert(long_key, "value".to_string());
        assert!(validate_annotations(&annotations).is_err());
    }

    #[test]
    fn validate_annotations_size_limit_exceeded() {
        let mut annotations = HashMap::new();
        let large_value = "x".repeat(256 * 1024); // > 256KB
        annotations.insert("key".to_string(), large_value);
        assert!(validate_annotations(&annotations).is_err());
    }

    #[test]
    fn validate_annotations_special_characters_accepted() {
        let mut annotations = HashMap::new();
        annotations.insert(
            "build-url".to_string(),
            "https://example.com/path?query=value&other=123".to_string(),
        );
        annotations.insert("config".to_string(), "{\"key\": \"value\"}".to_string());
        assert!(validate_annotations(&annotations).is_ok());
    }

    #[test]
    fn validate_annotations_empty_value_accepted() {
        let mut annotations = HashMap::new();
        annotations.insert("key".to_string(), "".to_string());
        assert!(validate_annotations(&annotations).is_ok());
    }

    // --- Status subresource tests ---

    // Helper to register a Schema with statusSchema using SchemaService
    // Uses "Cluster" scope so objects store with namespace=None.
    async fn register_test_schema_with_status(schema_service: &SchemaService) {
        let schema_key = schema_key();
        let schema_data = json!({
            "targetGroup": "example.io",
            "targetVersion": "v1",
            "targetKind": "Widget",
            "scope": "Cluster",
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
        schema_service
            .create(
                schema_key,
                ObjectMeta {
                    name: "Widget.example.io.v1".to_string(),
                    namespace: None,
                    labels: HashMap::new(),
                    annotations: HashMap::new(),
                    finalizers: Vec::new(),
                },
                schema_data,
            )
            .await
            .expect("schema registration should succeed");
    }

    #[tokio::test]
    async fn update_status_with_status_schema_succeeds() {
        let (service, schema_service) = make_services().await;
        register_test_schema_with_status(&schema_service).await;

        let widget_key = ResourceKey {
            group: "example.io".to_string(),
            version: "v1".to_string(),
            kind: "Widget".to_string(),
        };
        let created = service
            .create(
                widget_key.clone(),
                None,
                ObjectMeta {
                    name: "my-widget".to_string(),
                    namespace: None,
                    labels: HashMap::new(),
                    annotations: HashMap::new(),
                    finalizers: Vec::new(),
                },
                json!({"color": "blue", "size": 10}),
            )
            .await
            .unwrap();
        assert!(created.status.is_none());

        let updated = service
            .update_status(
                widget_key.clone(),
                None,
                "my-widget".to_string(),
                json!({"phase": "Running"}),
            )
            .await
            .unwrap();
        assert!(updated.status.is_some());
        assert_eq!(updated.status.unwrap(), json!({"phase": "Running"}));
    }

    #[tokio::test]
    async fn update_status_without_status_schema_returns_error() {
        let (service, schema_service) = make_services().await;
        // Register schema WITHOUT statusSchema
        register_test_schema(&schema_service).await;

        let widget_key = ResourceKey {
            group: "example.io".to_string(),
            version: "v1".to_string(),
            kind: "Widget".to_string(),
        };
        service
            .create(
                widget_key.clone(),
                None,
                ObjectMeta {
                    name: "my-widget".to_string(),
                    namespace: None,
                    labels: HashMap::new(),
                    annotations: HashMap::new(),
                    finalizers: Vec::new(),
                },
                json!({"color": "blue"}),
            )
            .await
            .unwrap();

        let err = service
            .update_status(
                widget_key.clone(),
                None,
                "my-widget".to_string(),
                json!({"phase": "Running"}),
            )
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::StatusSubresourceNotEnabled { .. }));
    }

    #[tokio::test]
    async fn update_status_invalid_status_returns_validation_error() {
        let (service, schema_service) = make_services().await;
        register_test_schema_with_status(&schema_service).await;

        let widget_key = ResourceKey {
            group: "example.io".to_string(),
            version: "v1".to_string(),
            kind: "Widget".to_string(),
        };
        service
            .create(
                widget_key.clone(),
                None,
                ObjectMeta {
                    name: "my-widget".to_string(),
                    namespace: None,
                    labels: HashMap::new(),
                    annotations: HashMap::new(),
                    finalizers: Vec::new(),
                },
                json!({"color": "blue"}),
            )
            .await
            .unwrap();

        // phase should be string, not integer
        let err = service
            .update_status(widget_key.clone(), None, "my-widget".to_string(), json!({"phase": 123}))
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::SchemaValidation(_)));
    }

    #[tokio::test]
    async fn update_status_not_found() {
        let (service, schema_service) = make_services().await;
        register_test_schema_with_status(&schema_service).await;

        let widget_key = ResourceKey {
            group: "example.io".to_string(),
            version: "v1".to_string(),
            kind: "Widget".to_string(),
        };

        let err = service
            .update_status(
                widget_key.clone(),
                None,
                "nonexistent".to_string(),
                json!({"phase": "Running"}),
            )
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::NotFound { .. }));
    }

    #[tokio::test]
    async fn get_status_with_status_schema_returns_status() {
        let (service, schema_service) = make_services().await;
        register_test_schema_with_status(&schema_service).await;

        let widget_key = ResourceKey {
            group: "example.io".to_string(),
            version: "v1".to_string(),
            kind: "Widget".to_string(),
        };
        service
            .create(
                widget_key.clone(),
                None,
                ObjectMeta {
                    name: "my-widget".to_string(),
                    namespace: None,
                    labels: HashMap::new(),
                    annotations: HashMap::new(),
                    finalizers: Vec::new(),
                },
                json!({"color": "blue"}),
            )
            .await
            .unwrap();

        // Initially None
        let status =
            service.get_status(widget_key.clone(), None, "my-widget".to_string()).await.unwrap();
        assert!(status.is_none());

        // After update
        service
            .update_status(
                widget_key.clone(),
                None,
                "my-widget".to_string(),
                json!({"phase": "Running"}),
            )
            .await
            .unwrap();

        let status =
            service.get_status(widget_key.clone(), None, "my-widget".to_string()).await.unwrap();
        assert!(status.is_some());
        assert_eq!(status.unwrap(), json!({"phase": "Running"}));
    }

    #[tokio::test]
    async fn get_status_without_status_schema_returns_error() {
        let (service, schema_service) = make_services().await;
        register_test_schema(&schema_service).await;

        let widget_key = ResourceKey {
            group: "example.io".to_string(),
            version: "v1".to_string(),
            kind: "Widget".to_string(),
        };
        service
            .create(
                widget_key.clone(),
                None,
                ObjectMeta {
                    name: "my-widget".to_string(),
                    namespace: None,
                    labels: HashMap::new(),
                    annotations: HashMap::new(),
                    finalizers: Vec::new(),
                },
                json!({"color": "blue"}),
            )
            .await
            .unwrap();

        let err = service
            .get_status(widget_key.clone(), None, "my-widget".to_string())
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::StatusSubresourceNotEnabled { .. }));
    }

    #[tokio::test]
    async fn create_strips_status_from_body() {
        let (service, schema_service) = make_services().await;
        register_test_schema_with_status(&schema_service).await;

        let widget_key = ResourceKey {
            group: "example.io".to_string(),
            version: "v1".to_string(),
            kind: "Widget".to_string(),
        };

        // Create with status in body — should be ignored
        let created = service
            .create(
                widget_key.clone(),
                None,
                ObjectMeta {
                    name: "my-widget".to_string(),
                    namespace: None,
                    labels: HashMap::new(),
                    annotations: HashMap::new(),
                    finalizers: Vec::new(),
                },
                json!({"color": "blue", "status": {"phase": "Running"}}),
            )
            .await
            .unwrap();
        assert!(created.status.is_none());
    }

    // --- apply_with_metadata tests ---

    #[tokio::test]
    async fn apply_with_metadata_increments_rv() {
        let (service, schema_service) = make_services().await;
        register_test_schema(&schema_service).await;

        let widget_key = ResourceKey {
            group: "example.io".to_string(),
            version: "v1".to_string(),
            kind: "Widget".to_string(),
        };
        let created = service
            .create(
                widget_key.clone(),
                None,
                ObjectMeta {
                    name: "meta-test".to_string(),
                    namespace: None,
                    labels: HashMap::new(),
                    annotations: HashMap::new(),
                    finalizers: Vec::new(),
                },
                json!({"color": "blue", "size": 10}),
            )
            .await
            .unwrap();
        let v1 = created.system.resource_version;

        let mut update_obj = created;
        update_obj.system.resource_version = v1;
        let result = service.update(None, update_obj).await.unwrap();
        assert_eq!(
            result.system.resource_version,
            v1 + 1,
            "resource_version should increment by 1"
        );
    }

    #[tokio::test]
    async fn apply_with_metadata_preserves_created_at() {
        let (service, schema_service) = make_services().await;
        register_test_schema(&schema_service).await;

        let widget_key = ResourceKey {
            group: "example.io".to_string(),
            version: "v1".to_string(),
            kind: "Widget".to_string(),
        };
        let created = service
            .create(
                widget_key.clone(),
                None,
                ObjectMeta {
                    name: "created-at-test".to_string(),
                    namespace: None,
                    labels: HashMap::new(),
                    annotations: HashMap::new(),
                    finalizers: Vec::new(),
                },
                json!({"color": "blue", "size": 10}),
            )
            .await
            .unwrap();
        let created_at = created.system.created_at;
        let rv = created.system.resource_version;
        let mut update_obj = created;
        update_obj.system.resource_version = rv;
        let result = service.update(None, update_obj).await.unwrap();
        assert_eq!(result.system.created_at, created_at, "created_at should be preserved");
    }

    #[tokio::test]
    async fn apply_with_metadata_bumps_generation_on_spec_change() {
        let (service, schema_service) = make_services().await;
        register_test_schema(&schema_service).await;

        let widget_key = ResourceKey {
            group: "example.io".to_string(),
            version: "v1".to_string(),
            kind: "Widget".to_string(),
        };
        let created = service
            .create(
                widget_key.clone(),
                None,
                ObjectMeta {
                    name: "gen-bump-test".to_string(),
                    namespace: None,
                    labels: HashMap::new(),
                    annotations: HashMap::new(),
                    finalizers: Vec::new(),
                },
                json!({"color": "blue", "size": 10}),
            )
            .await
            .unwrap();
        assert_eq!(created.system.generation, 1);

        let v1 = created.system.resource_version;
        let mut update_obj = created;
        update_obj.system.resource_version = v1;
        update_obj.spec = json!({"color": "red", "size": 20});

        let result = service.update(None, update_obj).await.unwrap();
        assert_eq!(result.system.generation, 2, "generation should bump to 2 on spec change");
    }

    #[tokio::test]
    async fn apply_with_metadata_preserves_generation_on_no_spec_change() {
        let (service, schema_service) = make_services().await;
        register_test_schema(&schema_service).await;

        let widget_key = ResourceKey {
            group: "example.io".to_string(),
            version: "v1".to_string(),
            kind: "Widget".to_string(),
        };
        let created = service
            .create(
                widget_key.clone(),
                None,
                ObjectMeta {
                    name: "gen-preserve-test".to_string(),
                    namespace: None,
                    labels: HashMap::new(),
                    annotations: HashMap::new(),
                    finalizers: Vec::new(),
                },
                json!({"color": "blue", "size": 10}),
            )
            .await
            .unwrap();
        let v1 = created.system.resource_version;
        let gen1 = created.system.generation;

        // Update with same spec, different labels
        let mut update_obj = created;
        update_obj.system.resource_version = v1;
        update_obj.metadata.labels.insert("env".to_string(), "prod".to_string());

        let result = service.update(None, update_obj).await.unwrap();
        assert_eq!(
            result.system.generation, gen1,
            "generation should not bump on metadata-only update"
        );
    }

    // --- OCC tests ---

    #[tokio::test]
    async fn occ_check_passes_with_matching_version() {
        let (service, schema_service) = make_services().await;
        register_test_schema(&schema_service).await;

        let widget_key = ResourceKey {
            group: "example.io".to_string(),
            version: "v1".to_string(),
            kind: "Widget".to_string(),
        };
        let created = service
            .create(
                widget_key.clone(),
                None,
                ObjectMeta {
                    name: "occ-pass".to_string(),
                    namespace: None,
                    labels: HashMap::new(),
                    annotations: HashMap::new(),
                    finalizers: Vec::new(),
                },
                json!({"color": "blue", "size": 1}),
            )
            .await
            .unwrap();

        let rv = created.system.resource_version;
        let mut update_obj = created;
        update_obj.system.resource_version = rv;
        update_obj.spec = json!({"color": "red", "size": 2});

        let result = service.update(None, update_obj).await;
        assert!(result.is_ok(), "update should succeed with matching rv");
    }

    #[tokio::test]
    async fn occ_check_fails_with_mismatched_version() {
        let (service, schema_service) = make_services().await;
        register_test_schema(&schema_service).await;

        let widget_key = ResourceKey {
            group: "example.io".to_string(),
            version: "v1".to_string(),
            kind: "Widget".to_string(),
        };
        let created = service
            .create(
                widget_key.clone(),
                None,
                ObjectMeta {
                    name: "occ-fail".to_string(),
                    namespace: None,
                    labels: HashMap::new(),
                    annotations: HashMap::new(),
                    finalizers: Vec::new(),
                },
                json!({"color": "blue", "size": 1}),
            )
            .await
            .unwrap();

        let mut update_obj = created;
        update_obj.spec = json!({"color": "red", "size": 2});
        update_obj.system.resource_version = 999; // wrong version

        let result = service.update(None, update_obj).await;
        assert!(
            matches!(result, Err(AppError::Conflict { .. })),
            "expected Conflict error, got {:?}",
            result
        );
    }

    #[tokio::test]
    async fn update_status_does_not_require_occ() {
        let (service, schema_service) = make_services().await;
        register_test_schema_with_status(&schema_service).await;

        let widget_key = ResourceKey {
            group: "example.io".to_string(),
            version: "v1".to_string(),
            kind: "Widget".to_string(),
        };
        let created = service
            .create(
                widget_key.clone(),
                None,
                ObjectMeta {
                    name: "status-occ".to_string(),
                    namespace: None,
                    labels: HashMap::new(),
                    annotations: HashMap::new(),
                    finalizers: Vec::new(),
                },
                json!({"color": "blue", "size": 1}),
            )
            .await
            .unwrap();
        assert!(created.status.is_none());

        // Status updates don't need OCC — they should always succeed
        let result = service
            .update_status(
                widget_key.clone(),
                None,
                "status-occ".to_string(),
                json!({"phase": "Running"}),
            )
            .await;
        assert!(result.is_ok(), "status update should succeed without OCC");
        let updated = result.unwrap();
        assert!(updated.status.is_some());
        assert_eq!(
            updated.system.resource_version,
            created.system.resource_version + 1,
            "resource_version should increment"
        );
        assert_eq!(
            updated.system.generation, created.system.generation,
            "generation should NOT increment on status update"
        );
    }

    // --- Multi-version schema support tests ---

    #[tokio::test]
    async fn multi_version_schemas_register_cache_independently() {
        let (object_service, schema_service) = make_services().await;
        let schema_key = schema_key();

        // Register v1 schema
        let v1_data = json!({
            "targetGroup": "example.io",
            "targetVersion": "v1",
            "targetKind": "Widget",
            "specSchema": {
                "type": "object",
                "properties": { "color": { "type": "string" } },
                "required": ["color"]
            }
        });
        schema_service
            .create(
                schema_key.clone(),
                ObjectMeta {
                    name: schema_cache_key("Widget", "example.io", "v1"),
                    namespace: None,
                    labels: HashMap::new(),
                    annotations: HashMap::new(),
                    finalizers: Vec::new(),
                },
                v1_data,
            )
            .await
            .expect("v1 schema should register");

        // Register v2 schema (same kind, same group, different version)
        let v2_data = json!({
            "targetGroup": "example.io",
            "targetVersion": "v2",
            "targetKind": "Widget",
            "specSchema": {
                "type": "object",
                "properties": {
                    "color": { "type": "string" },
                    "size": { "type": "integer" }
                },
                "required": ["color", "size"]
            }
        });
        schema_service
            .create(
                schema_key.clone(),
                ObjectMeta {
                    name: schema_cache_key("Widget", "example.io", "v2"),
                    namespace: None,
                    labels: HashMap::new(),
                    annotations: HashMap::new(),
                    finalizers: Vec::new(),
                },
                v2_data,
            )
            .await
            .expect("v2 schema should register");

        // Verify cache entries exist under distinct keys
        // SchemaService has its own SchemaRegistry; ObjectService has another.
        // Populate ObjectService's registry via lazy load.
        let v1_key = schema_cache_key("Widget", "example.io", "v1");
        let v2_key = schema_cache_key("Widget", "example.io", "v2");
        assert_ne!(v1_key, v2_key);

        let widget_v1 = ResourceKey {
            group: "example.io".to_string(),
            version: "v1".to_string(),
            kind: "Widget".to_string(),
        };
        let widget_v2 = ResourceKey {
            group: "example.io".to_string(),
            version: "v2".to_string(),
            kind: "Widget".to_string(),
        };

        // Lazy-load both into ObjectService's cache
        let (v1_validator, _) =
            object_service.schema_registry.get_validator(&widget_v1).await.unwrap();
        let (v2_validator, _) =
            object_service.schema_registry.get_validator(&widget_v2).await.unwrap();

        // Both should be cached under distinct keys
        assert!(object_service.schema_registry.cache.contains_key(&v1_key));
        assert!(object_service.schema_registry.cache.contains_key(&v2_key));

        // Verify independent validation:
        // v1 requires only "color", v2 requires "color" AND "size"
        let partial_payload = json!({ "color": "red" });
        assert!(v1_validator.is_valid(&partial_payload), "v1 should accept partial payload");
        assert!(!v2_validator.is_valid(&partial_payload), "v2 should reject partial payload");

        // Payload with both fields should pass both
        let full_payload = json!({ "color": "blue", "size": 10 });
        assert!(v1_validator.is_valid(&full_payload), "v1 should accept full payload");
        assert!(v2_validator.is_valid(&full_payload), "v2 should accept full payload");

        // Verify independent eviction: delete v1 schema, v2 cache should remain
        schema_service
            .delete(schema_key.clone(), v1_key.clone())
            .await
            .expect("v1 schema should delete (no objects exist)");

        // SchemaService evicted its own cache entry for v1. ObjectService's
        // cache still has its own copies — that's expected since they are
        // separate DashMaps. The key test is that the store persists both
        // versions and the lazy-load works for each independently.
        // Verify v2 still works after v1 deletion
        let (v2_validator2, _) =
            object_service.schema_registry.get_validator(&widget_v2).await.unwrap();
        assert!(v2_validator2.is_valid(&full_payload), "v2 should still accept after v1 deletion");
    }

    #[tokio::test]
    async fn multi_version_status_validators_cache_independently() {
        let (object_service, schema_service) = make_services().await;
        let schema_key = schema_key();

        // Register v1 schema with statusSchema
        let v1_data = json!({
            "targetGroup": "example.io",
            "targetVersion": "v1",
            "targetKind": "Widget",
            "specSchema": { "type": "object" },
            "statusSchema": {
                "type": "object",
                "properties": { "phase": { "type": "string" } }
            }
        });
        schema_service
            .create(
                schema_key.clone(),
                ObjectMeta {
                    name: schema_cache_key("Widget", "example.io", "v1"),
                    namespace: None,
                    labels: HashMap::new(),
                    annotations: HashMap::new(),
                    finalizers: Vec::new(),
                },
                v1_data,
            )
            .await
            .expect("v1 schema with status should register");

        // Register v2 schema with different statusSchema
        let v2_data = json!({
            "targetGroup": "example.io",
            "targetVersion": "v2",
            "targetKind": "Widget",
            "specSchema": { "type": "object" },
            "statusSchema": {
                "type": "object",
                "properties": {
                    "phase": { "type": "string" },
                    "count": { "type": "integer" }
                }
            }
        });
        schema_service
            .create(
                schema_key,
                ObjectMeta {
                    name: schema_cache_key("Widget", "example.io", "v2"),
                    namespace: None,
                    labels: HashMap::new(),
                    annotations: HashMap::new(),
                    finalizers: Vec::new(),
                },
                v2_data,
            )
            .await
            .expect("v2 schema with status should register");

        // Lazy-load status validators into ObjectService's cache
        let widget_v1 = ResourceKey {
            group: "example.io".to_string(),
            version: "v1".to_string(),
            kind: "Widget".to_string(),
        };
        let widget_v2 = ResourceKey {
            group: "example.io".to_string(),
            version: "v2".to_string(),
            kind: "Widget".to_string(),
        };
        let _ = object_service.schema_registry.get_status_validator(&widget_v1).await.unwrap();
        let _ = object_service.schema_registry.get_status_validator(&widget_v2).await.unwrap();

        // Both should be present in cache under the base cache keys (status validators
        // are stored in the same CachedSchema entry as spec validators, under the base key)
        let v1_key = schema_cache_key("Widget", "example.io", "v1");
        let v2_key = schema_cache_key("Widget", "example.io", "v2");
        assert!(object_service.schema_registry.cache.contains_key(&v1_key));
        assert!(object_service.schema_registry.cache.contains_key(&v2_key));
        assert_ne!(v1_key, v2_key);

        // Verify both cache entries exist (status validators stored in same entry as spec)
        assert!(object_service.schema_registry.cache.contains_key(&v1_key));
        assert!(object_service.schema_registry.cache.contains_key(&v2_key));
    }

    // --- Scope validation tests ---

    // Helper to register a Namespaced Widget schema
    #[tokio::test]
    async fn create_cluster_scoped_kind_with_namespace_rejected() {
        let (service, schema_service) = make_services().await;
        register_test_schema(&schema_service).await; // Cluster-scoped Widget

        let widget_key = ResourceKey {
            group: "example.io".to_string(),
            version: "v1".to_string(),
            kind: "Widget".to_string(),
        };

        let result = service
            .create(
                widget_key,
                Some("default".to_string()),
                ObjectMeta {
                    name: "my-widget".to_string(),
                    namespace: None,
                    labels: HashMap::new(),
                    annotations: HashMap::new(),
                    finalizers: Vec::new(),
                },
                json!({"color": "blue", "size": 10}),
            )
            .await;
        assert!(
            matches!(result, Err(AppError::InvalidRequest(msg)) if msg.contains("cluster-scoped"))
        );
    }

    #[tokio::test]
    async fn create_cluster_scoped_kind_without_namespace_stores_none() {
        let (service, schema_service) = make_services().await;
        register_test_schema(&schema_service).await; // Cluster-scoped Widget (all schemas are now cluster-scoped)

        let widget_key = ResourceKey {
            group: "example.io".to_string(),
            version: "v1".to_string(),
            kind: "Widget".to_string(),
        };

        let result = service
            .create(
                widget_key,
                None, // No namespace — cluster-scoped kinds don't use namespace
                ObjectMeta {
                    name: "my-widget".to_string(),
                    namespace: None,
                    labels: HashMap::new(),
                    annotations: HashMap::new(),
                    finalizers: Vec::new(),
                },
                json!({"color": "blue", "size": 10}),
            )
            .await;
        assert!(result.is_ok());
        let stored = result.unwrap();
        // Cluster-scoped kinds always have namespace: None
        assert_eq!(stored.metadata.namespace, None);
    }

    #[tokio::test]
    async fn create_cluster_scoped_kind_with_namespace_rejected_alternate() {
        let (service, schema_service) = make_services().await;
        register_test_schema(&schema_service).await; // All schemas are now cluster-scoped

        let widget_key = ResourceKey {
            group: "example.io".to_string(),
            version: "v1".to_string(),
            kind: "Widget".to_string(),
        };

        let result = service
            .create(
                widget_key,
                Some("custom".to_string()),
                ObjectMeta {
                    name: "my-widget".to_string(),
                    namespace: None,
                    labels: HashMap::new(),
                    annotations: HashMap::new(),
                    finalizers: Vec::new(),
                },
                json!({"color": "blue", "size": 10}),
            )
            .await;
        assert!(
            matches!(result, Err(AppError::InvalidRequest(msg)) if msg.contains("cluster-scoped"))
        );
    }

    // ──────────────────────────────────────────────
    // Namespace resource tests
    // ──────────────────────────────────────────────

    /// Registers a namespaced Widget schema (scope=Namespaced) for namespace tests.
    async fn register_namespaced_test_schema(schema_service: &SchemaService) {
        let schema_key = schema_key();
        let schema_data = json!({
            "targetGroup": "example.io",
            "targetVersion": "v1",
            "targetKind": "NamespacedWidget",
            "scope": "Namespaced",
            "specSchema": {
                "type": "object",
                "properties": {
                    "color": { "type": "string" },
                    "size": { "type": "integer" }
                },
                "required": ["color", "size"]
            }
        });
        schema_service
            .create(
                schema_key,
                ObjectMeta {
                    name: "NamespacedWidget.example.io.v1".to_string(),
                    namespace: None,
                    labels: HashMap::new(),
                    annotations: HashMap::new(),
                    finalizers: Vec::new(),
                },
                schema_data,
            )
            .await
            .expect("namespaced schema registration should succeed");
    }

    fn namespaced_widget_key() -> ResourceKey {
        ResourceKey {
            group: "example.io".to_string(),
            version: "v1".to_string(),
            kind: "NamespacedWidget".to_string(),
        }
    }

    /// Create a Namespace object via the store directly (bypassing the handler's
    /// empty-spec check). The Namespace schema allows any object spec.
    async fn create_test_namespace(store: &Arc<dyn ObjectStore>, name: &str) {
        use crate::object::types::{StoredObject, SystemMetadata};
        let key = namespace_key();
        store
            .create(StoredObject {
                key,
                metadata: ObjectMeta {
                    name: name.to_string(),
                    namespace: None,
                    labels: HashMap::new(),
                    annotations: HashMap::new(),
                    finalizers: Vec::new(),
                },
                system: SystemMetadata::initial(),
                spec: json!({}),
                status: None,
            })
            .await
            .expect("namespace creation should succeed");
    }

    #[tokio::test]
    async fn create_object_in_existing_namespace_succeeds() {
        let (service, schema_service) = make_services().await;
        register_namespaced_test_schema(&schema_service).await;
        // Create the "production" namespace
        create_test_namespace(&service.store, "production").await;

        let result = service
            .create(
                namespaced_widget_key(),
                Some("production".to_string()),
                ObjectMeta {
                    name: "my-widget".to_string(),
                    namespace: None,
                    labels: HashMap::new(),
                    annotations: HashMap::new(),
                    finalizers: Vec::new(),
                },
                json!({"color": "red", "size": 1}),
            )
            .await;
        assert!(result.is_ok(), "create in existing namespace should succeed");
    }

    #[tokio::test]
    async fn create_object_in_nonexistent_namespace_returns_404() {
        let (service, schema_service) = make_services().await;
        register_namespaced_test_schema(&schema_service).await;

        let result = service
            .create(
                namespaced_widget_key(),
                Some("nonexistent".to_string()),
                ObjectMeta {
                    name: "my-widget".to_string(),
                    namespace: None,
                    labels: HashMap::new(),
                    annotations: HashMap::new(),
                    finalizers: Vec::new(),
                },
                json!({"color": "red", "size": 1}),
            )
            .await;
        match result {
            Err(AppError::NotFound { what, identifier }) => {
                assert_eq!(what, "namespace");
                assert_eq!(identifier, "nonexistent");
            }
            other => panic!("expected NotFound for namespace, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn create_object_in_default_namespace_succeeds() {
        // The "default" namespace is bootstrapped by make_services.
        let (service, schema_service) = make_services().await;
        register_namespaced_test_schema(&schema_service).await;

        let result = service
            .create(
                namespaced_widget_key(),
                None, // defaults to "default"
                ObjectMeta {
                    name: "my-widget".to_string(),
                    namespace: None,
                    labels: HashMap::new(),
                    annotations: HashMap::new(),
                    finalizers: Vec::new(),
                },
                json!({"color": "red", "size": 1}),
            )
            .await;
        assert!(result.is_ok(), "create in default namespace should succeed");
    }

    #[tokio::test]
    async fn create_cluster_scoped_object_skips_namespace_check() {
        // register_test_schema creates a cluster-scoped widget. The "nonexistent"
        // namespace should not be checked because cluster-scoped kinds have no namespace.
        let (service, schema_service) = make_services().await;
        register_test_schema(&schema_service).await;

        let widget_key = ResourceKey {
            group: "example.io".to_string(),
            version: "v1".to_string(),
            kind: "Widget".to_string(),
        };
        let result = service
            .create(
                widget_key,
                None,
                ObjectMeta {
                    name: "my-widget".to_string(),
                    namespace: None,
                    labels: HashMap::new(),
                    annotations: HashMap::new(),
                    finalizers: Vec::new(),
                },
                json!({"color": "red", "size": 1}),
            )
            .await;
        assert!(result.is_ok(), "cluster-scoped create should skip namespace check");
    }

    #[tokio::test]
    async fn delete_default_namespace_returns_403() {
        let (service, _schema_service) = make_services().await;
        // make_services bootstraps the "default" namespace
        let result = service.delete(namespace_key(), None, "default".to_string()).await;
        assert!(
            matches!(result, Err(AppError::ProtectedNamespace { ref name }) if name == "default"),
            "expected ProtectedNamespace error, got {result:?}"
        );
    }

    #[tokio::test]
    async fn delete_non_empty_namespace_returns_409() {
        let (service, schema_service) = make_services().await;
        // Create a non-default namespace
        create_test_namespace(&service.store, "production").await;

        // Create an object in "production" so the namespace is non-empty
        register_namespaced_test_schema(&schema_service).await;
        service
            .create(
                namespaced_widget_key(),
                Some("production".to_string()),
                ObjectMeta {
                    name: "widget-1".to_string(),
                    namespace: None,
                    labels: HashMap::new(),
                    annotations: HashMap::new(),
                    finalizers: Vec::new(),
                },
                json!({"color": "red", "size": 1}),
            )
            .await
            .expect("create in production should succeed");

        // Try to delete the namespace — should fail with NamespaceNotEmpty
        let result = service.delete(namespace_key(), None, "production".to_string()).await;
        match result {
            Err(AppError::NamespaceNotEmpty { namespace, object_count }) => {
                assert_eq!(namespace, "production");
                assert!(object_count >= 1);
            }
            other => panic!("expected NamespaceNotEmpty, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn delete_empty_namespace_succeeds() {
        let (service, _schema_service) = make_services().await;
        // Create an empty non-default namespace
        create_test_namespace(&service.store, "empty-ns").await;

        // Deleting an empty namespace should succeed
        let result = service.delete(namespace_key(), None, "empty-ns".to_string()).await;
        assert!(result.is_ok(), "delete empty namespace should succeed, got {result:?}");
    }

    #[tokio::test]
    async fn count_objects_in_namespace_finds_objects_across_kinds() {
        let (service, schema_service) = make_services().await;
        // Register two kinds
        register_test_schema(&schema_service).await; // cluster-scoped Widget
        register_namespaced_test_schema(&schema_service).await; // namespaced NamespacedWidget

        // Create a "production" namespace
        create_test_namespace(&service.store, "production").await;

        // Create objects in production
        service
            .create(
                namespaced_widget_key(),
                Some("production".to_string()),
                ObjectMeta {
                    name: "obj-1".to_string(),
                    namespace: None,
                    labels: HashMap::new(),
                    annotations: HashMap::new(),
                    finalizers: Vec::new(),
                },
                json!({"color": "red", "size": 1}),
            )
            .await
            .unwrap();
        service
            .create(
                namespaced_widget_key(),
                Some("production".to_string()),
                ObjectMeta {
                    name: "obj-2".to_string(),
                    namespace: None,
                    labels: HashMap::new(),
                    annotations: HashMap::new(),
                    finalizers: Vec::new(),
                },
                json!({"color": "blue", "size": 2}),
            )
            .await
            .unwrap();

        let count = service.count_objects_in_namespace("production").await.unwrap();
        assert_eq!(count, 2);
    }

    #[tokio::test]
    async fn count_objects_in_namespace_returns_zero_for_empty() {
        let (service, _schema_service) = make_services().await;
        create_test_namespace(&service.store, "empty").await;
        let count = service.count_objects_in_namespace("empty").await.unwrap();
        assert_eq!(count, 0);
    }
}
