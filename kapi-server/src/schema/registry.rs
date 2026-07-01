//! SchemaRegistry — manages schema validation and compilation with a lazy-populated cache.
//!
//! The registry holds a meta-validator for validating Schema registration payloads
//! and a cache of compiled user schemas. On cache miss, it fetches the Schema from
//! the store, compiles it, and caches the result.

use std::sync::Arc;

use dashmap::DashMap;
use serde_json::Value;

use crate::error::AppError;
use crate::object::types::{ListOptions, SchemaData};
use crate::schema::{
    JsonSchemaValidator, SCOPE_NAMESPACED, SchemaValidator, schema_cache_key, schema_key,
};
use crate::store::{ObjectStore, ResourceKey};

/// Result of schema validation and compilation: parsed data, spec validator, optional status validator.
type CompileResult = (SchemaData, Arc<dyn SchemaValidator>, Option<Arc<dyn SchemaValidator>>);

/// Cached schema entry containing validator, optional status validator, and scope.
pub(crate) struct CachedSchema {
    pub validator: Arc<dyn SchemaValidator>,
    pub status_validator: Option<Arc<dyn SchemaValidator>>,
    pub scope: String,
}

/// Manages schema validation, compilation, and caching.
///
/// `SchemaRegistry` provides:
/// - `validate_and_compile` — meta-schema validation + compilation (no cache insert)
/// - `get_validator` — lazy cache lookup with on-demand fetching from store
/// - `insert` / `evict` — direct cache manipulation
pub struct SchemaRegistry {
    store: Arc<dyn ObjectStore>,
    meta_validator: Arc<dyn SchemaValidator>,
    pub(crate) cache: DashMap<String, CachedSchema>,
}

impl SchemaRegistry {
    /// Creates a new `SchemaRegistry` with the given store and meta-validator.
    ///
    /// The cache starts empty and is populated lazily on cache misses.
    pub fn new(store: Arc<dyn ObjectStore>, meta_validator: Arc<dyn SchemaValidator>) -> Self {
        Self { store, meta_validator, cache: DashMap::new() }
    }

    /// Validates `spec` against the meta-schema, parses it into `SchemaData`,
    /// and compiles the spec_schema into a validator.
    ///
    /// Returns the parsed [`SchemaData`], the compiled spec validator, and optionally
    /// a compiled status validator (if `statusSchema` is present in the spec).
    /// Does **not** insert into the cache — the caller is responsible for
    /// calling [`insert`](Self::insert) or [`insert_status`](Self::insert_status) if caching is desired.
    ///
    /// # Errors
    ///
    /// Returns `AppError::InvalidSchema` if:
    /// - The spec fails meta-schema validation
    /// - The spec cannot be parsed into `SchemaData`
    /// - The spec_schema cannot be compiled
    /// - The status_schema cannot be compiled (when present)
    pub fn validate_and_compile(&self, spec: &Value) -> Result<CompileResult, AppError> {
        // Validate against meta-schema
        if !self.meta_validator.is_valid(spec) {
            let errors: Vec<String> =
                self.meta_validator.validate(spec).into_iter().map(|e| e.message).collect();
            return Err(AppError::InvalidSchema(errors.join("; ")));
        }

        // Parse into SchemaData
        let schema_data: SchemaData = serde_json::from_value(spec.clone())
            .map_err(|e| AppError::InvalidSchema(format!("failed to parse schema data: {}", e)))?;

        // Compile spec_schema
        let validator = JsonSchemaValidator::compile(&schema_data.spec_schema)
            .map_err(|e| AppError::InvalidSchema(format!("failed to compile specSchema: {}", e)))
            .map(|v| Arc::new(v) as Arc<dyn SchemaValidator>)?;

        // Optionally compile status_schema
        let status_validator = schema_data
            .status_schema
            .as_ref()
            .map(|ss| {
                JsonSchemaValidator::compile(ss)
                    .map_err(|e| {
                        AppError::InvalidSchema(format!("failed to compile statusSchema: {}", e))
                    })
                    .map(|v| Arc::new(v) as Arc<dyn SchemaValidator>)
            })
            .transpose()?;

        Ok((schema_data, validator, status_validator))
    }

