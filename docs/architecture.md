# kapi — Architecture

## Overview

kapi is a Kubernetes-apiserver-inspired API server in Rust. Users register JSON Schemas for custom object kinds, then CRUD objects validated against those schemas, with real-time change notification via SSE watch semantics and pluggable storage.

This is **not** a Kubernetes compatibility layer — it borrows the API model (group/version/kind, resourceVersion, watch) but is a standalone system.

## System Architecture

```
Request → TraceLayer → CorsLayer → Handler ──┬── SchemaService ──┐
                                              │                    │
                                              └── ObjectService ──┤
                                                                   ▼
                                                                 Store
                                                                   │
                                                        ┌──────────┴──────────────────────┐
                                                        │  publish/subscribe via traits    │
                                                        ▼                                  ▼
                                                 EventPublisher               SchemaRegistry
                                                 (trait — Arc<dyn>)           (validation + caching)
                                                        │                                  │
                                                        ▼                                  ▼
                                                   EventBus                      SchemaValidator
                                         (predicate routing — Vec<Watcher>       (trait — Arc<dyn>)
                                          with WatchFilter + mpsc::Sender)        │
                                                                                  ▼
                                                                          JsonSchemaValidator
                                                                        (wraps jsonschema crate)

                   ┌─────────────────────────────┐
                   │         AppState             │
                   │                              │
                   │  ObjectService               │  (regular object CRUD)
                   │  SchemaService               │  (Schema lifecycle management)
                   │  (both Arc<>)                │
                   └──────────────────────────────┘
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
- Extract and validate `metadata.labels` and `metadata.annotations` from request body
- Validate input format eagerly — reject invalid labels/annotations before any service I/O
- Deserialize request bodies (strip `metadata` before sending to store)
- Call `ObjectService` methods
- Convert results into HTTP responses

**Format validation** (label regex, annotation size limits) is done at the handler edge.
**Stateful validation** (schema lookup, JSON Schema validation, OCC checks, deletion guards)
remains in the service layer, which also re-validates format as defense-in-depth.

### 3. ObjectService (object/service.rs)

The orchestrator for regular (non-Schema) object CRUD. Schema operations are handled by `SchemaService` (see §4). All regular object mutations flow through `ObjectService`:

- **Regular objects**: look up Schema from store, validate against cached compiled schema
- **All mutations**: validate labels and annotations (`ObjectMeta.labels` and `ObjectMeta.annotations`) before persisting (defense-in-depth — handler already validates at edge)
- **All mutations**: publish WatchEvent to EventBus after successful store operation
- Shared helpers (metadata computation, status updates) live in `object/helpers.rs`
- Finalizer state machine logic lives in `object/finalizer.rs`

`ObjectService` holds:
- `store: Arc<dyn ObjectStore>` — pluggable storage
- `event_bus: Arc<dyn EventPublisher>` — pluggable event distribution
- `schema_registry: SchemaRegistry` — manages schema validation, compilation, and caching

### 4. SchemaService (object/schema_service.rs)

A dedicated service for Schema lifecycle management, extracted from the former monolithic `ObjectService`. The handler layer dispatches to `SchemaService` when `kind == "Schema"`, and to `ObjectService` for all other kinds.

`SchemaService` operations:

- **Create**: validates the registration payload against the meta-schema, compiles the user's `jsonSchema` into a cached validator, persists the Schema object, and publishes a `WatchEvent::Added`.
- **Update**: re-compiles the schema, replaces the cached validator, persists the update, and publishes a `WatchEvent::Modified`.
- **Delete**: fetches the Schema, checks for existing objects of the target kind (returns `409 SchemaHasObjects` if any exist), evicts the cached validator, deletes the Schema, and publishes a `WatchEvent::Deleted`.

`SchemaService` holds:
- `store: Arc<dyn ObjectStore>` — pluggable storage (same store as ObjectService)
- `event_bus: Arc<dyn EventPublisher>` — pluggable event distribution
- `schema_registry: SchemaRegistry` — manages schema validation, compilation, and caching

### 5. Store (store/mod.rs)

Pluggable via the `ObjectStore` async trait. Two implementations are available: `InMemoryStore` using `DashMap` for ephemeral storage, and `SQLiteStore` using `rusqlite` for persistent storage. Schema objects are stored in the same store as regular objects (kind `"Schema"` in group `kapi.io`).

### 6. Event Bus (event/bus.rs)

Pluggable via the `EventPublisher` trait. The production implementation is `EventBus` using predicate routing with per-kind `Vec<Watcher>`. Each `Watcher` holds a `WatchFilter` and an `mpsc::Sender<WatchEvent>`. On `publish`, events are delivered only to watchers whose filter matches the event. Dead watchers (disconnected clients) are lazily removed via `retain()` on the next publish.

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
│   ├── memory.rs           # InMemoryStore (DashMap, AtomicU64) + tests
│   └── sqlite.rs           # SQLiteStore (rusqlite, spawn_blocking) + tests
├── schema/
│   ├── meta_schema.rs      # Meta-schema constant + SchemaValidator trait
│   │                       # + JsonSchemaValidator wrapper + tests
│   └── registry.rs         # SchemaRegistry — validation, compilation, caching
├── object/
│   ├── types.rs            # Core types (StoredObject, ObjectMeta, SystemMetadata, etc.)
│   ├── helpers.rs          # Shared helpers (apply_with_metadata, status updates)
│   ├── finalizer.rs        # Finalizer state machine logic
│   ├── service.rs          # ObjectService orchestrator (regular objects only) + tests
│   ├── schema_service.rs   # SchemaService — Schema lifecycle management + tests
│   └── handler.rs          # Axum route handlers (format validation at edge)
├── validation/
│   └── mod.rs              # Stateless format validation (labels, annotations) + tests
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
  ▼ Handler: extract group/version/kind/body, extract ObjectMeta + labels
  │   ├── validate_labels(meta.labels) → 400 if invalid (fail-fast)
  │   └── validate_annotations(meta.annotations) → 400 if invalid (fail-fast)
  │
  ▼ Handler dispatches by kind
  │   ├── kind == "Schema" → SchemaService::create(key, meta, spec)
  │   └── else            → ObjectService::create(key, meta, spec)
  │
  ▼ ObjectService::create(key, meta, spec)  [regular objects only]
  │   ├── validate_labels(meta.labels) → 400 if invalid (defense-in-depth)
  │   ├── validate_annotations(meta.annotations) → 400 if invalid (defense-in-depth)
  │   ├── SchemaRegistry.get_validator() → validate against compiled schema
  │   ├── store.create(key, meta, spec) → StoredObject
  │   └── event_bus.publish(key, WatchEvent::Added(obj))
  │
  ▼ SchemaService::create(key, meta, spec)  [Schema objects only]
  │   ├── SchemaRegistry.validate_and_compile() → meta-schema validate + compile
  │   ├── SchemaRegistry.insert() → cache compiled validator
  │   ├── store.create() → StoredObject
  │   └── event_bus.publish(key, WatchEvent::Added(obj))
  │
  ▼ Response: 201 Created + StoredObject JSON
```

