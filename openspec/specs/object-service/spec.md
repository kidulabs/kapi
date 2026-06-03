## Purpose

Define the `ObjectService` that orchestrates validation, storage, and event publishing for all object operations. The service is the single entry point for object CRUD — handlers call the service, never the store directly.
## Requirements
### Requirement: ObjectService wraps store, event bus, and schema registry
The system SHALL define an `ObjectService` struct containing:
- `store: Arc<dyn ObjectStore>` — the storage backend
- `event_bus: Arc<dyn EventPublisher>` — the per-kind event bus for watch notifications
- `schema_registry: SchemaRegistry` — schema compilation, caching, and lookup collaborator

#### Scenario: Service construction with SchemaRegistry
- **WHEN** `ObjectService::new(store, event_bus, meta_validator)` is called
- **THEN** the service constructs a `SchemaRegistry` internally from `store` and `meta_validator`
- **AND** the registry's cache starts empty
- **AND** no store query is performed during construction

### Requirement: create delegates schema work to SchemaRegistry
The `create(key, meta, spec)` method SHALL:
1. Validate `meta.labels` using label validation rules (key format, value format, length limits)
2. If `key.kind == "Schema"`: call `schema_registry.validate_and_compile(&spec)` to validate and compile, then `store.create()`, then `schema_registry.insert()` to cache, then `event_bus.publish()`
3. If `key.kind != "Schema"`: call `schema_registry.get_validator(&key)` to obtain the validator, validate `spec` against it, then `store.create()`, then `event_bus.publish()`

Label validation SHALL occur after schema validation of the spec payload but before store persistence. If label validation fails, an `AppError::InvalidLabel` error SHALL be returned with a descriptive message.

#### Scenario: Create valid Schema
- **WHEN** a Schema registration passed meta-schema validation and its jsonSchema compiles
- **THEN** the schema is stored with the generated name, the compiled validator is cached under that name, and an `Added` event is published

#### Scenario: Create Schema with invalid meta-schema
- **WHEN** a Schema registration fails meta-schema validation
- **THEN** the error is `InvalidSchema` and nothing is stored or published

#### Scenario: Create Schema with uncompileable jsonSchema
- **WHEN** a Schema registration passes meta-schema validation but `jsonSchema` fails compilation
- **THEN** the error is `InvalidSchema` and nothing is stored or published

#### Scenario: Create object for unregistered kind
- **WHEN** creating an object for a kind with no registered Schema
- **THEN** the error is `NotFound` (no schema found for this kind)

#### Scenario: Create object with invalid spec
- **WHEN** creating an object whose spec fails schema validation
- **THEN** the error is `SchemaValidation` with the list of validation errors

#### Scenario: Create duplicate object
- **WHEN** creating an object with a name that already exists
- **THEN** the store returns `AlreadyExists` and no event is published

#### Scenario: Create object with schema not in cache but in store
- **WHEN** creating an object for a kind whose Schema exists in the store but is not in the cache
- **THEN** `schema_registry.get_validator()` fetches the schema from the store, compiles it on-demand, caches it, and the object is validated against it

#### Scenario: Create object with stored schema that fails compilation
- **WHEN** creating an object for a kind whose Schema exists in the store but whose `jsonSchema` fails compilation
- **THEN** `schema_registry.get_validator()` returns `AppError::StoredSchemaCompilationFailed`
- **AND** no object is created

#### Scenario: Create with valid labels
- **WHEN** `create()` is called with valid labels
- **THEN** validation SHALL pass and the object SHALL be persisted with labels

#### Scenario: Create with invalid label key
- **WHEN** `create()` is called with a label key that violates format rules
- **THEN** an `AppError::InvalidLabel` error SHALL be returned with a descriptive message

#### Scenario: Create with invalid label value
- **WHEN** `create()` is called with a label value that violates format rules
- **THEN** an `AppError::InvalidLabel` error SHALL be returned with a descriptive message

#### Scenario: Create with empty labels map
- **WHEN** `create()` is called with an empty labels `HashMap`
- **THEN** validation SHALL pass and the object SHALL be persisted

### Requirement: get delegates to store
The `get(key, name)` method SHALL delegate to `store.get(key, name)` without additional validation.

#### Scenario: Get existing object
- **WHEN** `get` is called for an existing object
- **THEN** the `StoredObject` is returned

#### Scenario: Get missing object
- **WHEN** `get` is called for a non-existent object
- **THEN** the error is `NotFound`

### Requirement: list delegates to store
The `list(key, opts)` method SHALL delegate to `store.list(key, opts)` without additional validation.

#### Scenario: List objects with pagination
- **WHEN** `list` is called with limit and continue token
- **THEN** the paginated `ListResponse` is returned

### Requirement: update delegates schema work to SchemaRegistry
The `update(object)` method SHALL:
1. Validate `object.metadata.labels` using label validation rules
2. If `object.key.kind == "Schema"`: call `schema_registry.validate_and_compile(&spec)`, then `store.update()`, then `schema_registry.insert()`, then `event_bus.publish()`
3. If `object.key.kind != "Schema"`: call `schema_registry.get_validator(&key)`, validate spec, then `store.update()`, then `event_bus.publish()`