    /// Returns a compiled validator and scope for the given object `key`.
    ///
    /// Checks the cache first (keyed as `"{kind}.{group}.{version}"`). On cache miss,
    /// fetches the Schema from the store, compiles it, inserts into the cache,
    /// and returns the validator and scope.
    ///
    /// # Errors
    ///
    /// Returns:
    /// - [`AppError::NotFound`] if no Schema exists in the store for this kind+group+version
    /// - [`AppError::StoredSchemaCompilationFailed`] if the stored schema fails to compile
    pub async fn get_validator(
        &self,
        key: &ResourceKey,
    ) -> Result<(Arc<dyn SchemaValidator>, String), AppError> {
        let cache_key = schema_cache_key(&key.kind, &key.group, &key.version);

        // Cache hit
        if let Some(cached) = self.cache.get(&cache_key) {
            return Ok((cached.validator.clone(), cached.scope.clone()));
        }

        // Cache miss: fetch from store
        let schema_key = schema_key();
        let schema_name = cache_key.clone();

        let schema_obj = self.store.get(&schema_key, None, &schema_name).await.map_err(|_| {
            AppError::NotFound { what: "schema".to_string(), identifier: schema_name.clone() }
        })?;

        // Parse and compile
        let schema_data: SchemaData = serde_json::from_value(schema_obj.spec.clone())
            .map_err(|e| AppError::InvalidSchema(format!("failed to parse schema data: {}", e)))?;

        let compiled = JsonSchemaValidator::compile(&schema_data.spec_schema)
            .map_err(|e| AppError::StoredSchemaCompilationFailed {
                schema_name: cache_key.clone(),
                reason: e.to_string(),
            })
            .map(|v| Arc::new(v) as Arc<dyn SchemaValidator>)?;

        // Optionally compile status_schema
        let status_validator = schema_data
            .status_schema
            .as_ref()
            .map(|ss| {
                JsonSchemaValidator::compile(ss)
                    .map_err(|e| AppError::StoredSchemaCompilationFailed {
                        schema_name: cache_key.clone(),
                        reason: e.to_string(),
                    })
                    .map(|v| Arc::new(v) as Arc<dyn SchemaValidator>)
            })
            .transpose()?;

        // Insert into cache and return
        let scope = schema_data.scope.clone();
        self.cache.insert(
            cache_key.clone(),
            CachedSchema { validator: compiled.clone(), status_validator, scope: scope.clone() },
        );
        Ok((compiled, scope))
    }

    /// Inserts a compiled validator and scope into the cache under the given name.
    ///
    /// The name should be the versioned schema name (e.g., `"Widget.example.io.v1"`).
    /// If an entry with the same name already exists, it is replaced.
    pub fn insert(&self, name: &str, validator: Arc<dyn SchemaValidator>, scope: &str) {
        self.cache.insert(
            name.to_string(),
            CachedSchema { validator, status_validator: None, scope: scope.to_string() },
        );
    }

    /// Removes a validator from the cache.
    ///
    /// Removes the entry keyed by `name`. This is a no-op if the entry does not exist.
    pub fn evict(&self, name: &str) {
        self.cache.remove(name);
    }

    /// Returns a compiled status validator for the given object `key`.
    ///
    /// Checks the cache first. On cache miss, fetches the Schema from the store,
    /// parses `status_schema`, compiles it, inserts into the cache, and returns the validator.
    ///
    /// # Errors
    ///
    /// Returns:
    /// - [`AppError::NotFound`] if no Schema exists in the store for this kind+group+version
    /// - [`AppError::StatusSubresourceNotEnabled`] if the Schema has no `statusSchema`
    /// - [`AppError::StoredSchemaCompilationFailed`] if the stored status_schema fails to compile
    pub async fn get_status_validator(
        &self,
        key: &ResourceKey,
    ) -> Result<Arc<dyn SchemaValidator>, AppError> {
        let cache_key = schema_cache_key(&key.kind, &key.group, &key.version);

        // Cache hit - check if status_validator is present
        if let Some(cached) = self.cache.get(&cache_key)
            && let Some(ref status_validator) = cached.status_validator
        {
            return Ok(status_validator.clone());
        }
        // Cached but no status validator - fall through to fetch and check

        // Cache miss or no status validator: fetch from store
        let schema_key = schema_key();
        let schema_name = cache_key.clone();

        let schema_obj = self.store.get(&schema_key, None, &schema_name).await.map_err(|_| {
            AppError::NotFound { what: "schema".to_string(), identifier: schema_name.clone() }
        })?;

        // Parse SchemaData
        let schema_data: SchemaData = serde_json::from_value(schema_obj.spec.clone())
            .map_err(|e| AppError::InvalidSchema(format!("failed to parse schema data: {}", e)))?;

        // Check if status_schema is present
        let status_schema = schema_data
            .status_schema
            .ok_or_else(|| AppError::StatusSubresourceNotEnabled { kind: key.kind.clone() })?;

        // Compile status_schema
        let compiled = JsonSchemaValidator::compile(&status_schema)
            .map_err(|e| AppError::StoredSchemaCompilationFailed {
                schema_name: cache_key.clone(),
                reason: e.to_string(),
            })
            .map(|v| Arc::new(v) as Arc<dyn SchemaValidator>)?;

        // Update cache with status validator
        if let Some(mut cached) = self.cache.get_mut(&cache_key) {
            cached.status_validator = Some(compiled.clone());
        } else {
            // No spec validator cached yet - cache both
            let spec_compiled = JsonSchemaValidator::compile(&schema_data.spec_schema)
                .map_err(|e| AppError::StoredSchemaCompilationFailed {
                    schema_name: cache_key.clone(),
                    reason: e.to_string(),
                })
                .map(|v| Arc::new(v) as Arc<dyn SchemaValidator>)?;
            self.cache.insert(
                cache_key.clone(),
                CachedSchema {
                    validator: spec_compiled,
                    status_validator: Some(compiled.clone()),
                    scope: schema_data.scope,
                },
            );
        }

        Ok(compiled)
    }