### Update Object

```
PUT /apis/example.io/v1/Widget/my-widget
  │  Body: StoredObject (with system.resourceVersion for OCC)
  │
  ▼ Handler: validate URL key/name match body
  │   ├── validate_labels(meta.labels) → 400 if invalid (fail-fast)
  │   └── validate_annotations(meta.annotations) → 400 if invalid (fail-fast)
  │
  ▼ Handler dispatches by kind
  │   ├── kind == "Schema" → SchemaService::update(stored_object)
  │   └── else            → ObjectService::update(stored_object)
  │
  ▼ ObjectService::update(stored_object)  [regular objects only]
  │   ├── validate_labels(stored_object.metadata.labels) → 400 if invalid (defense-in-depth)
  │   ├── validate_annotations(stored_object.metadata.annotations) → 400 if invalid (defense-in-depth)
  │   ├── Validate spec payload against schema
  │   ├── store.update(object) — OCC check on system.resourceVersion
  │   │   └── diff-based label update (read existing → compute delta → apply)
  │   └── event_bus.publish(key, WatchEvent::Modified(obj))
  │
  ▼ SchemaService::update(stored_object)  [Schema objects only]
  │   ├── SchemaRegistry.validate_and_compile() → re-compile schema
  │   ├── SchemaRegistry.insert() → replace cached validator
  │   ├── store.update(object) → persist
  │   └── event_bus.publish(key, WatchEvent::Modified(obj))
  │
  ▼ Response: 200 OK + StoredObject (new system.resourceVersion)
```

