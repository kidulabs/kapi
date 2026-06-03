//! SchemaRegistry — manages schema validation and compilation with a lazy-populated cache.
//!
//! The registry holds a meta-validator for validating Schema registration payloads
//! and a cache of compiled user schemas. On cache miss, it fetches the Schema from
//! the store, compiles it, and caches the result.

use std::sync::Arc;

use dashmap::DashMap;
use serde_json::Value;

use crate::error::AppError;
use crate::object::types::SchemaData;
use crate::schema::{JsonSchemaValidator, SchemaValidator, schema_key};
use crate::store::{ObjectStore, ResourceKey};

/// Manages schema validation, compilation, and caching.
///
/// `SchemaRegistry` provides:
/// - `validate_and_compile` — meta-schema validation + compilation (no cache insert)
/// - `get_validator` — lazy cache lookup with on-demand fetching from store
/// - `insert` / `evict` — direct cache manipulation
pub struct SchemaRegistry {
    store: Arc<dyn ObjectStore>,
    meta_validator: Arc<dyn SchemaValidator>,
    pub(crate) cache: DashMap<String, Arc<dyn SchemaValidator>>,
}

impl SchemaRegistry {
    /// Creates a new `SchemaRegistry` with the given store and meta-validator.
    ///
    /// The cache starts empty and is populated lazily on cache misses.
    pub fn new(
        store: Arc<dyn ObjectStore>,
        meta_validator: Arc<dyn SchemaValidator>,
    ) -> Self {
        Self {
            store,
            meta_validator,
            cache: DashMap::new(),
        }
    }

    /// Validates `spec` against the meta-schema, parses it into `SchemaData`,
    /// and compiles the json_schema into a validator.
    ///
    /// Returns the parsed [`SchemaData`] and compiled validator.
    /// Does **not** insert into the cache — the caller is responsible for
    /// calling [`insert`](Self::insert) if caching is desired.
    ///
    /// # Errors
    ///
    /// Returns `AppError::InvalidSchema` if:
    /// - The spec fails meta-schema validation
    /// - The spec cannot be parsed into `SchemaData`
    /// - The json_schema cannot be compiled
    pub fn validate_and_compile(
        &self,
        spec: &Value,
    ) -> Result<(SchemaData, Arc<dyn SchemaValidator>), AppError> {
        // Validate against meta-schema
        if !self.meta_validator.is_valid(spec) {
            let errors: Vec<String> = self
                .meta_validator
                .validate(spec)
                .into_iter()
                .map(|e| e.message)
                .collect();
            return Err(AppError::InvalidSchema(errors.join("; ")));
        }

        // Parse into SchemaData
        let schema_data: SchemaData = serde_json::from_value(spec.clone())
            .map_err(|e| AppError::InvalidSchema(format!("failed to parse schema data: {}", e)))?;

        // Compile json_schema
        let validator = JsonSchemaValidator::compile(&schema_data.json_schema)
            .map_err(|e| AppError::InvalidSchema(format!("failed to compile jsonSchema: {}", e)))
            .map(|v| Arc::new(v) as Arc<dyn SchemaValidator>)?;

        Ok((schema_data, validator))
    }

    /// Returns a compiled validator for the given object `key`.
    ///
    /// Checks the cache first (keyed as `"{kind}.{group}"`). On cache miss,
    /// fetches the Schema from the store, compiles it, inserts into the cache,
    /// and returns the validator.
    ///
    /// # Errors
    ///
    /// Returns:
    /// - [`AppError::NotFound`] if no Schema exists in the store for this kind+group
    /// - [`AppError::StoredSchemaCompilationFailed`] if the stored schema fails to compile
    pub async fn get_validator(
        &self,
        key: &ResourceKey,
    ) -> Result<Arc<dyn SchemaValidator>, AppError> {
        let cache_key = format!("{}.{}", key.kind, key.group);

        // Cache hit
        if let Some(validator) = self.cache.get(&cache_key) {
            return Ok(validator.clone());
        }

        // Cache miss: fetch from store
        let schema_key = schema_key();
        let schema_name = cache_key.clone();

        let schema_obj = self
            .store
            .get(&schema_key, &schema_name)
            .await
            .map_err(|_| AppError::NotFound {
                what: "schema".to_string(),
                identifier: schema_name.clone(),
            })?;

        // Parse and compile
        let schema_data: SchemaData = serde_json::from_value(schema_obj.spec.value)
            .map_err(|e| AppError::InvalidSchema(format!("failed to parse schema data: {}", e)))?;

        let compiled = JsonSchemaValidator::compile(&schema_data.json_schema)
            .map_err(|e| AppError::StoredSchemaCompilationFailed {
                schema_name: cache_key.clone(),
                reason: e.to_string(),
            })
            .map(|v| Arc::new(v) as Arc<dyn SchemaValidator>)?;

        // Insert into cache and return
        self.cache.insert(cache_key.clone(), compiled.clone());
        Ok(compiled)
    }