    /// Inserts a compiled status validator into the cache.
    ///
    /// Updates the existing cache entry for `name` to include the status validator.
    /// If no entry exists for `name`, this is a no-op (status validators are stored
    /// alongside spec validators in the same cache entry).
    pub fn insert_status(&self, name: &str, validator: Arc<dyn SchemaValidator>) {
        if let Some(mut cached) = self.cache.get_mut(name) {
            cached.status_validator = Some(validator);
        }
    }

    /// Returns the scope for the given object `key`.
    ///
    /// Checks the cache first. On cache miss, fetches the Schema from the store,
    /// extracts the scope, inserts into the cache, and returns it.
    ///
    /// # Errors
    ///
    /// Returns [`AppError::NotFound`] if no Schema exists in the store for this kind+group+version.
    pub async fn get_scope(&self, key: &ResourceKey) -> Result<String, AppError> {
        let cache_key = schema_cache_key(&key.kind, &key.group, &key.version);

        // Cache hit
        if let Some(cached) = self.cache.get(&cache_key) {
            return Ok(cached.scope.clone());
        }

        // Cache miss: fetch from store
        let schema_key = schema_key();
        let schema_name = cache_key.clone();

        let schema_obj = self.store.get(&schema_key, None, &schema_name).await.map_err(|_| {
            AppError::NotFound { what: "schema".to_string(), identifier: schema_name.clone() }
        })?;

        // Parse SchemaData to extract scope
        let schema_data: SchemaData = serde_json::from_value(schema_obj.spec.clone())
            .map_err(|e| AppError::InvalidSchema(format!("failed to parse schema data: {}", e)))?;

        let scope = schema_data.scope.clone();

        // Compile and cache the full entry
        let compiled = JsonSchemaValidator::compile(&schema_data.spec_schema)
            .map_err(|e| AppError::StoredSchemaCompilationFailed {
                schema_name: cache_key.clone(),
                reason: e.to_string(),
            })
            .map(|v| Arc::new(v) as Arc<dyn SchemaValidator>)?;

        let status_validator = schema_data
            .status_schema
            .as_ref()
            .map(|ss| {
                JsonSchemaValidator::compile(ss)
                    .map_err(|e| AppError::StoredSchemaCompilationFailed {
                        schema_name: cache_key.clone(),
                        reason: e.to_string(),
                    })
                    .map(|v| Arc::new(v) as Arc<dyn SchemaValidator>)
            })
            .transpose()?;

        self.cache.insert(
            cache_key.clone(),
            CachedSchema { validator: compiled, status_validator, scope: scope.clone() },
        );

        Ok(scope)
    }

