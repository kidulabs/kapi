//! SchemaService — orchestrates Schema lifecycle operations.
//!
//! Owns the SchemaRegistry and handles Schema create, update, and delete.
//! Uses shared helpers from `helpers.rs` for metadata management and event publishing.

use std::sync::Arc;

use serde_json::Value;

use crate::error::AppError;
use crate::event::EventPublisher;
use crate::object::helpers;
use crate::object::types::{ObjectMeta, SchemaData, StoredObject, SystemMetadata, WatchEventType};
use crate::schema::{SchemaRegistry, SchemaValidator};
use crate::store::{ObjectStore, ResourceKey, TransactionOp};
use crate::validation::{validate_annotations, validate_finalizers, validate_labels};

/// SchemaService wraps store, event bus, and schema registry.
///
/// Orchestrates Schema lifecycle: meta-schema validation, compilation,
/// caching, storage, and event publishing.
pub struct SchemaService {
    store: Arc<dyn ObjectStore>,
    event_bus: Arc<dyn EventPublisher>,
    schema_registry: SchemaRegistry,
}

impl SchemaService {
    /// Creates a new SchemaService.
    ///
    /// Constructs a SchemaRegistry internally from `store` and `meta_validator`.
    pub fn new(
        store: Arc<dyn ObjectStore>,
        event_bus: Arc<dyn EventPublisher>,
        meta_validator: Arc<dyn SchemaValidator>,
    ) -> Self {
        let schema_registry = SchemaRegistry::new(store.clone(), meta_validator);
        Self { store, event_bus, schema_registry }
    }

    /// Creates a Schema object: validates against meta-schema, compiles, stores, caches, publishes.
    ///
    /// The Schema resource itself is always cluster-scoped (`metadata.namespace` is forced to
    /// `None`). However, the `scope` field in the spec (`SchemaData.scope`) is user-specified
    /// and determines whether objects of the target kind are cluster-scoped or namespaced.
    /// It is preserved as-is from the request body.
    pub async fn create(
        &self,
        key: ResourceKey,
        meta: ObjectMeta,
        spec: Value,
    ) -> Result<StoredObject, AppError> {
        // Schema resource is always cluster-scoped — force namespace to None
        // but preserve the user-specified scope from the spec for target objects
        let meta = ObjectMeta { namespace: None, ..meta };

        validate_labels(&meta.labels)?;
        validate_annotations(&meta.annotations)?;
        validate_finalizers(&meta.finalizers)?;
        let (schema_data, compiled, status_compiled) =
            self.schema_registry.validate_and_compile(&spec)?;
        let stored = self
            .store
            .create(StoredObject {
                key: key.clone(),
                metadata: meta.clone(),
                system: SystemMetadata::initial(),
                spec,
                status: None,
            })
            .await?;
        self.schema_registry.insert(&meta.name, compiled, &schema_data.scope);
        if let Some(status_validator) = status_compiled {
            self.schema_registry.insert_status(&meta.name, status_validator);
        }
        helpers::publish_event(self.event_bus.as_ref(), &key, WatchEventType::Added, &stored);
        Ok(stored)
    }

    /// Updates a Schema object: revalidates, recompiles, persists, caches, publishes.
    ///
    /// The Schema resource itself is always cluster-scoped (`metadata.namespace` is forced to
    /// `None`). However, the `scope` field in the spec (`SchemaData.scope`) is user-specified
    /// and determines whether objects of the target kind are cluster-scoped or namespaced.
    /// It is preserved as-is from the request body.
    pub async fn update(&self, mut object: StoredObject) -> Result<StoredObject, AppError> {
        // Schema resource is always cluster-scoped — force namespace to None
        // but preserve the user-specified scope from the spec for target objects
        object.metadata.namespace = None;
        let spec = object.spec.clone();

        validate_labels(&object.metadata.labels)?;
        validate_annotations(&object.metadata.annotations)?;
        validate_finalizers(&object.metadata.finalizers)?;
        let (schema_data, compiled, status_compiled) =
            self.schema_registry.validate_and_compile(&spec)?;

        let key = object.key.clone();
        let name = object.metadata.name.clone();
        let incoming_rv = object.system.resource_version;
        let updated = self.store.transaction(
            &key,
            None,
            &name,
            Box::new(move |existing| {
                if incoming_rv != existing.system.resource_version {
                    return TransactionOp::Abort(AppError::Conflict {
                        expected: existing.system.resource_version,
                        actual: incoming_rv,
                    });
                }
                helpers::apply_with_metadata(existing, |_existing| {
                    let mut updated = existing.clone();
                    updated.metadata = object.metadata.clone();
                    updated.spec = object.spec.clone();
                    updated
                })
            }),
        )?;
        self.schema_registry.insert(&updated.metadata.name, compiled, &schema_data.scope);
        if let Some(status_validator) = status_compiled {
            self.schema_registry.insert_status(&updated.metadata.name, status_validator);
        }
        helpers::publish_event(
            self.event_bus.as_ref(),
            &updated.key,
            WatchEventType::Modified,
            &updated,
        );
        Ok(updated)
    }

