## Meta-Schema (P4)

### Draft Version: 2020-12

The meta-schema uses Draft 2020-12. Rationale:
- Best compliance in the `jsonschema` 0.46 crate (bowtie.report scores)
- OpenAPI 3.1 aligns with Draft 2020-12
- Supports `unevaluatedProperties: false` to reject unknown fields in Schema registrations
- User schemas auto-detect via `validator_for()` — users can register Draft 4 through 2020-12 schemas regardless

### Meta-Schema JSON

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "required": ["targetGroup", "targetVersion", "targetKind", "jsonSchema"],
  "properties": {
    "targetGroup":  { "type": "string", "minLength": 1 },
    "targetVersion": { "type": "string", "minLength": 1 },
    "targetKind":   { "type": "string", "minLength": 1 },
    "jsonSchema":   { "type": "object" }
  },
  "unevaluatedProperties": false
}
```

The meta-schema validates the **registration envelope** only — that the four fields exist and have the right shape. It does NOT validate the contents of `jsonSchema`. That is left to `jsonschema::compile()`.

### Compilation

```
compile_meta_schema() -> Result<jsonschema::Validator, anyhow::Error>
```

Called once at server startup. The resulting `Validator` is injected into `ObjectService`.

---

## ObjectService (P5)

### Architecture

```
┌──────────────────────────────────────────────────────────────────┐
│                        ObjectService                             │
├──────────────────────────────────────────────────────────────────┤
│                                                                  │
│  store: Arc<dyn ObjectStore>                                     │
│  event_bus: EventBus                                             │
│  meta_validator: jsonschema::Validator    ← compiled at startup  │
│  schema_cache: DashMap<ResourceKey, Arc<jsonschema::Validator>>  │
│                                                                  │
│  create(key, name, data) -> Result<StoredObject, AppError>       │
│  get(key, name) -> Result<StoredObject, AppError>                │
│  list(key, opts) -> Result<ListResponse, AppError>               │
│  update(object) -> Result<StoredObject, AppError>                │
│  delete(key, name) -> Result<StoredObject, AppError>             │
│                                                                  │
└──────────────────────────────────────────────────────────────────┘
```

### Validation Dispatch

```
create(key, name, data):
    if key.kind == "Schema":
        1. meta_validator.validate(data)
           → InvalidSchema on failure
        2. compile(data.jsonSchema) via validator_for()
           → InvalidSchema on compilation failure
        3. cache compiled schema: schema_cache.insert(key, Arc<Validator>)
    else:
        1. look up Schema from store (key = {kapi.io, v1, Schema}, name = "{kind}.{group}")
           → NotFound if no schema registered for this kind
        2. validate data against cached compiled schema
           → SchemaValidation on failure
    4. store.create(key, name, data)
    5. event_bus.publish(key, WatchEvent::Added(obj))

update(object):
    same validation flow as create (Schema vs regular object)
    store.update(object)
    event_bus.publish(key, WatchEvent::Modified(obj))

delete(key, name):
    if key.kind == "Schema":
        1. parse target kind from schema data (targetGroup, targetVersion, targetKind)
        2. list(target_key) → if non-empty, return Conflict with object_count
    store.delete(key, name)
    schema_cache.remove(key)  ← evict compiled schema from cache
    event_bus.publish(key, WatchEvent::Deleted(obj))
```

### Schema Cache (Option B — simple, extendable)

```
schema_cache: DashMap<ResourceKey, Arc<jsonschema::Validator>>
```

- **Key**: The Schema's `ResourceKey` (`{kapi.io, v1, Schema}`) + name combination
- **Value**: `Arc<jsonschema::Validator>` — thread-safe, cheap to clone
- **Population**: On Schema create/update, compile and insert
- **Invalidation**: On Schema delete, remove from cache
- **Lookup**: For regular object validation, look up by the target kind's schema name

The cache key for lookup is not the Schema's own ResourceKey, but a derived key from the schema's `targetGroup`/`targetVersion`/`targetKind`. The simplest approach: use the schema's `name` field (e.g., `"Widget.example.io"`) as the lookup key.

### SchemaData Struct

Schema objects store their data as a `UserData` wrapping raw JSON. For type safety, define a struct:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaData {
    pub target_group: String,
    pub target_version: String,
    pub target_kind: String,
    pub json_schema: serde_json::Value,
}
```

This is used when reading Schema objects from the store to extract the target kind for cache lookups and deletion guards.

---

## Error Handling

### New Variant: `InvalidSchema`

```rust
pub enum AppError {
    NotFound { what: String, identifier: String },
    Conflict { expected: u64, actual: u64 },
    SchemaValidation(Vec<ValidationError>),  // object doesn't match its schema
    InvalidSchema(String),                    // the schema itself is broken
    Internal(anyhow::Error),
}
```

