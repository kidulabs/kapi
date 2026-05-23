## MODIFIED Requirements

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
The `create(key, name, data)` method SHALL:
1. If `key.kind == "Schema"`: validate `data` against `meta_validator`, compile `data.jsonSchema` via `compile_jsonschema()`, cache the compiled validator under the `name` parameter (which is generated as `{targetKind}.{targetGroup}` by the handler)
2. If `key.kind != "Schema"`: look up the Schema from the store via `lookup_object_validator()`, which compiles on cache miss if the schema exists in the store
3. Call `store.create(key, name, data)`
4. Call `event_bus.publish(key, WatchEvent::Added(obj))`
5. Return the created `StoredObject`

#### Scenario: Create object with schema not in cache but in store
- **WHEN** creating an object for a kind whose Schema exists in the store but is not in the cache
- **THEN** `lookup_object_validator()` compiles the schema on-demand, caches it, and the object is validated against it

#### Scenario: Create object with stored schema that fails compilation
- **WHEN** creating an object for a kind whose Schema exists in the store but whose `jsonSchema` fails compilation
- **THEN** `lookup_object_validator()` returns `AppError::StoredSchemaCompilationFailed`
- **AND** no object is created

### Requirement: update validates and stores objects
The `update(object)` method SHALL:
1. Determine if the object is a Schema or regular object based on `object.key.kind`
2. Apply the same validation flow as `create` (meta-schema for Schema, compiled schema for objects with lazy compilation fallback)
3. Call `store.update(object)`
4. Call `event_bus.publish(key, WatchEvent::Modified(obj))`
5. Return the updated `StoredObject`

#### Scenario: Update object with schema not in cache but in store
- **WHEN** updating an object for a kind whose Schema exists in the store but is not in the cache
- **THEN** `lookup_object_validator()` compiles the schema on-demand, caches it, and the object is validated against it
