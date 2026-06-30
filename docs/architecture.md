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
- Extract path parameters (group, version, kind, name, optional namespace)
- Extract and validate `metadata.labels` and `metadata.annotations` from request body
- Validate input format eagerly — reject invalid labels/annotations before any service I/O
- Deserialize request bodies (strip `metadata` before sending to store)
- Dispatch to `SchemaService` when `kind == "Schema"`, to `ObjectService` for all other kinds
- Call service methods with extracted namespace (or `None` for cluster-scoped routes)
- Convert results into HTTP responses

**Format validation** (label regex, annotation size limits) is done at the handler edge.
**Stateful validation** (schema lookup, JSON Schema validation, OCC checks, deletion guards,
scope validation) remains in the service layer, which also re-validates format as defense-in-depth.

**URL patterns:**
- Cluster-scoped: `/apis/{group}/{version}/{kind}` and `/apis/{group}/{version}/{kind}/{name}`
- Namespace-scoped: `/apis/{group}/{version}/namespaces/{ns}/{kind}` and `/apis/{group}/{version}/namespaces/{ns}/{kind}/{name}`
- Separate handler functions exist for each pattern: `list`/`create` (cluster) and `list_namespaced`/`create_namespaced` (namespace-scoped).

### 3. ObjectService (object/service.rs)

The orchestrator for regular (non-Schema) object CRUD. Schema operations are handled by `SchemaService` (see §4). All regular object mutations flow through `ObjectService`:

- **Regular objects**: look up Schema from store, validate against cached compiled schema
- **All mutations**: validate labels and annotations (`ObjectMeta.labels` and `ObjectMeta.annotations`) before persisting (defense-in-depth — handler already validates at edge)
- **All mutations**: publish WatchEvent to EventBus after successful store operation
- **Scope validation**: For each request, the service looks up the schema's scope (`Namespaced` or `Cluster`) from the `SchemaRegistry`:
  - Cluster-scoped kinds reject namespace in URL → `400 InvalidRequest`
  - Namespaced kinds on cluster-scoped URLs default namespace to `"default"`
  - Namespaced kinds on namespace-scoped URLs use the URL namespace
- **Namespace consistency**: On updates, `metadata.namespace` in the body must match the URL namespace (if provided), or must match the stored object's namespace.
- Shared helpers (metadata computation, status updates) live in `object/helpers.rs`
- Finalizer state machine logic lives in `object/finalizer.rs`

`ObjectService` holds:
- `store: Arc<dyn ObjectStore>` — pluggable storage
- `event_bus: Arc<dyn EventPublisher>` — pluggable event distribution
- `schema_registry: SchemaRegistry` — manages schema validation, compilation, and caching

### 4. SchemaService (object/schema_service.rs)

A dedicated service for Schema lifecycle management, extracted from the former monolithic `ObjectService`. The handler layer dispatches to `SchemaService` when `kind == "Schema"`, and to `ObjectService` for all other kinds.

Schema is **always cluster-scoped** — URL patterns use `/apis/kapi.io/v1/Schema` without a namespace segment. All store operations pass `namespace: None`. The Schema's `metadata.namespace` is always `null`.

`SchemaService` operations:

- **Create**: validates the registration payload against the meta-schema, extracts the target kind's scope (`Namespaced` or `Cluster`), compiles the user's `jsonSchema` into a cached validator (alongside the scope), persists the Schema object with `namespace: None`, and publishes a `WatchEvent::Added`.
- **Update**: re-compiles the schema, replaces the cached validator and scope, persists the update, and publishes a `WatchEvent::Modified`.
- **Delete**: fetches the Schema, checks for existing objects of the target kind (returns `409 SchemaHasObjects` if any exist), evicts the cached validator, deletes the Schema, and publishes a `WatchEvent::Deleted`.

`SchemaService` holds:
- `store: Arc<dyn ObjectStore>` — pluggable storage (same store as ObjectService)
- `event_bus: Arc<dyn EventPublisher>` — pluggable event distribution
- `schema_registry: SchemaRegistry` — manages schema validation, compilation, and caching

### 5. Store (store/mod.rs)

Pluggable via the `ObjectStore` async trait. Two implementations are available: `InMemoryStore` using `DashMap` for ephemeral storage, and `SQLiteStore` using `rusqlite` for persistent storage. Schema objects are stored in the same store as regular objects (kind `"Schema"` in group `kapi.io`).

All store methods accept `namespace: Option<&str>` for namespace-aware operations:
- **`get`**: retrieves an object by key + namespace + name
- **`list`**: when `namespace` is `None`, returns objects from all namespaces (cross-namespace list); when `Some`, returns only objects in that namespace
- **`transaction`**: mutates an object within a specific namespace
- **`create`**: persists the object with its `metadata.namespace` value
- **`exists`**: checks for any objects of a given key, across all namespaces

### 6. Event Bus (event/bus.rs)

Pluggable via the `EventPublisher` trait. The production implementation is `EventBus` using predicate routing with per-kind `Vec<Watcher>`. Each `Watcher` holds a `WatchFilter` and an `mpsc::Sender<WatchEvent>`. On `publish`, events are delivered only to watchers whose filter matches the event. Dead watchers (disconnected clients) are lazily removed via `retain()` on the next publish.

