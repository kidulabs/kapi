# kapi — Architecture

## Overview

kapi is a Kubernetes-apiserver-inspired API server in Rust. Users register JSON Schemas for custom object kinds, then CRUD objects validated against those schemas, with real-time change notification via SSE watch semantics and pluggable storage.

This is **not** a Kubernetes compatibility layer — it borrows the API model (group/version/kind, resourceVersion, watch) but is a standalone system.

## System Architecture

```
Request → TraceLayer → CorsLayer → Handler → ObjectService → Store
                                                       │
                                           ┌───────────┴──────────────────────┐
                                           │  publish/subscribe via traits    │
                                           ▼                                  ▼
                                    EventPublisher               SchemaValidator
                                    (trait — Arc<dyn>)           (trait — Arc<dyn>)
                                           │                                  │
                                           ▼                                  ▼
                                     EventBus                      JsonSchemaValidator
                                 (broadcast channels)              (wraps jsonschema crate)

                       ┌──────────────────┐
                       │    AppState      │
                       │                  │
                       │  ObjectService   │  (wraps store + event publisher + validators)
                       │  (Arc<>)         │
                       └──────────────────┘
```

## Layers

### 1. Tower Middleware (routes.rs)

The outermost layer is a composable middleware chain:

- **TraceLayer** (tower-http) — logs every HTTP request for observability
- **CorsLayer** (tower-http) — permissive CORS for development
- **AuthLayer** (stub, T43) — placeholder for authentication/authorization
- **MetricsLayer** (stub, T44) — placeholder for request metrics collection

These layers compose via `ServiceBuilder` and wrap the entire router.

### 2. Handlers (object/handler.rs)

Thin Axum extractors and response builders. Handlers:
- Extract path parameters (group, version, kind, name)
- Deserialize request bodies
- Call `ObjectService` methods
- Convert results into HTTP responses

**No business logic** lives in handlers — they are pure translation layers.

### 3. ObjectService (object/service.rs)

The central orchestrator that coordinates validation, storage, and event publishing. All CRUD operations flow through `ObjectService`:

- **Schema objects** (kind == "Schema"): validate against meta-schema, compile nested jsonSchema, cache compiled validator
- **Regular objects**: look up Schema from store, validate against cached compiled schema
- **All mutations**: publish WatchEvent to EventBus after successful store operation

`ObjectService` holds:
- `store: Arc<dyn ObjectStore>` — pluggable storage
- `event_bus: Arc<dyn EventPublisher>` — pluggable event distribution
- `meta_validator: Arc<dyn SchemaValidator>` — compiled meta-schema
- `schema_cache: DashMap<String, Arc<dyn SchemaValidator>>` — compiled user schemas

### 4. Store (store/mod.rs)

Pluggable via the `ObjectStore` async trait. The v1 implementation is `InMemoryStore` using `DashMap`. Schema objects are stored in the same store as regular objects (kind `"Schema"` in group `kapi.io`).

### 5. Event Bus (event/bus.rs)

Pluggable via the `EventPublisher` trait. The production implementation is `EventBus` using per-kind `tokio::broadcast` channels. Channels are auto-created on first `subscribe` and lazily cleaned up on `publish` when all receivers are dropped.

## Module Tree

```
src/
├── main.rs                 # Tokio runtime, config construction, kapi::run()
├── lib.rs                  # Module tree, re-exports, create_app(), run()
├── config/mod.rs           # AppConfig struct (port, store, event_bus)
├── error.rs                # AppError enum + IntoResponse impl
├── routes.rs               # Router composition (all route definitions)
├── store/
│   ├── mod.rs              # ObjectStore trait, ResourceKey
│   └── memory.rs           # InMemoryStore (DashMap, AtomicU64) + tests
├── schema/
│   └── meta_schema.rs      # Meta-schema constant + SchemaValidator trait
│                           # + JsonSchemaValidator wrapper + tests
├── object/
│   ├── types.rs            # Core types (StoredObject, ObjectMetadata, etc.)
│   ├── service.rs          # ObjectService orchestrator + tests
│   └── handler.rs          # Axum route handlers
├── event/
│   ├── bus.rs              # EventBus + EventPublisher trait + WatchStream + tests
├── middleware/
│   ├── auth.rs             # AuthLayer stub (TODO)
│   └── metrics.rs          # MetricsLayer stub (TODO)
└── openapi/
    ├── mod.rs              # Module root + GET /openapi handler + tests
    ├── components.rs       # Static + dynamic OpenAPI component builders
    ├── paths.rs            # Static + dynamic path builders + spec orchestrator
    └── swagger.rs          # Swagger UI HTML constant and handler
```