### Watch Events

```
GET /apis/example.io/v1/Widget?watch=true&fieldSelector=metadata.name=my-widget
GET /apis/example.io/v1/Widget?watch=true&labelSelector=app=nginx,env=prod
  │
  ▼ Handler: detect ?watch=true, parse fieldSelector and/or labelSelector into WatchFilter
  │   (400 if selector on non-watch request, unsupported field, or malformed syntax)
  │   (When both fieldSelector and labelSelector are present, labelSelector takes precedence)
  │
  ▼ Handler dispatches by kind (subscribes via SchemaService or ObjectService)
  │   Both services delegate to EventBus::subscribe with WatchFilter
  │   (EventBus creates mpsc::channel + Watcher, filters on publish)
  │
  ▼ Response: SSE stream of WatchEvent (Added/Modified/Deleted)
  │   only events matching the WatchFilter are delivered
```

### Schema Registration

```
POST /apis/kapi.io/v1/Schema
  │  Body: { targetGroup, targetVersion, targetKind, jsonSchema }
  │
  ▼ Handler: generate name as "{targetKind}.{targetGroup}", dispatch to SchemaService
  │
  ▼ SchemaService::create(key, meta, spec)
  │   ├── SchemaRegistry.validate_and_compile() — meta-schema validate + compile
  │   ├── SchemaRegistry.insert() — cache compiled validator
  │   ├── store.create() → StoredObject
  │   └── publish WatchEvent::Added
  │
  ▼ Response: 201 Created + StoredObject (with generated name)
```

### Schema Deletion

```
DELETE /apis/kapi.io/v1/Schema/{name}
  │
  ▼ Handler: dispatch to SchemaService
  │
  ▼ SchemaService::delete(key, name)
  │   ├── Fetch schema, parse target kind
  │   ├── List target kind objects (limit=1)
  │   ├── If objects exist → 409 SchemaHasObjects
  │   ├── Delete schema
  │   ├── SchemaRegistry.evict() — remove from cache
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
| v1 storage | In-memory (DashMap) + SQLite (rusqlite) | Zero ops for dev, persistent option for production; trait makes swapping trivial |
| API paths | Kube-style `/apis/{group}/{version}/{kind}` | Familiar to kube users, supports multiple API groups |
| Watch semantics | `?watch=true` on list endpoint | Kube-native pattern, handler branches on query param |
| Event bus | Predicate routing — `Vec<Watcher>` with `WatchFilter` + `mpsc::Sender` per watcher | Eliminates unnecessary work — filtered watchers only receive matching events; swappable for testing; `WatchFilter` supports `All`, `FieldSelector`, and `LabelSelector` variants |
| Concurrency | Global monotonic `AtomicU64` | Enables "give me events since version N" for watch resume |
| Schema validation | `SchemaRegistry` with `Arc<dyn SchemaValidator>` | Isolates jsonschema crate behind trait; manages compilation+ caching; swappable |
| Schema deletion | Block if objects exist (409) | Prevent accidental data loss |
| Schema compilation | At registration time | Reject invalid schemas at registration with 422 |
| Optimistic concurrency | Embedded system.resourceVersion on StoredObject | Version travels with the object; cleaner trait signature |
| Update contract | `update(StoredObject)` not decomposed params | What comes out goes back in; symmetric contract |
| Delete | Unconditional (no version check) | Deletes are idempotent; simplifies contract |
| Wire format | `metadata` (user) + `system` (server) fields | Clear ownership boundary; camelCase for JSON APIs |
| Validation scope | Schema validates data only | Metadata is server-managed; users define only their domain |
| Binary construction | `AppConfig + create_app() + run()` in lib | main.rs is ~15 lines; all wiring in lib for testability |