    /// Inserts a compiled validator into the cache under the given name.
    ///
    /// If an entry with the same name already exists, it is replaced.
    pub fn insert(&self, name: &str, validator: Arc<dyn SchemaValidator>) {
        self.cache.insert(name.to_string(), validator);
    }

    /// Removes a validator from the cache.
    ///
    /// This is a no-op if the entry does not exist.
    pub fn evict(&self, name: &str) {
        self.cache.remove(name);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::meta_schema::compile_meta_schema;
    use crate::store::memory::InMemoryStore;
    use serde_json::json;

    fn make_registry() -> SchemaRegistry {
        let store: Arc<dyn ObjectStore> = Arc::new(InMemoryStore::new());
        let meta_validator: Arc<dyn SchemaValidator> =
            Arc::new(compile_meta_schema().expect("meta-schema should compile"));
        SchemaRegistry::new(store, meta_validator)
    }

    async fn store_test_schema(registry: &SchemaRegistry, name: &str) {
        let schema_key = schema_key();
        let schema_data = json!({
            "targetGroup": "example.io",
            "targetVersion": "v1",
            "targetKind": "Widget",
            "jsonSchema": {
                "type": "object",
                "properties": {
                    "color": { "type": "string" },
                    "size": { "type": "integer" }
                }
            }
        });
        registry
            .store
            .create(
                &schema_key,
                crate::object::types::ObjectMeta {
                    name: name.to_string(),
                    labels: std::collections::HashMap::new(),
                },
                schema_data,
            )
            .await
            .expect("store create should succeed");
    }

    // --- validate_and_compile tests ---

    #[tokio::test]
    async fn validate_and_compile_valid_data_returns_schema_and_validator() {
        let registry = make_registry();
        let spec = json!({
            "targetGroup": "example.io",
            "targetVersion": "v1",
            "targetKind": "Widget",
            "jsonSchema": {
                "type": "object",
                "properties": { "color": { "type": "string" } }
            }
        });

        let result = registry.validate_and_compile(&spec);
        assert!(result.is_ok());
        let (schema_data, _validator) = result.unwrap();
        assert_eq!(schema_data.target_kind, "Widget");
        assert_eq!(schema_data.target_group, "example.io");

        // Cache should NOT be modified
        assert!(!registry.cache.contains_key("Widget.example.io"));
    }

    #[tokio::test]
    async fn validate_and_compile_invalid_meta_schema_returns_invalid_schema() {
        let registry = make_registry();
        // Missing required fields targetVersion, targetKind, jsonSchema
        let spec = json!({ "targetGroup": "example.io" });

        let result = registry.validate_and_compile(&spec);
        assert!(matches!(result, Err(AppError::InvalidSchema(_))));
    }

    #[tokio::test]
    async fn validate_and_compile_uncompilable_jsonschema_returns_invalid_schema() {
        let registry = make_registry();
        let spec = json!({
            "targetGroup": "example.io",
            "targetVersion": "v1",
            "targetKind": "Widget",
            "jsonSchema": { "type": "not-a-real-type" }
        });

        let result = registry.validate_and_compile(&spec);
        assert!(matches!(result, Err(AppError::InvalidSchema(_))));
    }

    // --- get_validator tests ---

    #[tokio::test]
    async fn get_validator_cache_hit_returns_cached_validator() {
        let registry = make_registry();
        let key = ResourceKey {
            group: "example.io".to_string(),
            version: "v1".to_string(),
            kind: "Widget".to_string(),
        };

        // Prime the cache directly
        let dummy_validator: Arc<dyn SchemaValidator> =
            Arc::new(compile_meta_schema().expect("meta-schema should compile"));
        registry.cache.insert("Widget.example.io".to_string(), dummy_validator.clone());

        let result = registry.get_validator(&key).await;
        assert!(result.is_ok());
        // Verify it's the same validator by checking pointer identity via Arc::ptr_eq
        assert!(Arc::ptr_eq(&result.unwrap(), &dummy_validator));
    }

    #[tokio::test]
    async fn get_validator_cache_miss_fetches_compiles_and_caches() {
        let registry = make_registry();
        let key = ResourceKey {
            group: "example.io".to_string(),
            version: "v1".to_string(),
            kind: "Widget".to_string(),
        };

        // Store a schema in the store
        store_test_schema(&registry, "Widget.example.io").await;

        let result = registry.get_validator(&key).await;
        assert!(result.is_ok());

        // Verify it was cached
        assert!(registry.cache.contains_key("Widget.example.io"));
    }

    #[tokio::test]
    async fn get_validator_cache_miss_no_schema_returns_not_found() {
        let registry = make_registry();
        let key = ResourceKey {
            group: "unknown.io".to_string(),
            version: "v1".to_string(),
            kind: "Unknown".to_string(),
        };

        let result = registry.get_validator(&key).await;
        assert!(matches!(result, Err(AppError::NotFound { .. })));
    }

    #[tokio::test]
    async fn get_validator_cache_miss_uncompilable_schema_returns_compilation_failed() {
        let registry = make_registry();
        let schema_key = schema_key();
        let invalid_schema = json!({
            "targetGroup": "example.io",
            "targetVersion": "v1",
            "targetKind": "Widget",
            "jsonSchema": { "type": "not-a-real-type" }
        });
        registry
            .store
            .create(
                &schema_key,
                crate::object::types::ObjectMeta {
                    name: "Widget.example.io".to_string(),
                    labels: std::collections::HashMap::new(),
                },
                invalid_schema,
            )
            .await
            .expect("store create should succeed");

        let key = ResourceKey {
            group: "example.io".to_string(),
            version: "v1".to_string(),
            kind: "Widget".to_string(),
        };

        let result = registry.get_validator(&key).await;
        assert!(
            matches!(result, Err(AppError::StoredSchemaCompilationFailed { .. })),
            "expected StoredSchemaCompilationFailed, got something else"
        );
    }

    // --- insert tests ---

    #[tokio::test]
    async fn insert_adds_new_entry() {
        let registry = make_registry();
        let validator: Arc<dyn SchemaValidator> =
            Arc::new(compile_meta_schema().expect("meta-schema should compile"));

        assert!(!registry.cache.contains_key("test-schema"));
        registry.insert("test-schema", validator.clone());
        assert!(registry.cache.contains_key("test-schema"));
    }

    #[tokio::test]
    async fn insert_replaces_existing_entry() {
        let registry = make_registry();
        let validator1: Arc<dyn SchemaValidator> =
            Arc::new(compile_meta_schema().expect("meta-schema should compile"));
        let validator2: Arc<dyn SchemaValidator> =
            Arc::new(compile_meta_schema().expect("meta-schema should compile"));

        registry.insert("test-schema", validator1);
        assert!(registry.cache.contains_key("test-schema"));

        registry.insert("test-schema", validator2);
        // Still present (replaced)
        assert!(registry.cache.contains_key("test-schema"));
    }

    // --- evict tests ---

    #[tokio::test]
    async fn evict_removes_existing_entry() {
        let registry = make_registry();
        let validator: Arc<dyn SchemaValidator> =
            Arc::new(compile_meta_schema().expect("meta-schema should compile"));

        registry.insert("test-schema", validator);
        assert!(registry.cache.contains_key("test-schema"));

        registry.evict("test-schema");
        assert!(!registry.cache.contains_key("test-schema"));
    }

    #[tokio::test]
    async fn evict_non_existent_entry_is_noop() {
        let registry = make_registry();
        // Should not panic
        registry.evict("non-existent");
    }
}