Label validation SHALL occur after schema validation but before store persistence. If label validation fails, an `AppError::InvalidLabel` error SHALL be returned and no persistence SHALL occur.

#### Scenario: Update object with schema not in cache but in store
- **WHEN** updating an object for a kind whose Schema exists in the store but is not in the cache
- **THEN** `schema_registry.get_validator()` compiles the schema on-demand, caches it, and the object is validated against it

#### Scenario: Update with correct version
- **WHEN** `update` is called with a matching `resourceVersion`
- **THEN** the object is updated, a `Modified` event is published, and the updated object is returned

#### Scenario: Update with wrong version
- **WHEN** `update` is called with a stale `resourceVersion`
- **THEN** the store returns `Conflict` and no event is published

#### Scenario: Update with valid labels
- **WHEN** `update()` is called with valid labels
- **THEN** validation SHALL pass and the object SHALL be persisted with updated labels

#### Scenario: Update with invalid labels
- **WHEN** `update()` is called with invalid labels
- **THEN** an `AppError::InvalidLabel` error SHALL be returned and no persistence SHALL occur

### Requirement: delete delegates cache eviction to SchemaRegistry
The `delete(key, name)` method SHALL:
1. If `key.kind == "Schema"`: fetch the Schema, extract target kind, check if objects of that kind exist using `store.exists()`; if so, return `SchemaHasObjects`. Then `store.delete()`, then `schema_registry.evict()`, then `event_bus.publish()`
2. If `key.kind != "Schema"`: `store.delete()`, then `event_bus.publish()`

#### Scenario: Delete Schema with no objects
- **WHEN** deleting a Schema and no objects of the target kind exist
- **THEN** the Schema is deleted, the cache entry is removed via `schema_registry.evict()`, a `Deleted` event is published, and the deleted object is returned

#### Scenario: Delete Schema with existing objects
- **WHEN** deleting a Schema and objects of the target kind exist
- **THEN** the error is `SchemaHasObjects { kind }` and nothing is deleted, evicted, or published

#### Scenario: Delete regular object
- **WHEN** deleting a non-Schema object
- **THEN** the object is deleted, a `Deleted` event is published, and the deleted object is returned

### Requirement: Schema cache uses schema name as key
The `SchemaRegistry` cache SHALL be keyed by the Schema's `name` field (e.g., `"Widget.example.io"`). `ObjectService` SHALL pass the schema name to `schema_registry.insert()` and `schema_registry.evict()`.

#### Scenario: Cache insertion on Schema create
- **WHEN** a Schema is created with name `"Widget.example.io"`
- **THEN** `schema_registry.insert("Widget.example.io", compiled)` is called after successful store persistence

#### Scenario: Cache lookup for object validation
- **WHEN** validating an object of kind `Widget` in group `example.io`
- **THEN** `schema_registry.get_validator()` queries the cache with key `"Widget.example.io"`

#### Scenario: Cache eviction on Schema delete
- **WHEN** a Schema with name `"Widget.example.io"` is deleted
- **THEN** `schema_registry.evict("Widget.example.io")` is called after successful store deletion

### Requirement: Service provides subscribe() with WatchFilter for SSE watch
The system SHALL provide an `ObjectService::subscribe(&self, key: &ResourceKey, filter: WatchFilter) -> WatchStream` method that delegates to the internal `EventPublisher::subscribe(key, filter)`.

#### Scenario: Subscribe with WatchFilter::All returns a WatchStream
- **WHEN** `object_service.subscribe(&key, WatchFilter::All)` is called
- **THEN** a `WatchStream` is returned that delivers all events for the given resource key

#### Scenario: Subscribe with WatchFilter::FieldSelector returns a filtered WatchStream
- **WHEN** `object_service.subscribe(&key, WatchFilter::FieldSelector(FieldSelector::NameEquals("my-widget".into())))` is called
- **THEN** a `WatchStream` is returned that delivers only events matching the filter

### Requirement: Schema compilation uses JsonSchemaValidator via SchemaRegistry
The system SHALL compile user schemas via `SchemaRegistry::validate_and_compile()`, which internally uses `JsonSchemaValidator::compile()`. `ObjectService` SHALL NOT directly call `JsonSchemaValidator::compile()`.

#### Scenario: Schema compiled via SchemaRegistry
- **WHEN** a Schema registration payload passes meta-schema validation
- **THEN** `schema_registry.validate_and_compile()` calls `JsonSchemaValidator::compile()` internally
- **AND** the resulting `Arc<dyn SchemaValidator>` is cached via `schema_registry.insert()`

### Requirement: Schema registration compiles status validator
When a Schema is created or updated with a `statusSchema` field, the `ObjectService` SHALL compile the `statusSchema` into a validator and cache it alongside the spec validator in the `SchemaRegistry`. The cache key for the status validator SHALL be `{kind}.{group}.status`.

