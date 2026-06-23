## MODIFIED Requirements

### Requirement: SchemaRegistry wraps store, meta-validator, and cache
The system SHALL define a `SchemaRegistry` struct containing:
- `store: Arc<dyn ObjectStore>` — the storage backend for cache-miss lookups
- `meta_validator: Arc<dyn SchemaValidator>` — compiled meta-schema for Schema validation
- `cache: DashMap<String, Arc<dyn SchemaValidator>>` — compiled user schemas keyed by `{kind}.{group}.{version}` (e.g., `"Widget.example.io.v1"`)

The registry SHALL cache status validators alongside spec validators. The cache key for status validators SHALL be `{kind}.{group}.{version}.status`. The SchemaRegistry SHALL provide a `schema_cache_key(kind, group, version) -> String` helper in `src/schema/mod.rs` to centralize the cache key format; all cache keys SHALL be derived through this helper.

#### Scenario: Registry construction
- **WHEN** `SchemaRegistry::new(store, meta_validator)` is called
- **THEN** the registry is constructed with an empty `cache`
- **AND** no store query is performed during construction

### Requirement: get_validator returns cached or lazily compiled validator
The `get_validator(key: &ResourceKey)` method SHALL:
1. Compute the cache key as `schema_cache_key(key.kind, key.group, key.version)` (yielding `"{kind}.{group}.{version}"`) from the provided `ResourceKey`
2. Check the cache for an existing validator
3. On cache hit, return the cached validator
4. On cache miss, fetch the Schema from the store using `schema_key()` and the same cache key as the schema name
5. Parse the fetched Schema's data into `SchemaData`
6. Compile `schema_data.spec_schema`
7. Insert the compiled validator into the cache
8. Return the compiled validator

#### Scenario: Cache hit returns cached validator
- **WHEN** `get_validator` is called for a key whose validator exists in the cache
- **THEN** the cached validator is returned without any store access

#### Scenario: Cache miss compiles and caches validator
- **WHEN** `get_validator` is called for a key whose validator is not in the cache but whose Schema exists in the store
- **THEN** the Schema is fetched from the store, compiled, inserted into the cache, and the compiled validator is returned

#### Scenario: Cache miss with no Schema in store returns NotFound
- **WHEN** `get_validator` is called for a key whose Schema does not exist in the store
- **THEN** the method returns `Err(AppError::NotFound { what: "schema", identifier: schema_name })` where `schema_name` is the versioned name

#### Scenario: Cache miss with uncompilable stored schema returns StoredSchemaCompilationFailed
- **WHEN** `get_validator` is called for a key whose Schema exists in the store but whose `specSchema` fails compilation
- **THEN** the method returns `Err(AppError::StoredSchemaCompilationFailed { schema_name, reason })` where `schema_name` is the versioned name

### Requirement: SchemaRegistry caches status validators
When a Schema with `statusSchema` is created or updated, the status validator SHALL be compiled and cached. The cache key for status validators SHALL be `{kind}.{group}.{version}.status`. When a Schema is deleted, both spec and status cache entries SHALL be evicted.

#### Scenario: Schema with statusSchema caches both validators
- **WHEN** a Schema with `statusSchema` is registered for `example.io/v1/Widget`
- **THEN** the spec validator is cached under `"Widget.example.io.v1"` and the status validator is cached under `"Widget.example.io.v1.status"`

#### Scenario: Schema without statusSchema caches only spec validator
- **WHEN** a Schema without `statusSchema` is registered for `example.io/v1/Widget`
- **THEN** only the spec validator is cached under `"Widget.example.io.v1"`

#### Scenario: Schema deletion evicts both validators
- **WHEN** a Schema with `statusSchema` is deleted
- **THEN** both `"{kind}.{group}.{version}"` and `"{kind}.{group}.{version}.status"` cache entries are evicted

