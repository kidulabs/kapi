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
    pub async fn create(
        &self,
        key: ResourceKey,
        meta: ObjectMeta,
        spec: Value,
    ) -> Result<StoredObject, AppError> {
        // Copy exact logic from ObjectService::validate_and_create_schema
        // but use helpers::publish_event(&self.event_bus, ...) instead of self.publish_event(...)
        validate_labels(&meta.labels)?;
        validate_annotations(&meta.annotations)?;
        validate_finalizers(&meta.finalizers)?;
        let (_schema_data, compiled, status_compiled) =
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
        self.schema_registry.insert(&meta.name, compiled);
        if let Some(status_validator) = status_compiled {
            self.schema_registry.insert_status(&meta.name, status_validator);
        }
        helpers::publish_event(self.event_bus.as_ref(), &key, WatchEventType::Added, &stored);
        Ok(stored)
    }

    /// Updates a Schema object: revalidates, recompiles, persists, caches, publishes.
    pub async fn update(&self, object: StoredObject) -> Result<StoredObject, AppError> {
        // Copy exact logic from ObjectService::validate_and_update_schema
        // but use helpers::apply_with_metadata(...) and helpers::publish_event(...)
        let spec = object.spec.clone();
        validate_labels(&object.metadata.labels)?;
        validate_annotations(&object.metadata.annotations)?;
        validate_finalizers(&object.metadata.finalizers)?;
        let (_schema_data, compiled, status_compiled) =
            self.schema_registry.validate_and_compile(&spec)?;

        let key = object.key.clone();
        let name = object.metadata.name.clone();
        let incoming_rv = object.system.resource_version;
        let updated = self.store.transaction(
            &key,
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
        self.schema_registry.insert(&updated.metadata.name, compiled);
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
        let schema_obj = self.store.get(&key, &name).await?;
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
        let deleted =
            self.store.transaction(&key, &name, Box::new(|_existing| TransactionOp::Delete))?;
        self.schema_registry.evict(&name);
        helpers::publish_event(self.event_bus.as_ref(), &key, WatchEventType::Deleted, &deleted);
        Ok(deleted)
    }

    /// Returns a compiled validator for the given object key.
    /// Delegates to the internal SchemaRegistry.
    pub async fn get_validator(
        &self,
        key: &ResourceKey,
    ) -> Result<Arc<dyn SchemaValidator>, AppError> {
        self.schema_registry.get_validator(key).await
    }

    /// Returns a compiled status validator for the given object key.
    pub async fn get_status_validator(
        &self,
        key: &ResourceKey,
    ) -> Result<Arc<dyn SchemaValidator>, AppError> {
        self.schema_registry.get_status_validator(key).await
    }
}