    /// Deletes a Schema object: checks for dependent objects, removes, evicts cache, publishes.
    pub async fn delete(&self, key: ResourceKey, name: String) -> Result<StoredObject, AppError> {
        // Copy exact logic from ObjectService::delete_schema
        // but use helpers::publish_event(...)
        let schema_obj = self.store.get(&key, None, &name).await?;
        let schema_data: SchemaData = serde_json::from_value(schema_obj.spec.clone())
            .map_err(|e| AppError::InvalidSchema(format!("failed to parse schema data: {}", e)))?;
        let target_key = ResourceKey {
            group: schema_data.target_group,
            version: schema_data.target_version,
            kind: schema_data.target_kind,
        };
        if self.store.exists(&target_key).await? {
            return Err(AppError::SchemaHasObjects { kind: target_key.kind });
        }
        let deleted = self.store.transaction(
            &key,
            None,
            &name,
            Box::new(|_existing| TransactionOp::Delete),
        )?;
        self.schema_registry.evict(&name);
        helpers::publish_event(self.event_bus.as_ref(), &key, WatchEventType::Deleted, &deleted);
        Ok(deleted)
    }

    /// Returns a compiled validator and scope for the given object key.
    /// Delegates to the internal SchemaRegistry.
    pub async fn get_validator(
        &self,
        key: &ResourceKey,
    ) -> Result<(Arc<dyn SchemaValidator>, String), AppError> {
        self.schema_registry.get_validator(key).await
    }

    /// Returns a compiled status validator for the given object key.
    pub async fn get_status_validator(
        &self,
        key: &ResourceKey,
    ) -> Result<Arc<dyn SchemaValidator>, AppError> {
        self.schema_registry.get_status_validator(key).await
    }

