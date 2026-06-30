## MODIFIED Requirements

### Requirement: SchemaRegistry wraps store, meta-validator, and cache
The system SHALL define a `SchemaRegistry` struct containing:
- `store: Arc<dyn ObjectStore>` — the storage backend for cache-miss lookups
- `meta_validator: Arc<dyn SchemaValidator>` — compiled meta-schema for Schema validation
- `cache: DashMap<String, CachedSchema>` — compiled user schemas keyed by versioned schema name

The `CachedSchema` struct SHALL contain `validator: Arc<dyn SchemaValidator>`, `status_validator: Option<Arc<dyn SchemaValidator>>`, and `scope: String`. The cache key for spec validators SHALL be `{kind}.{group}.{version}`. The cache key for status validators SHALL be `{kind}.{group}.{version}.status`.

#### Scenario: Registry construction
- **WHEN** `SchemaRegistry::new(store, meta_validator)` is called
- **THEN** the registry is constructed with an empty `cache`

### Requirement: get_validator returns cached or lazily compiled validator with scope
The `get_validator(key: &ResourceKey)` method SHALL:
1. Compute the cache key as `"{kind}.{group}.{version}"`
2. Check the cache for an existing entry
3. On cache hit, return the cached validator and scope
4. On cache miss, fetch the Schema from the store
5. Parse the Schema's data into `SchemaData` (including scope)
6. Compile `schema_data.json_schema`
7. Insert the compiled validator and scope into the cache
8. Return the compiled validator and scope

The method SHALL return `(Arc<dyn SchemaValidator>, String)` where the string is the scope.

#### Scenario: Cache hit returns cached validator and scope
- **WHEN** `get_validator` is called for a key whose validator exists in the cache
- **THEN** the cached validator and scope are returned without store access

#### Scenario: Cache miss compiles and caches validator with scope
- **WHEN** `get_validator` is called for a key not in cache but in store
- **THEN** the Schema is fetched, compiled, cached with scope, and validator + scope returned

### Requirement: get_scope returns scope for a kind
The `SchemaRegistry` SHALL provide a `get_scope(key: &ResourceKey) -> Result<String, AppError>` method that returns the scope for the given kind. On cache hit, it returns the cached scope. On cache miss, it fetches the Schema from the store, extracts the scope, caches it, and returns it.

#### Scenario: Get scope for cached kind
- **WHEN** `get_scope` is called for a kind in cache
- **THEN** the scope is returned without store access

#### Scenario: Get scope for uncached kind
- **WHEN** `get_scope` is called for a kind not in cache
- **THEN** the Schema is fetched, scope extracted, cached, and returned

### Requirement: insert adds validator and scope to cache
The `insert(name: &str, validator: Arc<dyn SchemaValidator>, scope: &str)` method SHALL insert the validator and scope into the cache under the given name, replacing any existing entry.

#### Scenario: Insert new validator with scope
- **WHEN** `insert("Widget.example.io.v1", validator, "Namespaced")` is called
- **THEN** the validator and scope are stored in the cache

### Requirement: SchemaRegistry caches status validators
When a Schema with `statusSchema` is created or updated, the status validator SHALL be compiled and cached alongside the scope. When a Schema is deleted, both spec and status cache entries SHALL be evicted.

#### Scenario: Schema with statusSchema caches both validators and scope
- **WHEN** a Schema with `statusSchema` and `scope: "Cluster"` is registered
- **THEN** the spec validator, status validator, and scope are cached
