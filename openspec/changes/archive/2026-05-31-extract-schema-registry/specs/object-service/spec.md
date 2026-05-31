## Purpose

Delta spec for `ObjectService` modifications resulting from the `SchemaRegistry` extraction. This spec describes only the changes to the existing `object-service` capability.

## Requirements

### Requirement: ObjectService wraps store, event bus, and schema registry
The system SHALL define an `ObjectService` struct containing:
- `store: Arc<dyn ObjectStore>` — the storage backend
- `event_bus: Arc<dyn EventPublisher>` — the per-kind event bus for watch notifications
- `schema_registry: SchemaRegistry` — schema compilation, caching, and lookup collaborator

The `meta_validator` and `schema_cache` fields SHALL be removed. Their responsibilities are delegated to `SchemaRegistry`.

#### Scenario: Service construction with SchemaRegistry
- **WHEN** `ObjectService::new(store, event_bus, meta_validator)` is called
- **THEN** the service constructs a `SchemaRegistry` internally from `store` and `meta_validator`
- **AND** the registry's cache starts empty
- **AND** no store query is performed during construction

### Requirement: create delegates schema work to SchemaRegistry
The `create(key, meta, data)` method SHALL:
1. Validate `meta.labels` using label validation rules
2. If `key.kind == "Schema"`: call `schema_registry.validate_and_compile(&data)` to validate and compile, then `store.create()`, then `schema_registry.insert()` to cache, then `event_bus.publish()`
3. If `key.kind != "Schema"`: call `schema_registry.get_validator(&key)` to obtain the validator, validate `data` against it, then `store.create()`, then `event_bus.publish()`

#### Scenario: Create valid Schema
- **WHEN** a Schema registration passes label validation and `schema_registry.validate_and_compile()` succeeds
- **THEN** the schema is stored, the compiled validator is cached via `schema_registry.insert()`, and an `Added` event is published

#### Scenario: Create Schema with invalid meta-schema
- **WHEN** `schema_registry.validate_and_compile()` returns `Err(AppError::InvalidSchema)`
- **THEN** nothing is stored, cached, or published

#### Scenario: Create object for unregistered kind
- **WHEN** `schema_registry.get_validator()` returns `Err(AppError::NotFound)`
- **THEN** nothing is stored or published

#### Scenario: Create object with invalid data
- **WHEN** `schema_registry.get_validator()` succeeds but data fails validation against the returned validator
- **THEN** the error is `SchemaValidation` and nothing is stored or published

### Requirement: update delegates schema work to SchemaRegistry
The `update(object)` method SHALL:
1. Validate `object.metadata.labels` using label validation rules
2. If `object.key.kind == "Schema"`: call `schema_registry.validate_and_compile(&data)`, then `store.update()`, then `schema_registry.insert()`, then `event_bus.publish()`
3. If `object.key.kind != "Schema"`: call `schema_registry.get_validator(&key)`, validate data, then `store.update()`, then `event_bus.publish()`

#### Scenario: Update Schema with valid data
- **WHEN** updating a Schema that passes label validation and `schema_registry.validate_and_compile()` succeeds
- **THEN** the schema is updated, the compiled validator replaces the cached entry via `schema_registry.insert()`, and a `Modified` event is published

#### Scenario: Update object with valid data
- **WHEN** updating an object that passes label validation and schema validation via `schema_registry.get_validator()`
- **THEN** the object is updated and a `Modified` event is published

### Requirement: delete delegates cache eviction to SchemaRegistry
The `delete(key, name)` method SHALL:
1. If `key.kind == "Schema"`: fetch the Schema, extract target kind, check if objects of that kind exist; if so, return `SchemaHasObjects`. Then `store.delete()`, then `schema_registry.evict()`, then `event_bus.publish()`
2. If `key.kind != "Schema"`: `store.delete()`, then `event_bus.publish()`

#### Scenario: Delete Schema with no objects
- **WHEN** deleting a Schema and no objects of the target kind exist
- **THEN** the Schema is deleted, the cache entry is removed via `schema_registry.evict()`, and a `Deleted` event is published

#### Scenario: Delete Schema with existing objects
- **WHEN** deleting a Schema and objects of the target kind exist
- **THEN** the error is `SchemaHasObjects { kind, count }` and nothing is deleted, evicted, or published

### Requirement: Schema cache uses schema name as key
The `SchemaRegistry` cache SHALL be keyed by the Schema's `name` field (e.g., `"Widget.example.io"`). `ObjectService` SHALL pass the schema name to `schema_registry.insert()` and `schema_registry.evict()`.

#### Scenario: Cache insertion on Schema create
- **WHEN** a Schema is created with name `"Widget.example.io"`
- **THEN** `schema_registry.insert("Widget.example.io", compiled)` is called after successful store persistence

#### Scenario: Cache eviction on Schema delete
- **WHEN** a Schema with name `"Widget.example.io"` is deleted
- **THEN** `schema_registry.evict("Widget.example.io")` is called after successful store deletion