#### Scenario: Schema with statusSchema is registered
- **WHEN** a Schema is created with `statusSchema` defined
- **THEN** both the spec validator and status validator are compiled and cached

#### Scenario: Schema without statusSchema is registered
- **WHEN** a Schema is created without `statusSchema`
- **THEN** only the spec validator is compiled and cached

#### Scenario: Schema with invalid statusSchema fails registration
- **WHEN** a Schema is created with a `statusSchema` that fails JSON Schema compilation
- **THEN** the error is `AppError::InvalidSchema` and nothing is stored or cached

### Requirement: ObjectService create ignores status field
The `create` method SHALL ignore any `status` field present in the request body. Objects are always created with `status: None`.

#### Scenario: Create with status in body
- **WHEN** `create` is called with a body containing a `status` field
- **THEN** the `status` field SHALL be removed from the body before storage
- **AND** the created object SHALL have `status: None`

### Requirement: ObjectService get_status method
The `ObjectService` SHALL provide a `get_status(key: ResourceKey, name: String)` method that:
1. Fetches the object from the store
2. Looks up the Schema for the given kind to check if `statusSchema` is defined
3. If no `statusSchema` exists, returns `AppError::StatusSubresourceNotEnabled { kind }`
4. Returns the `status` field of the object (may be `None`/`null`)

#### Scenario: Get status for kind with statusSchema
- **WHEN** `get_status` is called for a kind with `statusSchema`
- **THEN** the `status` field of the object is returned

#### Scenario: Get status for kind without statusSchema
- **WHEN** `get_status` is called for a kind without `statusSchema`
- **THEN** the error is `AppError::StatusSubresourceNotEnabled { kind }`

### Requirement: ObjectService update_status method
The `ObjectService` SHALL provide an `update_status(key: ResourceKey, name: String, status: Value)` method that:
1. Looks up the Schema for the given kind to check if `statusSchema` is defined
2. If no `statusSchema` exists, returns `AppError::StatusSubresourceNotEnabled { kind }`
3. Fetches the current object from the store
4. Validates the status value against the `statusSchema`
5. If validation fails, returns `AppError::SchemaValidation`
6. Calls `store.update_status(key, name, status)` to perform the server-side read-modify-write
7. Publishes a `WatchEvent` with `event_type: StatusModified` containing the updated `StoredObject`
8. Returns the updated `StoredObject`

#### Scenario: Update status for kind with statusSchema
- **WHEN** `update_status` is called for a kind with `statusSchema` defined
- **THEN** the status is validated against `statusSchema`, stored, and a `StatusModified` event is published

#### Scenario: Update status for kind without statusSchema
- **WHEN** `update_status` is called for a kind without `statusSchema`
- **THEN** the error is `AppError::StatusSubresourceNotEnabled { kind }`

#### Scenario: Update status with invalid status
- **WHEN** `update_status` is called with a status value that fails `statusSchema` validation
- **THEN** the error is `AppError::SchemaValidation` with the list of validation errors

#### Scenario: Update status for non-existent object
- **WHEN** `update_status` is called for an object that does not exist
- **THEN** the error is `AppError::NotFound`

### Requirement: Validation error mapping in object operations
The system SHALL map `SchemaValidationError` from `SchemaValidator::validate()` to the domain `ValidationError` type when validating regular objects. Meta-schema validation errors are handled internally by `SchemaRegistry::validate_and_compile()` and returned as `AppError::InvalidSchema`. A `validate_labels()` function SHALL validate a `HashMap<String, String>` against label validation rules, returning `Result<(), AppError>` with descriptive error messages identifying the offending key or value.

#### Scenario: Meta-schema errors mapped to strings
- **WHEN** meta-schema validation fails during Schema create or update
- **THEN** `schema_registry.validate_and_compile()` collects error messages and returns `AppError::InvalidSchema`

#### Scenario: Object validation errors mapped to ValidationError
- **WHEN** object validation fails during object create or update
- **THEN** `SchemaValidationError` values are mapped to `object::types::ValidationError { path, message }` and returned as `AppError::SchemaValidation`

#### Scenario: Validate key with prefix
- **WHEN** `validate_labels()` is called with key `app.example.io/name`
- **THEN** validation SHALL check prefix format (DNS subdomain, max 253 chars) and name format (max 256 chars, valid characters)

#### Scenario: Validate empty value
- **WHEN** `validate_labels()` is called with a label whose value is an empty string
- **THEN** validation SHALL pass (empty values are allowed)

### Requirement: Service publishes events after mutations only
The service SHALL publish events only after successful store operations. If the store returns an error, no event is published.

#### Scenario: Failed create does not publish
- **WHEN** `create` fails due to a duplicate (AlreadyExists) or validation error
- **THEN** no `Added` event is published

#### Scenario: Failed update does not publish
- **WHEN** `update` fails due to a version conflict
- **THEN** no `Modified` event is published