**HTTP mapping**: 422 Unprocessable Entity
**JSON body**: `{ "error": "...", "code": "InvalidSchema", "details": { "message": "..." } }`

**When produced**:
- Meta-schema validation fails on Schema registration
- `jsonschema::compile()` fails on the nested `jsonSchema`

---

## Handlers (P5)

### Route Structure

```
/apis/{group}/{version}/{kind}          → GET (list/watch), POST (create)
/apis/{group}/{version}/{kind}/{name}   → GET (get), PUT (update), DELETE (delete)
```

### Handler Responsibilities

Handlers are thin — extract path params, deserialize body, call service, return response. No business logic.

```
POST /apis/{group}/{version}/{kind}:
    extract group, version, kind from path
    deserialize body as serde_json::Value
    service.create(key, name, data)
    → 201 Created + StoredObject

GET /apis/{group}/{version}/{kind}:
    if ?watch=true:
        subscribe to event_bus, return Sse<impl Stream>
    else:
        deserialize query params (limit, continue)
        service.list(key, opts)
        → 200 OK + ListResponse

GET /apis/{group}/{version}/{kind}/{name}:
    service.get(key, name)
    → 200 OK + StoredObject

PUT /apis/{group}/{version}/{kind}/{name}:
    deserialize body as StoredObject
    validate URL key/name matches object's key/name
    service.update(object)
    → 200 OK + StoredObject (with new resourceVersion)

DELETE /apis/{group}/{version}/{kind}/{name}:
    service.delete(key, name)
    → 200 OK + StoredObject (the deleted object)
```

### Watch Detection

The list handler branches on `?watch=true`:

```
GET /apis/example.io/v1/Widget?watch=true
    → Sse stream of WatchEvent
GET /apis/example.io/v1/Widget
    → JSON ListResponse
```

The SSE stream maps `WatchEvent` to `axum::response::sse::Event`. Stream termination (lag) closes the SSE connection — client must reconnect.

---

## Application Wiring

### AppState

```rust
pub struct AppState {
    pub object_service: ObjectService<InMemoryStore>,
}
```

`ObjectService` holds the store and event bus internally. Handlers extract `State<AppState>` and call `state.object_service.*`.

### Startup Flow

```
main():
    1. tracing_subscriber::fmt::init()
    2. compile_meta_schema() → Validator
    3. InMemoryStore::new()
    4. EventBus::default()
    5. ObjectService::new(store, event_bus, meta_validator)
    6. build_router()
    7. port = env("PORT").unwrap_or(8080)
    8. axum::serve(listener, app)
```

### Router Composition

```
Router::new()
    .route("/apis/{group}/{version}/{kind}", get(list).post(create))
    .route("/apis/{group}/{version}/{kind}/{name}", get(get).put(update).delete(delete))
    .layer(ServiceBuilder::new()
        .layer(TraceLayer::new_for_http())
        .layer(AuthLayer)
        .layer(MetricsLayer))
    .with_state(AppState { object_service })
```

---

## Schema Deletion Guard

When deleting a Schema object, the service checks if any objects of the target kind exist:

```
delete(key = {kapi.io, v1, Schema}, name = "Widget.example.io"):
    1. get schema from store
    2. parse target_group, target_version, target_kind from schema data
    3. target_key = ResourceKey { target_group, target_version, target_kind }
    4. list(target_key, ListOptions { limit: Some(1), .. })
    5. if items non-empty → Conflict { expected: 0, actual: object_count }
    6. store.delete(key, name)
    7. schema_cache.remove(name)
    8. event_bus.publish(key, WatchEvent::Deleted(obj))
```

The `Conflict` response should include the count of existing objects in the details. This requires a minor extension to the `Conflict` variant or a new error variant. For now, reuse `Conflict` with `actual` = object count and add the count to the HTTP response details.

**Decision**: Reuse `Conflict` but the handler can enrich the response. The service returns `Conflict { expected: 0, actual: count }` and the handler adds context. Alternatively, add a `SchemaHasObjects { kind: String, count: usize }` variant. The latter is cleaner — let's add it.

```rust
pub enum AppError {
    NotFound { what: String, identifier: String },
    Conflict { expected: u64, actual: u64 },
    SchemaValidation(Vec<ValidationError>),
    InvalidSchema(String),
    SchemaHasObjects { kind: String, count: usize },  // new
    Internal(anyhow::Error),
}
```

**HTTP mapping**: 409 Conflict
**JSON body**: `{ "error": "...", "code": "SchemaHasObjects", "details": { "kind": "Widget", "count": 5 } }`