### Requirement: SchemaRegistry get_status_validator method
The `SchemaRegistry` SHALL provide a `get_status_validator(&self, key: &ResourceKey)` method that returns the cached status validator for the given kind. On cache miss, it SHALL fetch the Schema from the store (using the versioned schema name `"{kind}.{group}.{version}"` as the lookup key), parse `status_schema`, compile it, cache it under `"{kind}.{group}.{version}.status"`, and return it. If the Schema has no `status_schema`, it SHALL return `AppError::StatusSubresourceNotEnabled`.

#### Scenario: Get status validator for kind with statusSchema
- **WHEN** `get_status_validator` is called for a kind with `statusSchema`
- **THEN** the compiled status validator is returned (from cache or on-demand compilation)

#### Scenario: Get status validator for kind without statusSchema
- **WHEN** `get_status_validator` is called for a kind without `statusSchema`
- **THEN** the error is `AppError::StatusSubresourceNotEnabled { kind }`

#### Scenario: Get status validator cache miss
- **WHEN** `get_status_validator` is called and the cache does not contain the status validator
- **THEN** the Schema is fetched from the store using the versioned name, `status_schema` is compiled, cached, and returned

### Requirement: insert adds validator to cache
The `insert(name: &str, validator: Arc<dyn SchemaValidator>)` method SHALL insert the validator into the cache under the given name, replacing any existing entry. The `name` SHALL be the versioned schema name (`{kind}.{group}.{version}`) as produced by the handler.

#### Scenario: Insert new validator
- **WHEN** `insert("Widget.example.io.v1", validator)` is called and no entry exists for that name
- **THEN** the validator is stored in the cache under `"Widget.example.io.v1"`

#### Scenario: Insert replaces existing validator
- **WHEN** `insert("Widget.example.io.v1", new_validator)` is called and an entry already exists for that name
- **THEN** the existing entry is replaced with `new_validator`

### Requirement: evict removes validator from cache
The `evict(name: &str)` method SHALL remove the spec validator entry for the given name from the cache. It SHALL also remove the status validator entry for `{name}.status`. If no entry exists, this is a no-op.

#### Scenario: Evict existing entry
- **WHEN** `evict("Widget.example.io.v1")` is called and an entry exists for that name
- **THEN** the spec entry is removed from the cache and the status entry `"Widget.example.io.v1.status"` is also removed

#### Scenario: Evict non-existent entry is no-op
- **WHEN** `evict("NonExistent.example.io.v1")` is called and no entry exists for that name
- **THEN** no error occurs and the cache is unchanged

## ADDED Requirements

### Requirement: Multiple versions of the same kind coexist with independent cache entries
The SchemaRegistry SHALL allow two Schemas with the same `targetKind` and `targetGroup` but different `targetVersion` to be registered simultaneously. Their compiled spec validators and status validators SHALL occupy independent cache entries keyed by the versioned name, and SHALL NOT interfere with each other.

#### Scenario: Two versions cache independently
- **WHEN** Schemas are registered for `example.io/v1/Widget` and `example.io/v2/Widget`
- **THEN** the cache contains both `"Widget.example.io.v1"` and `"Widget.example.io.v2"`
- **AND** each entry holds a validator for that version's `specSchema`

#### Scenario: Two versions validate independently
- **WHEN** Schemas are registered for `example.io/v1/Widget` (requiring `color: string`) and `example.io/v2/Widget` (requiring `weight: number`)
- **THEN** an object posted to `/apis/example.io/v1/Widget` is validated against the v1 spec schema
- **AND** an object posted to `/apis/example.io/v2/Widget` is validated against the v2 spec schema
- **AND** a payload satisfying v1 but not v2 is accepted at v1 and rejected at v2

#### Scenario: Evicting one version does not affect the other
- **WHEN** the v1 Schema is deleted and `evict("Widget.example.io.v1")` is called
- **THEN** the cache no longer contains `"Widget.example.io.v1"` or `"Widget.example.io.v1.status"`
- **AND** the cache still contains `"Widget.example.io.v2"` and `"Widget.example.io.v2.status"`
