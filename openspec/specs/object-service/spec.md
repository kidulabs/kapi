## Purpose

Define the `ObjectService` that orchestrates validation, storage, and event publishing for all object operations. The service is the single entry point for object CRUD — handlers call the service, never the store directly.
## Requirements
### Requirement: ObjectService wraps store, event bus, and validators
The system SHALL define an `ObjectService` struct containing:
- `store: Arc<dyn ObjectStore>` — the storage backend
- `event_bus: Arc<dyn EventPublisher>` — the per-kind event bus for watch notifications
- `meta_validator: Arc<dyn SchemaValidator>` — compiled meta-schema for Schema validation
- `schema_cache: DashMap<String, Arc<dyn SchemaValidator>>` — compiled user schemas keyed by schema name (e.g., `"Widget.example.io"`)

#### Scenario: Service construction without schema warmup
- **WHEN** `ObjectService::new(store, event_bus, meta_validator)` is called
- **THEN** the service is constructed with an empty `schema_cache`
- **AND** no store query is performed during construction

### Requirement: create validates and stores objects
The `create(key, meta, data)` method SHALL:
1. If `key.kind == "Schema"`: validate `data` against `meta_validator`, compile `data.jsonSchema` via `compile_jsonschema()`, cache the compiled validator under the name from `meta.name` (which is generated as `{targetKind}.{targetGroup}` by the handler)
2. If `key.kind != "Schema"`: look up the Schema from the store via `lookup_object_validator()`, which compiles on cache miss if the schema exists in the store
3. Call `store.create(key, meta, data)`
4. Call `event_bus.publish(key, WatchEvent::Added(obj))`
5. Return the created `StoredObject`

#### Scenario: Create valid Schema
- **WHEN** a Schema registration passes meta-schema validation and its jsonSchema compiles
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

#### Scenario: Create object with invalid data
- **WHEN** creating an object whose data fails schema validation
- **THEN** the error is `SchemaValidation` with the list of validation errors

#### Scenario: Create duplicate object
- **WHEN** creating an object with a name that already exists
- **THEN** the store returns `AlreadyExists` and no event is published

#### Scenario: Create object with schema not in cache but in store
- **WHEN** creating an object for a kind whose Schema exists in the store but is not in the cache
- **THEN** the schema is compiled on-demand, cached, and the object is validated against it

#### Scenario: Create object with stored schema that fails compilation
- **WHEN** creating an object for a kind whose Schema exists in the store but whose `jsonSchema` fails compilation
- **THEN** `lookup_object_validator()` returns `AppError::StoredSchemaCompilationFailed`
- **AND** no object is created

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

### Requirement: update validates and stores objects
The `update(object)` method SHALL:
1. Determine if the object is a Schema or regular object based on `object.key.kind`
2. Apply the same validation flow as `create` (meta-schema for Schema, compiled schema for objects)
3. Call `store.update(object)`
4. Call `event_bus.publish(key, WatchEvent::Modified(obj))`
5. Return the updated `StoredObject`

#### Scenario: Update object with schema not in cache but in store
- **WHEN** updating an object for a kind whose Schema exists in the store but is not in the cache
- **THEN** `lookup_object_validator()` compiles the schema on-demand, caches it, and the object is validated against it

#### Scenario: Update with correct version
- **WHEN** `update` is called with a matching `resourceVersion`
- **THEN** the object is updated, a `Modified` event is published, and the updated object is returned

#### Scenario: Update with wrong version
- **WHEN** `update` is called with a stale `resourceVersion`
- **THEN** the store returns `Conflict` and no event is published

### Requirement: delete guards Schema deletion and publishes event
The `delete(key, name)` method SHALL:
1. If `key.kind == "Schema"`: fetch the Schema, extract target kind, check if objects of that kind exist; if so, return `SchemaHasObjects`
2. Call `store.delete(key, name)`
3. Remove the compiled schema from `schema_cache` (if it was a Schema)
4. Call `event_bus.publish(key, WatchEvent::Deleted(obj))`
5. Return the deleted `StoredObject`

