## ADDED Requirements

### Requirement: SchemaService wraps store, event bus, and schema registry
The system SHALL define a `SchemaService` struct containing:
- `store: Arc<dyn ObjectStore>` — the storage backend
- `event_bus: Arc<dyn EventPublisher>` — the per-kind event bus for watch notifications
- `schema_registry: SchemaRegistry` — schema compilation, caching, and lookup collaborator

The SchemaService SHALL own the SchemaRegistry. The SchemaService SHALL be the single orchestrator for Schema lifecycle operations: creation, update, and deletion of Schema objects.

#### Scenario: SchemaService construction
- **WHEN** `SchemaService::new(store, event_bus, meta_validator)` is called
- **THEN** the SchemaService SHALL construct a `SchemaRegistry` internally from `store` and `meta_validator`
- **AND** the registry's cache SHALL start empty
- **AND** no store query SHALL be performed during construction

### Requirement: SchemaService create validates, compiles, stores, caches, and publishes
The `SchemaService::create(key, meta, spec)` method SHALL:
1. Call `schema_registry.validate_and_compile(&spec)` to validate against meta-schema and compile the JSON Schema
2. If the spec contains a `statusSchema`, compile it and cache it via `schema_registry.insert_status()`
3. Construct a `StoredObject` with `system.resource_version = 1`, `system.generation = 1`, `system.created_at = Utc::now()`, `system.updated_at = Utc::now()`
4. Call `store.create()` to persist
5. Call `schema_registry.insert()` to cache the compiled validator
6. Call `event_bus.publish()` with an `Added` event

The SchemaService SHALL set `StoredObject.spec` to the `serde_json::Value` directly.

#### Scenario: Create valid Schema
- **WHEN** a Schema registration passes meta-schema validation and its jsonSchema compiles
- **THEN** the SchemaService SHALL construct a `StoredObject` with `resource_version = 1`, `generation = 1`, and current timestamps
- **AND** the schema SHALL be stored, the compiled validator SHALL be cached, and an `Added` event SHALL be published

#### Scenario: Create Schema with invalid meta-schema
- **WHEN** a Schema registration fails meta-schema validation
- **THEN** the error SHALL be `InvalidSchema` and nothing SHALL be stored or published

#### Scenario: Create Schema with uncompileable jsonSchema
- **WHEN** a Schema registration passes meta-schema validation but `jsonSchema` fails compilation
- **THEN** the error SHALL be `InvalidSchema` and nothing SHALL be stored or published

#### Scenario: Create Schema with statusSchema
- **WHEN** a Schema is created with `statusSchema` defined
- **THEN** both the spec validator and status validator SHALL be compiled and cached

### Requirement: SchemaService update recompiles, persists, and publishes
The `SchemaService::update(object)` method SHALL:
1. Call `schema_registry.validate_and_compile(&spec)` to revalidate and recompile
2. If the spec contains a `statusSchema`, compile it and cache it
3. Use `store.transaction()` with a callback that performs OCC check and returns `TransactionOp::Apply` with updated metadata via the centralized metadata wrapper
4. Call `schema_registry.insert()` to replace the cached validator
5. Call `event_bus.publish()` with a `Modified` event

#### Scenario: Update Schema with correct version
- **WHEN** `update` is called with a matching `resourceVersion`
- **THEN** the SchemaService SHALL recompile the schema, increment `resource_version`, preserve `created_at`, update `updated_at`, bump `generation` if spec changed, and publish a `Modified` event

#### Scenario: Update Schema with wrong version
- **WHEN** `update` is called with a stale `resourceVersion`
- **THEN** the transaction callback SHALL return `TransactionOp::Abort(AppError::Conflict)` and no event SHALL be published

### Requirement: SchemaService delete checks dependents, removes, evicts, and publishes
The `SchemaService::delete(key, name)` method SHALL:
1. Fetch the Schema from the store
2. Extract the target kind from the Schema
3. Check if objects of that kind exist using `store.exists()`
4. If objects exist, return `AppError::SchemaHasObjects { kind }`
5. Call `store.delete()` to remove the Schema
6. Call `schema_registry.evict()` to remove the cached validator (both spec and status)
7. Call `event_bus.publish()` with a `Deleted` event

#### Scenario: Delete Schema with no objects
- **WHEN** deleting a Schema and no objects of the target kind exist
- **THEN** the Schema SHALL be deleted, the cache entry SHALL be removed via `schema_registry.evict()`, a `Deleted` event SHALL be published, and the deleted object SHALL be returned

#### Scenario: Delete Schema with existing objects
- **WHEN** deleting a Schema and objects of the target kind exist
- **THEN** the error SHALL be `SchemaHasObjects { kind }` and nothing SHALL be deleted, evicted, or published

### Requirement: SchemaService provides get_validator for ObjectService
The `SchemaService` SHALL expose its `SchemaRegistry` (or provide a `get_validator` method) so that `ObjectService` can look up compiled validators for object validation.

#### Scenario: ObjectService looks up validator via SchemaService
- **WHEN** ObjectService needs to validate an object of kind `Widget` in group `example.io`
- **THEN** it SHALL obtain the validator from the SchemaService's registry
- **AND** the validator SHALL be returned from cache or compiled on-demand

### Requirement: SchemaService schema cache uses schema name as key
The SchemaRegistry cache SHALL be keyed by the Schema's `name` field (e.g., `"Widget.example.io"`). SchemaService SHALL pass the schema name to `schema_registry.insert()` and `schema_registry.evict()`.

#### Scenario: Cache insertion on Schema create
- **WHEN** a Schema is created with name `"Widget.example.io"`
- **THEN** `schema_registry.insert("Widget.example.io", compiled)` SHALL be called after successful store persistence

#### Scenario: Cache eviction on Schema delete
- **WHEN** a Schema with name `"Widget.example.io"` is deleted
- **THEN** `schema_registry.evict("Widget.example.io")` SHALL be called after successful store deletion

### Requirement: SchemaService uses centralized metadata wrapper
The SchemaService SHALL use the same centralized `apply_with_metadata` helper as ObjectService for transaction callbacks. The wrapper SHALL automatically handle `resource_version` increment, `generation` bumping, and timestamp management.

#### Scenario: Schema update uses shared metadata wrapper
- **WHEN** SchemaService updates a Schema
- **THEN** the transaction callback SHALL use `apply_with_metadata` to compute updated system metadata

### Requirement: SchemaService publishes events after mutations only
The SchemaService SHALL publish events only after successful store operations. If the store returns an error, no event SHALL be published.

#### Scenario: Failed Schema create does not publish
- **WHEN** Schema create fails due to validation error
- **THEN** no `Added` event SHALL be published

#### Scenario: Failed Schema update does not publish
- **WHEN** Schema update fails due to a version conflict
- **THEN** no `Modified` event SHALL be published
