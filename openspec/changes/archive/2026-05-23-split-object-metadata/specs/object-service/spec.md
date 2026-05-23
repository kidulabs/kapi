## MODIFIED Requirements

### Requirement: ObjectService wraps store, event bus, and validators
The system SHALL define an `ObjectService` struct containing:
- `store: Arc<dyn ObjectStore>` — the storage backend
- `event_bus: Arc<dyn EventPublisher>` — the per-kind event bus for watch notifications
- `meta_validator: Arc<dyn SchemaValidator>` — compiled meta-schema for Schema validation
- `schema_cache: DashMap<String, Arc<dyn SchemaValidator>>` — compiled user schemas keyed by schema name

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