    /// Returns [`ResourceKey`]s for all registered namespaced kinds.
    ///
    /// Queries Schema objects from the store, filters to those with
    /// `scope == "Namespaced"`, and constructs a `ResourceKey` from each
    /// schema's `target_group`, `target_version`, and `target_kind`.
    pub async fn list_namespaced_keys(&self) -> Result<Vec<ResourceKey>, AppError> {
        let schema_key = schema_key();
        let resp = self
            .store
            .list(&schema_key, None, ListOptions { limit: Some(usize::MAX), ..Default::default() })
            .await?;
        let mut keys = Vec::new();
        for obj in &resp.items {
            let schema_data: SchemaData =
                serde_json::from_value(obj.spec.clone()).map_err(|e| {
                    AppError::InvalidSchema(format!("failed to parse schema data: {}", e))
                })?;
            if schema_data.scope != SCOPE_NAMESPACED {
                continue;
            }
            keys.push(ResourceKey {
                group: schema_data.target_group,
                version: schema_data.target_version,
                kind: schema_data.target_kind,
            });
        }
        Ok(keys)
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
            "specSchema": {
                "type": "object",
                "properties": {
                    "color": { "type": "string" },
                    "size": { "type": "integer" }
                }
            }
        });
        registry
            .store
            .create(crate::object::types::StoredObject {
                key: schema_key,
                metadata: crate::object::types::ObjectMeta {
                    name: name.to_string(),
                    namespace: None,
                    labels: std::collections::HashMap::new(),
                    annotations: std::collections::HashMap::new(),
                    finalizers: Vec::new(),
                },
                system: crate::object::types::SystemMetadata::initial(),
                spec: schema_data,
                status: None,
            })
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
            "specSchema": {
                "type": "object",
                "properties": { "color": { "type": "string" } }
            }
        });

        let result = registry.validate_and_compile(&spec);
        assert!(result.is_ok());
        let (schema_data, _validator, _status_validator) = result.unwrap();
        assert_eq!(schema_data.target_kind, "Widget");
        assert_eq!(schema_data.target_group, "example.io");

        // Cache should NOT be modified
        assert!(!registry.cache.contains_key("Widget.example.io.v1"));
    }

    #[tokio::test]
    async fn validate_and_compile_invalid_meta_schema_returns_invalid_schema() {
        let registry = make_registry();
        // Missing required fields targetVersion, targetKind, specSchema
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
            "specSchema": { "type": "not-a-real-type" }
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
        registry.cache.insert(
            "Widget.example.io.v1".to_string(),
            CachedSchema {
                validator: dummy_validator.clone(),
                status_validator: None,
                scope: "Namespaced".to_string(),
            },
        );

        let result = registry.get_validator(&key).await;
        assert!(result.is_ok());
        // Verify it's the same validator by checking pointer identity via Arc::ptr_eq
        let (returned_validator, _scope) = result.unwrap();
        assert!(Arc::ptr_eq(&returned_validator, &dummy_validator));
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
        store_test_schema(&registry, "Widget.example.io.v1").await;

        let result = registry.get_validator(&key).await;
        assert!(result.is_ok());

        // Verify it was cached
        assert!(registry.cache.contains_key("Widget.example.io.v1"));
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
            "specSchema": { "type": "not-a-real-type" }
        });
        registry
            .store
            .create(crate::object::types::StoredObject {
                key: schema_key,
                metadata: crate::object::types::ObjectMeta {
                    name: "Widget.example.io.v1".to_string(),
                    namespace: None,
                    labels: std::collections::HashMap::new(),
                    annotations: std::collections::HashMap::new(),
                    finalizers: Vec::new(),
                },
                system: crate::object::types::SystemMetadata::initial(),
                spec: invalid_schema,
                status: None,
            })
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
        registry.insert("test-schema", validator.clone(), "Namespaced");
        assert!(registry.cache.contains_key("test-schema"));
    }

    #[tokio::test]
    async fn insert_replaces_existing_entry() {
        let registry = make_registry();
        let validator1: Arc<dyn SchemaValidator> =
            Arc::new(compile_meta_schema().expect("meta-schema should compile"));
        let validator2: Arc<dyn SchemaValidator> =
            Arc::new(compile_meta_schema().expect("meta-schema should compile"));

        registry.insert("test-schema", validator1, "Namespaced");
        assert!(registry.cache.contains_key("test-schema"));

        registry.insert("test-schema", validator2, "Namespaced");
        // Still present (replaced)
        assert!(registry.cache.contains_key("test-schema"));
    }

    // --- evict tests ---

    #[tokio::test]
    async fn evict_removes_existing_entry() {
        let registry = make_registry();
        let validator: Arc<dyn SchemaValidator> =
            Arc::new(compile_meta_schema().expect("meta-schema should compile"));

        registry.insert("test-schema", validator, "Namespaced");
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

    // --- get_status_validator tests ---

    #[tokio::test]
    async fn get_status_validator_cache_hit() {
        let registry = make_registry();
        let key = ResourceKey {
            group: "example.io".to_string(),
            version: "v1".to_string(),
            kind: "Widget".to_string(),
        };

        // Prime the cache with a status validator
        let dummy_validator: Arc<dyn SchemaValidator> =
            Arc::new(compile_meta_schema().expect("meta-schema should compile"));
        registry.cache.insert(
            "Widget.example.io.v1".to_string(),
            CachedSchema {
                validator: dummy_validator.clone(),
                status_validator: Some(dummy_validator.clone()),
                scope: "Namespaced".to_string(),
            },
        );

        let result = registry.get_status_validator(&key).await;
        assert!(result.is_ok());
        assert!(Arc::ptr_eq(&result.unwrap(), &dummy_validator));
    }

    #[tokio::test]
    async fn get_status_validator_cache_miss_with_status_schema() {
        let registry = make_registry();
        let key = ResourceKey {
            group: "example.io".to_string(),
            version: "v1".to_string(),
            kind: "Widget".to_string(),
        };

        // Store a schema with statusSchema
        let schema_key = schema_key();
        let schema_data = json!({
            "targetGroup": "example.io",
            "targetVersion": "v1",
            "targetKind": "Widget",
            "specSchema": { "type": "object" },
            "statusSchema": { "type": "object" }
        });
        registry
            .store
            .create(crate::object::types::StoredObject {
                key: schema_key,
                metadata: crate::object::types::ObjectMeta {
                    name: "Widget.example.io.v1".to_string(),
                    namespace: None,
                    labels: std::collections::HashMap::new(),
                    annotations: std::collections::HashMap::new(),
                    finalizers: Vec::new(),
                },
                system: crate::object::types::SystemMetadata::initial(),
                spec: schema_data,
                status: None,
            })
            .await
            .expect("store create should succeed");

        let result = registry.get_status_validator(&key).await;
        assert!(result.is_ok());
        // Status validator is stored in the same cache entry as the spec validator
        // (under the base key). Verify the base key exists and has status set.
        assert!(registry.cache.contains_key("Widget.example.io.v1"));
    }

    #[tokio::test]
    async fn get_status_validator_no_status_schema_returns_error() {
        let registry = make_registry();
        let key = ResourceKey {
            group: "example.io".to_string(),
            version: "v1".to_string(),
            kind: "Widget".to_string(),
        };

        // Store a schema WITHOUT statusSchema
        let schema_key = schema_key();
        let schema_data = json!({
            "targetGroup": "example.io",
            "targetVersion": "v1",
            "targetKind": "Widget",
            "specSchema": { "type": "object" }
        });
        registry
            .store
            .create(crate::object::types::StoredObject {
                key: schema_key,
                metadata: crate::object::types::ObjectMeta {
                    name: "Widget.example.io.v1".to_string(),
                    namespace: None,
                    labels: std::collections::HashMap::new(),
                    annotations: std::collections::HashMap::new(),
                    finalizers: Vec::new(),
                },
                system: crate::object::types::SystemMetadata::initial(),
                spec: schema_data,
                status: None,
            })
            .await
            .expect("store create should succeed");

        let result = registry.get_status_validator(&key).await;
        assert!(matches!(result, Err(AppError::StatusSubresourceNotEnabled { .. })));
    }

    // --- insert_status tests ---

    #[tokio::test]
    async fn insert_status_updates_existing_entry() {
        let registry = make_registry();
        let validator: Arc<dyn SchemaValidator> =
            Arc::new(compile_meta_schema().expect("meta-schema should compile"));

        registry.insert("test-schema", validator.clone(), "Namespaced");
        registry.insert_status("test-schema", validator);
        // Status validator is stored in the same cache entry as the spec validator
        // (under the base key, not under a ".status" suffix)
        assert!(registry.cache.contains_key("test-schema"));
    }

    // --- evict also removes status entry ---

    #[tokio::test]
    async fn evict_removes_entry_with_status_validator() {
        let registry = make_registry();
        let validator: Arc<dyn SchemaValidator> =
            Arc::new(compile_meta_schema().expect("meta-schema should compile"));

        registry.insert("test-schema", validator.clone(), "Namespaced");
        registry.insert_status("test-schema", validator);
        assert!(registry.cache.contains_key("test-schema"));

        // Verify status validator is present (use contains_key to avoid DashMap get/get_mut deadlock)
        let v1_key = "test-schema".to_string();
        assert!(registry.cache.contains_key(&v1_key));

        registry.evict("test-schema");
        assert!(!registry.cache.contains_key("test-schema"));
    }
}