Events carry `StoredObject` which includes `metadata.namespace`. This enables natural namespace-based filtering:
- Namespace-scoped watch streams (via `/namespaces/{ns}/{kind}?watch=true`) receive events whose object's namespace matches the watched namespace
- Cross-namespace watch streams (via `/{kind}?watch=true`) receive events from all namespaces

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
POST /apis/example.io/v1/Widget                          (cluster-scoped kind)
POST /apis/example.io/v1/namespaces/staging/NamespacedWidget  (namespace-scoped kind)
  │
  ▼ Handler: extract group/version/kind/body, extract namespace from path (or None)
  │   ├── validate_labels(meta.labels) → 400 if invalid (fail-fast)
  │   └── validate_annotations(meta.annotations) → 400 if invalid (fail-fast)
  │
  ▼ Handler dispatches by kind
  │   ├── kind == "Schema" → SchemaService::create(key, meta, spec)
  │   └── else            → ObjectService::create(key, namespace, meta, spec)
  │
  ▼ ObjectService::create(key, namespace, meta, spec)  [regular objects only]
  │   ├── Look up schema scope from SchemaRegistry
  │   ├── Validate scope vs namespace:
  │   │   ├── Cluster-scoped kind + namespace Some → 400 InvalidRequest
  │   │   ├── Namespaced kind + namespace None → default to "default"
  │   │   └── Namespaced kind + namespace Some → use URL namespace
  │   ├── validate_labels(meta.labels) → 400 if invalid (defense-in-depth)
  │   ├── validate_annotations(meta.annotations) → 400 if invalid (defense-in-depth)
  │   ├── SchemaRegistry.get_validator() → validate against compiled schema
  │   ├── store.create(key, meta, spec) → StoredObject
  │   └── event_bus.publish(key, WatchEvent::Added(obj))
  │
  ▼ SchemaService::create(key, meta, spec)  [Schema objects only — always cluster-scoped]
  │   ├── SchemaRegistry.validate_and_compile() → meta-schema validate + compile
  │   ├── SchemaRegistry.insert() → cache compiled validator + scope
  │   ├── store.create(namespace: None) → StoredObject with null namespace
  │   └── event_bus.publish(key, WatchEvent::Added(obj))
  │
  ▼ Response: 201 Created + StoredObject JSON
```

### Update Object

```
PUT /apis/example.io/v1/Widget/my-widget                              (cluster-scoped)
PUT /apis/example.io/v1/namespaces/staging/NamespacedWidget/widget-alpha  (namespace-scoped)
  │  Body: StoredObject (with system.resourceVersion for OCC)
  │
  ▼ Handler: validate URL key/name match body, extract namespace from URL
  │   ├── validate_labels(meta.labels) → 400 if invalid (fail-fast)
  │   ├── validate_annotations(meta.annotations) → 400 if invalid (fail-fast)
  │   └── Validate metadata.namespace in body matches URL namespace
  │
  ▼ Handler dispatches by kind
  │   ├── kind == "Schema" → SchemaService::update(stored_object)
  │   └── else            → ObjectService::update(namespace, stored_object)
  │
  ▼ ObjectService::update(namespace, stored_object)  [regular objects only]
  │   ├── Look up schema scope from SchemaRegistry
  │   ├── Validate stored_object.metadata.namespace matches namespace param
  │   ├── validate_labels(stored_object.metadata.labels) → 400 if invalid (defense-in-depth)
  │   ├── validate_annotations(stored_object.metadata.annotations) → 400 if invalid (defense-in-depth)
  │   ├── Validate spec payload against schema
  │   ├── store.transaction(key, namespace, name, callback) — OCC check on system.resourceVersion
  │   │   └── diff-based label update (read existing → compute delta → apply)
  │   └── event_bus.publish(key, WatchEvent::Modified(obj))
  │
  ▼ SchemaService::update(stored_object)  [Schema objects only — cluster-scoped]
  │   ├── SchemaRegistry.validate_and_compile() → re-compile schema
  │   ├── SchemaRegistry.insert() → replace cached validator
  │   ├── store.transaction(namespace: None) → persist
  │   └── event_bus.publish(key, WatchEvent::Modified(obj))
  │
  ▼ Response: 200 OK + StoredObject (new system.resourceVersion)
```

### Watch Events

```
GET /apis/example.io/v1/Widget?watch=true&fieldSelector=metadata.name=my-widget      (cluster)
GET /apis/example.io/v1/Widget?watch=true&labelSelector=app=nginx,env=prod           (cluster)
GET /apis/example.io/v1/namespaces/staging/NamespacedWidget?watch=true               (namespaced)
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
  │   namespace-scoped watch receives only events with matching metadata.namespace
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
| API paths | Kube-style `/apis/{group}/{version}/{kind}` (cluster) and `/apis/{group}/{version}/namespaces/{ns}/{kind}` (namespaced) | Familiar to kube users, supports both cluster-scoped and namespace-scoped resources |
| Namespace scoping | Scope is a property of the Schema (`scope: "Namespaced" | "Cluster"`), defaulting to `"Namespaced"` | Services validate namespace at runtime based on the registered schema scope |
| Schema scope | Schema is always cluster-scoped (`scope: "Cluster"`, `metadata.namespace: null`) | Schema is a global registry concept — unrelated to object namespacing |
| Namespace defaulting | Namespaced kinds on cluster-scoped URLs default to `"default"` namespace | Backward compatibility for clients that don't specify namespace |
| Namespace validation | URL namespace takes precedence over body `metadata.namespace` | Prevents spoofing — namespace is a routing concern, not a data concern |
| Cross-namespace list | `GET /apis/{group}/{version}/{kind}` with `namespace: None` returns objects from all namespaces | Standard kube pattern for admin/operator use cases |
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
