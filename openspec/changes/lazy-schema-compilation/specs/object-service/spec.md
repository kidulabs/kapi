## MODIFIED Requirements

### Requirement: ObjectService wraps store, event bus, and validators
The system SHALL define an `ObjectService` struct containing:
- `store: Arc<dyn ObjectStore>` — the storage backend
- `event_bus: Arc<dyn EventPublisher>` — the per-kind event bus for watch notifications
- `meta_validator: Arc<dyn SchemaValidator>` — compiled meta-schema for Schema validation
- `schema_cache: DashMap<String, Arc<dyn SchemaValidator>>` — compiled user schemas keyed by schema name (e.g., `"Widget.example.io"`)

#### Scenario: Service construction with schema warmup
- **WHEN** `ObjectService::new(store, event_bus, meta_validator)` is called with `store: Arc<dyn ObjectStore>`, `event_bus: Arc<dyn EventPublisher>`, and `meta_validator: Arc<dyn SchemaValidator>`
- **THEN** the service is constructed, all existing Schema objects are loaded from the store and compiled into the `schema_cache`, and the service is ready to accept requests

### Requirement: create validates and stores objects
The `create(key, name, data)` method SHALL:
1. If `key.kind == "Schema"`: validate `data` against `meta_validator`, compile `data.jsonSchema` via `validator_for()`, cache the compiled validator under the `name` parameter (which is generated as `{targetKind}.{targetGroup}` by the handler)
2. If `key.kind != "Schema"`: look up the Schema from the store, validate `data` against the cached compiled schema (with lazy compilation fallback if not in cache)
3. Call `store.create(key, name, data)`
4. Call `event_bus.publish(key, WatchEvent::Added(obj))`
5. Return the created `StoredObject`

#### Scenario: Create object with schema not in cache but in store
- **WHEN** creating an object for a kind whose Schema exists in the store but is not in the cache
- **THEN** the schema is compiled on-demand, cached, and the object is validated against it