    /// Expose the schema registry for testing.
    #[cfg(test)]
    pub(crate) fn registry(&self) -> &SchemaRegistry {
        &self.schema_registry
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::EventPublisher;
    use crate::object::service::ObjectService;
    use crate::schema::meta_schema::compile_meta_schema;
    use crate::schema::schema_cache_key;
    use crate::schema::schema_key;
    use crate::store::memory::InMemoryStore;
    use serde_json::json;
    use std::collections::HashMap;

    fn make_service() -> (ObjectService, SchemaService) {
        let store: Arc<dyn ObjectStore> = Arc::new(InMemoryStore::new());
        let event_bus: Arc<dyn EventPublisher> = Arc::new(crate::event::EventBus::default());
        let meta_validator: Arc<dyn SchemaValidator> =
            Arc::new(compile_meta_schema().expect("meta-schema should compile"));
        let schema_registry = SchemaRegistry::new(store.clone(), meta_validator.clone());
        let schema_service = SchemaService::new(store.clone(), event_bus.clone(), meta_validator);
        let object_service = ObjectService::new(store, event_bus, schema_registry);
        (object_service, schema_service)
    }

    #[tokio::test]
    async fn delete_v1_schema_keeps_v2_cache_intact() {
        let (object_service, schema_service) = make_service();
        let schema_key = schema_key();

        // Register v1 schema
        let v1_name = schema_cache_key("Widget", "example.io", "v1");
        schema_service
            .create(
                schema_key.clone(),
                ObjectMeta {
                    name: v1_name.clone(),
                    namespace: None,
                    labels: HashMap::new(),
                    annotations: HashMap::new(),
                    finalizers: Vec::new(),
                },
                json!({
                    "targetGroup": "example.io",
                    "targetVersion": "v1",
                    "targetKind": "Widget",
                    "specSchema": { "type": "object" }
                }),
            )
            .await
            .expect("v1 schema should register");

        // Register v2 schema
        let v2_name = schema_cache_key("Widget", "example.io", "v2");
        schema_service
            .create(
                schema_key.clone(),
                ObjectMeta {
                    name: v2_name.clone(),
                    namespace: None,
                    labels: HashMap::new(),
                    annotations: HashMap::new(),
                    finalizers: Vec::new(),
                },
                json!({
                    "targetGroup": "example.io",
                    "targetVersion": "v2",
                    "targetKind": "Widget",
                    "specSchema": { "type": "object" }
                }),
            )
            .await
            .expect("v2 schema should register");

        // Create an object at v2
        let widget_v2 = ResourceKey {
            group: "example.io".to_string(),
            version: "v2".to_string(),
            kind: "Widget".to_string(),
        };
        object_service
            .create(
                widget_v2,
                None,
                ObjectMeta {
                    name: "my-widget".to_string(),
                    namespace: None,
                    labels: HashMap::new(),
                    annotations: HashMap::new(),
                    finalizers: Vec::new(),
                },
                json!({"dummy": "data"}),
            )
            .await
            .expect("v2 object should create");

        // Delete v1 schema — should succeed (no objects at v1)
        let result = schema_service.delete(schema_key.clone(), v1_name.clone()).await;
        assert!(result.is_ok(), "v1 schema deletion should succeed");

        // Verify v2 cache entry is untouched
        assert!(
            schema_service.registry().cache.contains_key(&v2_name),
            "v2 cache entry should remain after v1 deletion"
        );
        assert!(
            !schema_service.registry().cache.contains_key(&v1_name),
            "v1 cache entry should be evicted"
        );
    }

    // T: Schema create stores object with namespace: None
    #[tokio::test]
    async fn schema_create_stores_with_no_namespace() {
        let (_object_service, schema_service) = make_service();
        let schema_key = schema_key();
        let name = "Widget.example.io.v1".to_string();

        let result = schema_service
            .create(
                schema_key.clone(),
                ObjectMeta {
                    name: name.clone(),
                    namespace: Some("should-be-ignored".to_string()),
                    labels: HashMap::new(),
                    annotations: HashMap::new(),
                    finalizers: Vec::new(),
                },
                json!({
                    "targetGroup": "example.io",
                    "targetVersion": "v1",
                    "targetKind": "Widget",
                    "specSchema": { "type": "object" }
                }),
            )
            .await;
        assert!(result.is_ok());
        let stored = result.unwrap();
        assert!(
            stored.metadata.namespace.is_none(),
            "Schema metadata.namespace should be None, got {:?}",
            stored.metadata.namespace
        );

        // Also verify via store get
        let retrieved = _object_service.get(schema_key, None, name).await.unwrap();
        assert!(
            retrieved.metadata.namespace.is_none(),
            "Schema stored in store should have namespace=None"
        );
    }

    // T: Schema create preserves user-specified scope in stored spec
    #[tokio::test]
    async fn schema_create_preserves_user_scope() {
        let (_object_service, schema_service) = make_service();
        let schema_key = schema_key();
        let name = "Widget.example.io.v1".to_string();

        // Create Schema with explicit "Namespaced" scope
        let result = schema_service
            .create(
                schema_key.clone(),
                ObjectMeta {
                    name: name.clone(),
                    namespace: None,
                    labels: HashMap::new(),
                    annotations: HashMap::new(),
                    finalizers: Vec::new(),
                },
                json!({
                    "targetGroup": "example.io",
                    "targetVersion": "v1",
                    "targetKind": "Widget",
                    "scope": "Namespaced",
                    "specSchema": { "type": "object" }
                }),
            )
            .await;
        assert!(result.is_ok());
        let stored = result.unwrap();

        // Verify the stored spec preserves the user-specified scope="Namespaced"
        let stored_scope = stored.spec.get("scope").and_then(|v| v.as_str());
        assert_eq!(
            stored_scope,
            Some("Namespaced"),
            "Schema spec.scope should be preserved as 'Namespaced', got {:?}",
            stored_scope
        );

        // Verify the registry cached it with "Namespaced" scope
        let widget_key = ResourceKey {
            group: "example.io".to_string(),
            version: "v1".to_string(),
            kind: "Widget".to_string(),
        };
        let (_, scope) = schema_service.get_validator(&widget_key).await.unwrap();
        assert_eq!(
            scope, "Namespaced",
            "Cached schema scope should be 'Namespaced', got '{}'",
            scope
        );

        // Also verify that creating with scope="Cluster" still works
        let cluster_name = "ClusterWidget.example.io.v1".to_string();
        let result2 = schema_service
            .create(
                schema_key.clone(),
                ObjectMeta {
                    name: cluster_name.clone(),
                    namespace: None,
                    labels: HashMap::new(),
                    annotations: HashMap::new(),
                    finalizers: Vec::new(),
                },
                json!({
                    "targetGroup": "example.io",
                    "targetVersion": "v1",
                    "targetKind": "ClusterWidget",
                    "scope": "Cluster",
                    "specSchema": { "type": "object" }
                }),
            )
            .await;
        assert!(result2.is_ok());
        let stored2 = result2.unwrap();
        let stored_scope2 = stored2.spec.get("scope").and_then(|v| v.as_str());
        assert_eq!(
            stored_scope2,
            Some("Cluster"),
            "Schema spec.scope should be preserved as 'Cluster', got {:?}",
            stored_scope2
        );

        // Verify default scope (no scope specified) resolves to "Namespaced" via SchemaData default
        let default_name = "DefaultScope.example.io.v1".to_string();
        let result3 = schema_service
            .create(
                schema_key,
                ObjectMeta {
                    name: default_name.clone(),
                    namespace: None,
                    labels: HashMap::new(),
                    annotations: HashMap::new(),
                    finalizers: Vec::new(),
                },
                json!({
                    "targetGroup": "example.io",
                    "targetVersion": "v1",
                    "targetKind": "DefaultScope",
                    "specSchema": { "type": "object" }
                }),
            )
            .await;
        assert!(result3.is_ok());

        // Verify the registry cached it with "Namespaced" scope (from serde default)
        let default_key = ResourceKey {
            group: "example.io".to_string(),
            version: "v1".to_string(),
            kind: "DefaultScope".to_string(),
        };
        let (_, scope) = schema_service.get_validator(&default_key).await.unwrap();
        assert_eq!(
            scope, "Namespaced",
            "Default scope should be 'Namespaced' when not specified in request"
        );
    }

    // T: Schema update preserves user-specified scope and forces namespace to None
    #[tokio::test]
    async fn schema_update_preserves_scope_and_forces_no_namespace() {
        let (_object_service, schema_service) = make_service();
        let schema_key = schema_key();
        let name = "Widget.example.io.v1".to_string();

        // Create schema first
        let created = schema_service
            .create(
                schema_key.clone(),
                ObjectMeta {
                    name: name.clone(),
                    namespace: None,
                    labels: HashMap::new(),
                    annotations: HashMap::new(),
                    finalizers: Vec::new(),
                },
                json!({
                    "targetGroup": "example.io",
                    "targetVersion": "v1",
                    "targetKind": "Widget",
                    "scope": "Namespaced",
                    "specSchema": { "type": "object" }
                }),
            )
            .await
            .unwrap();

        // Update with explicit scope and a namespace — scope should be preserved, namespace forced to None
        let mut updated_obj = created;
        updated_obj.spec = json!({
            "targetGroup": "example.io",
            "targetVersion": "v1",
            "targetKind": "Widget",
            "scope": "Namespaced",
            "specSchema": { "type": "object", "properties": { "color": { "type": "string" } } }
        });
        updated_obj.metadata.namespace = Some("should-be-ignored".to_string());

        let result = schema_service.update(updated_obj).await;
        assert!(result.is_ok());
        let stored = result.unwrap();

        // Verify namespace is None (forced by schema service)
        assert!(
            stored.metadata.namespace.is_none(),
            "Updated Schema metadata.namespace should be None, got {:?}",
            stored.metadata.namespace
        );

        // Verify scope is preserved as "Namespaced" (user-specified)
        let stored_scope = stored.spec.get("scope").and_then(|v| v.as_str());
        assert_eq!(
            stored_scope,
            Some("Namespaced"),
            "Updated Schema spec.scope should be preserved as 'Namespaced', got {:?}",
            stored_scope
        );
    }
}