#### Scenario: Delete Schema with no objects
- **WHEN** deleting a Schema and no objects of the target kind exist
- **THEN** the Schema is deleted, the cache entry is removed, a `Deleted` event is published, and the deleted object is returned

#### Scenario: Delete Schema with existing objects
- **WHEN** deleting a Schema and objects of the target kind exist
- **THEN** the error is `SchemaHasObjects { kind, count }` and nothing is deleted or published

#### Scenario: Delete regular object
- **WHEN** deleting a non-Schema object
- **THEN** the object is deleted, a `Deleted` event is published, and the deleted object is returned

### Requirement: Schema cache uses schema name as key
The `schema_cache` SHALL be keyed by the Schema's `name` field (e.g., `"Widget.example.io"`), not by the Schema's `ResourceKey`. This allows lookup by the same name format used in schema registration URLs.

#### Scenario: Cache insertion on Schema create
- **WHEN** a Schema is created with name `"Widget.example.io"`
- **THEN** the compiled validator is cached under key `"Widget.example.io"`

#### Scenario: Cache lookup for object validation
- **WHEN** validating an object of kind `Widget` in group `example.io`
- **THEN** the cache is queried with key `"Widget.example.io"`

#### Scenario: Cache eviction on Schema delete
- **WHEN** a Schema with name `"Widget.example.io"` is deleted
- **THEN** the cache entry for `"Widget.example.io"` is removed

### Requirement: Service provides subscribe() with WatchFilter for SSE watch
The system SHALL provide an `ObjectService::subscribe(&self, key: &ResourceKey, filter: WatchFilter) -> WatchStream` method that delegates to the internal `EventPublisher::subscribe(key, filter)`.

#### Scenario: Subscribe with WatchFilter::All returns a WatchStream
- **WHEN** `object_service.subscribe(&key, WatchFilter::All)` is called
- **THEN** a `WatchStream` is returned that delivers all events for the given resource key

#### Scenario: Subscribe with WatchFilter::FieldSelector returns a filtered WatchStream
- **WHEN** `object_service.subscribe(&key, WatchFilter::FieldSelector(FieldSelector::NameEquals("my-widget".into())))` is called
- **THEN** a `WatchStream` is returned that delivers only events matching the filter

### Requirement: Schema compilation uses JsonSchemaValidator
The system SHALL compile user schemas during `create` and `update` operations using `JsonSchemaValidator::compile(&schema_data.json_schema)` instead of calling `draft202012::options().build()` directly.

#### Scenario: Schema compiled via JsonSchemaValidator
- **WHEN** a Schema registration payload passes meta-schema validation
- **THEN** `JsonSchemaValidator::compile()` is called to compile the `jsonSchema` field
- **AND** the resulting `JsonSchemaValidator` is cached as `Arc<dyn SchemaValidator>`

### Requirement: Validation error mapping in object operations
The system SHALL map `SchemaValidationError` from `SchemaValidator::validate()` to the domain `ValidationError` type when validating regular objects, and to `Vec<String>` when validating meta-schema payloads.

#### Scenario: Meta-schema errors mapped to strings
- **WHEN** meta-schema validation fails during Schema create or update
- **THEN** `SchemaValidationError.message` values are collected into `Vec<String>` and returned as `AppError::InvalidSchema`

#### Scenario: Object validation errors mapped to ValidationError
- **WHEN** object validation fails during object create or update
- **THEN** `SchemaValidationError` values are mapped to `object::types::ValidationError { path, message }` and returned as `AppError::SchemaValidation`

### Requirement: Service publishes events after mutations only
The service SHALL publish events only after successful store operations. If the store returns an error, no event is published.

#### Scenario: Failed create does not publish
- **WHEN** `create` fails due to a duplicate (AlreadyExists) or validation error
- **THEN** no `Added` event is published

#### Scenario: Failed update does not publish
- **WHEN** `update` fails due to a version conflict
- **THEN** no `Modified` event is published