## Request Flow

### Create Object

```
POST /apis/example.io/v1/Widget
  │
  ▼ Handler: extract group/version/kind/body, strip metadata
  │
  ▼ ObjectService::create(key, name, data)
  │   ├── Schema path: validate meta-schema → compile jsonSchema → cache
  │   ├── Object path:  look up Schema → validate against compiled schema
  │   ├── store.create(key, name, data) → StoredObject
  │   └── event_bus.publish(key, WatchEvent::Added(obj))
  │
  ▼ Response: 201 Created + StoredObject JSON
```

### Update Object

```
PUT /apis/example.io/v1/Widget/my-widget
  │  Body: StoredObject (with metadata.resourceVersion for OCC)
  │
  ▼ Handler: validate URL key/name match body
  │
  ▼ ObjectService::update(stored_object)
  │   ├── Validate data payload against schema
  │   ├── store.update(object) — OCC check on resourceVersion
  │   └── event_bus.publish(key, WatchEvent::Modified(obj))
  │
  ▼ Response: 200 OK + StoredObject (new resourceVersion)
```

### Watch Events

```
GET /apis/example.io/v1/Widget?watch=true
  │
  ▼ Handler: detect ?watch=true
  │
  ▼ ObjectService::subscribe(key) → WatchStream
  │   (delegates to EventBus::subscribe)
  │
  ▼ Response: SSE stream of WatchEvent (Added/Modified/Deleted)
```

### Schema Registration

```
POST /apis/kapi.io/v1/Schema
  │  Body: { targetGroup, targetVersion, targetKind, jsonSchema }
  │
  ▼ Handler: generate name as "{targetKind}.{targetGroup}"
  │
  ▼ ObjectService::create (Schema path)
  │   ├── Validate against meta-schema
  │   ├── Compile jsonSchema via jsonschema crate
  │   ├── Cache compiled validator
  │   ├── store.create() → StoredObject
  │   └── publish WatchEvent::Added
  │
  ▼ Response: 201 Created + StoredObject (with generated name)
```

### Schema Deletion

```
DELETE /apis/kapi.io/v1/Schema/{name}
  │
  ▼ ObjectService::delete (Schema path)
  │   ├── Fetch schema, parse target kind
  │   ├── List target kind objects (limit=1)
  │   ├── If objects exist → 409 SchemaHasObjects
  │   ├── Delete schema
  │   ├── Evict compiled validator from cache
  │   └── publish WatchEvent::Deleted
  │
  ▼ Response: 200 OK or 409 Conflict
```

## Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Framework | Axum | Tower composability for middleware, SSE support, nested routers |
| Storage abstraction | Single ObjectStore trait | Schema is also an object; one store simplifies backends |
| Event publishing | Service layer publishes, store is pure data | Impossible to "forget to publish" — handlers call service only |
| v1 storage | In-memory (DashMap) | Zero ops overhead, perfect for dev; trait makes swapping trivial |
| API paths | Kube-style `/apis/{group}/{version}/{kind}` | Familiar to kube users, supports multiple API groups |
| Watch semantics | `?watch=true` on list endpoint | Kube-native pattern, handler branches on query param |
| Event bus | Per-kind `tokio::broadcast` channels | Each kind gets its own channel; swappable for testing |
| Concurrency | Global monotonic `AtomicU64` | Enables "give me events since version N" for watch resume |
| Schema validation | `Arc<dyn SchemaValidator>` | Isolates jsonschema crate behind trait; swappable |
| Schema deletion | Block if objects exist (409) | Prevent accidental data loss |
| Schema compilation | At registration time | Reject invalid schemas at registration with 422 |
| Optimistic concurrency | Embedded resourceVersion on StoredObject | Version travels with the object; cleaner trait signature |
| Update contract | `update(StoredObject)` not decomposed params | What comes out goes back in; symmetric contract |
| Delete | Unconditional (no version check) | Deletes are idempotent; simplifies contract |
| Wire format | camelCase metadata fields | Standard for JSON APIs; matches K8s conventions |
| Validation scope | Schema validates data only | Metadata is server-managed; users define only their domain |
| Binary construction | `AppConfig + create_app() + run()` in lib | main.rs is ~15 lines; all wiring in lib for testability |
