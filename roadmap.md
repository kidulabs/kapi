# kapi — Roadmap

## Project Goal

A Kubernetes-apiserver-inspired API server in Rust where users register JSON Schemas for custom object kinds, then CRUD objects validated against those schemas, with real-time change notification via SSE watch semantics and pluggable storage.

This is **not** a Kubernetes compatibility layer — it borrows the API model (group/version/kind, resourceVersion, watch) but is a standalone system.

---

## Architecture

```
Request → Auth Layer → Metrics Layer → Admission Validation → Handler → Service → Store
                                                                        │
                                                                        └→ EventBus → SSE Watchers

                                   ┌─────────────┐
                                   │  AppState   │
                                   │             │
                                   │ SchemaStore │  (trait)
                                   │ ObjectStore │  (trait)
                                   │ EventBus    │
                                   └─────────────┘
```

**Layers:**

1. **Tower middleware** — composable chain (auth, metrics, trace, future: admission webhook)
2. **Handlers** — thin Axum extractors + response, no business logic
3. **Services** — orchestrate store + event bus; guarantee publish on every mutation
4. **Store** — pluggable via `SchemaStore` + `ObjectStore` async traits; v1 = in-memory (DashMap)
5. **EventBus** — per-kind `tokio::broadcast` channels; subscribers watch a specific kind and receive all CUD events for that kind

---

## API Surface

### Schema Registry (`/apis/kapi.io/v1/`)

| Method | Path | Action |
|--------|------|--------|
| GET | `/schemas` | List all registered schemas |
| POST | `/schemas` | Register a new JSON Schema (validated on admission) |
| GET | `/schemas/{group}/{version}/{kind}` | Get a specific schema |
| DELETE | `/schemas/{group}/{version}/{kind}` | Delete a schema |

### Object CRUD (`/apis/{group}/{version}/`)

| Method | Path | Action |
|--------|------|--------|
| GET | `/{kind}?watch=true` | List objects, or stream watch events |
| POST | `/{kind}` | Create object (validated against registered schema) |
| GET | `/{kind}/{name}` | Get a specific object |
| PUT | `/{kind}/{name}?resourceVersion=N` | Update with optimistic concurrency |
| DELETE | `/{kind}/{name}` | Delete object |

### Other

| Method | Path | Action |
|--------|------|--------|
| GET | `/openapi` | OpenAPI specification |
| GET | `/swagger-ui/` | Swagger UI |

---

## Key Types

```rust
#[derive(Hash, Eq, PartialEq, Clone, Serialize, Deserialize)]
struct ResourceKey { group: String, version: String, kind: String }

struct UserData { value: serde_json::Value }

struct ContinueToken(String);

struct Schema {
    key: ResourceKey,
    json_schema: serde_json::Value,
    created_at: chrono::DateTime<chrono::Utc>,
}

struct ValidationError { path: String, message: String }

struct StoredObject {
    key: ResourceKey,
    name: String,
    data: UserData,
    version: u64,  // resourceVersion, global monotonic
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
}

struct ListOptions { limit: Option<usize>, continue_token: Option<ContinueToken> }
struct ListResponse { items: Vec<StoredObject>, continue_token: Option<ContinueToken> }

enum WatchEventType { Added, Modified, Deleted }
struct WatchEvent { event_type: WatchEventType, object: StoredObject }

enum AppError {
    NotFound { what: String, identifier: String },
    Conflict { expected: u64, actual: u64 },
    SchemaValidation(Vec<ValidationError>),
    Internal(anyhow::Error),
}
```

---

## Storage Traits

```rust
#[async_trait]
trait SchemaStore: Send + Sync {
    async fn register(&self, schema: Schema) -> Result<Schema, AppError>;
    async fn get(&self, key: &ResourceKey) -> Result<Schema, AppError>;
    async fn list(&self) -> Result<Vec<Schema>, AppError>;
    async fn delete(&self, key: &ResourceKey) -> Result<Schema, AppError>;
}

#[async_trait]
trait ObjectStore: Send + Sync {
    async fn create(&self, key: &ResourceKey, name: &str, data: Value) -> Result<StoredObject, AppError>;
    async fn get(&self, key: &ResourceKey, name: &str) -> Result<StoredObject, AppError>;
    async fn list(&self, key: &ResourceKey, opts: ListOptions) -> Result<ListResponse, AppError>;
    async fn update(&self, key: &ResourceKey, name: &str, data: Value, expected_version: u64) -> Result<StoredObject, AppError>;
    async fn delete(&self, key: &ResourceKey, name: &str, expected_version: Option<u64>) -> Result<StoredObject, AppError>;
}
```

---

## Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Framework | Axum | Tower composability for middleware chain, SSE support, nested routers |
| Storage abstraction | Split traits (SchemaStore + ObjectStore) | Type safety at handler level; schema handlers can't accidentally call object operations |
| Event publishing | Service layer publishes, store is pure data | Impossible to "forget to publish" — handlers only call service, never store directly |
| v1 storage | In-memory (DashMap) | Zero ops overhead, perfect for dev; trait abstraction makes swapping trivial later |
| API paths | Kube-style `/apis/{group}/{version}/{kind}` | Familiar to kube users, supports multiple API groups naturally |
| Watch semantics | `?watch=true` on list endpoint | Kube-native pattern, single URL, handler branches on query param |
| Event bus | Per-resource-kind broadcast channels | Each kind gets its own channel; `?watch=true` subscribes to all CUD events for that specific kind |
| Concurrency | Global monotonic `AtomicU64` counter | Enables "give me events since version N" for watch resume; sufficient for in-memory |
| Schema validation on registration | Compile JSON Schema via `jsonschema` crate | Reject invalid schemas at registration time with 422 |

---

## Module Tree

```
src/
├── main.rs                # Tokio runtime, wire everything, start server
├── lib.rs                 # Module tree, re-exports
├── error.rs               # AppError enum + IntoResponse impl
├── routes.rs               # Router composition (all route definitions)
├── store/
│   ├── mod.rs             # SchemaStore + ObjectStore trait definitions
│   └── memory.rs          # InMemoryStore (DashMap, AtomicU64)
├── schema/
│   ├── mod.rs
│   ├── types.rs           # Schema struct
│   ├── service.rs         # SchemaService<SchemaStore>
│   └── handler.rs         # Axum route handlers for /schemas
├── object/
│   ├── mod.rs
│   ├── types.rs           # StoredObject, ResourceKey, WatchEvent, etc.
│   ├── service.rs         # ObjectService<ObjectStore + EventBus>
│   └── handler.rs         # Axum route handlers for /objects + watch
├── event/
│   ├── mod.rs
│   └── bus.rs             # EventBus (DashMap<ResourceKey, broadcast::Sender>)
├── middleware/
│   ├── mod.rs
│   ├── auth.rs            # AuthLayer stub
│   └── metrics.rs         # MetricsLayer stub
└── openapi.rs              # utoipa OpenAPI spec + Swagger UI
```

---

## Dependencies

```toml
axum, tokio (full), serde, serde_json, jsonschema, dashmap,
tokio-stream, futures, tracing, tracing-subscriber,
utoipa, utoipa-swagger-ui, async-trait, chrono,
thiserror, anyhow, tower, tower-http (trace, cors)
```

---

## Request Flow

```
POST /apis/example.io/v1/Widget/my-widget
  │
  ▼ AuthLayer (stub) → MetricsLayer (stub) → TraceLayer
  │
  ▼ Admission: fetch schema for ResourceKey { example.io, v1, Widget } → validate payload
  │
  ▼ Handler: extract path params into ResourceKey + name + body
  │
  ▼ ObjectService::create(key, name, data)
  │   ├── store.create(key, name, data)           → StoredObject
  │   └── event_bus.publish(key, WatchEvent::Added(obj)) → per-kind watchers
  │
  ▼ Response: 201 Created + StoredObject JSON

GET /apis/example.io/v1/Widget?watch=true
  │
  ▼ Handler: detect ?watch=true, build ResourceKey from path
  │
  ▼ event_bus.subscribe(key) → BroadcastStream<WatchEvent>
  │
  ▼ Response: SSE stream of Added/Modified/Deleted events
```

---

## Non-Goals (v1)

- Auth/authorization implementation (stubs only)
- Persistent storage (SQLite, Postgres, etcd)
- Multi-node clustering or consensus
- Webhook admission controllers
- Kubernetes API compatibility
- PATCH endpoint (defer)
- UI or CLI client

---

## Open Questions

- Should schema registration itself go through admission validation? (Out of scope for v1.)
- Should `delete` require `resourceVersion` unconditionally? (Current: optional.)
- PATCH with strategic merge patch? (Deferred.)

---

## Backlog

### P0 — Project Scaffold

- [x] T1: Create `Cargo.toml` with all dependencies (axum, tokio, dashmap, jsonschema, utoipa, utoipa-swagger-ui, tower, tower-http, serde, serde_json, chrono, uuid, thiserror, async-trait, tracing, tracing-subscriber, tokio-stream, futures)
- [x] T2: Create module directory tree: `src/{store,schema,object,event,middleware}/` with `mod.rs` in each
- [x] T3: Create `src/lib.rs` declaring all modules
- [x] T4: Create `src/main.rs` with tokio `#[tokio::main]` stub that binds to `0.0.0.0:8080`
- [x] T5: Verify `cargo build` succeeds

### P1 — Core Types and Errors

- [x] T6: Define `AppError` in `src/error.rs` — variants: `NotFound { what, identifier }`, `Conflict { expected, actual }`, `SchemaValidation(Vec<ValidationError>)`, `Internal(anyhow::Error)` — derive `thiserror::Error`
- [x] T7: Implement `IntoResponse` for `AppError` — map to 404, 409, 422, 500 with rich JSON body `{"error", "code", "details"}`
- [x] T8: Complete `ResourceKey { group, version, kind }` in `src/store/mod.rs` with `Hash`, `Eq`, `Clone`, `Serialize`, `Deserialize`
- [x] T9: Define `StoredObject { key: ResourceKey, name, data: UserData, version, created_at, updated_at }` in `src/object/types.rs`
- [x] T10: Define `ListOptions { limit, continue_token: Option<ContinueToken> }` and `ListResponse { items, continue_token: Option<ContinueToken> }` in `src/object/types.rs`
- [x] T11: Define `WatchEventType { Added, Modified, Deleted }` and `WatchEvent { event_type, object }` in `src/object/types.rs`
- [x] T12: Define `Schema { key: ResourceKey, json_schema, created_at }` in `src/schema/types.rs`

### P2 — Storage Traits and In-Memory Implementation

- [ ] T13: Define `SchemaStore` async trait in `src/store/mod.rs` — `register`, `get`, `list`, `delete` (uses `ResourceKey` instead of separate group/version/kind params)
- [ ] T14: Define `ObjectStore` async trait in `src/store/mod.rs` — `create`, `get`, `list`, `update` (with `expected_version`), `delete` (with optional `expected_version`)
- [ ] T15: Implement `InMemorySchemaStore` inner in `InMemoryStore` using `DashMap<ResourceKey, SchemaEntry>`
- [ ] T16: Implement `InMemoryObjectStore` inner in `InMemoryStore` using `DashMap<(ResourceKey, name), ObjectEntry>`
- [ ] T17: Add `AtomicU64` version counter to `InMemoryStore`, auto-increment on every create/update
- [ ] T18: Implement optimistic concurrency in `update`: compare `expected_version` with stored version, return `Err(AppError::Conflict)` on mismatch
- [ ] T19: Implement optional version check in `delete`
- [ ] T20: Write unit tests for `InMemoryStore`: create+get, list, update success, update conflict, delete, get missing returns 404

### P3 — Event Bus

- [ ] T21: Define `EventBus` struct in `src/event/bus.rs` with `DashMap<ResourceKey, broadcast::Sender<WatchEvent>>` for per-kind channels
- [ ] T22: Implement `EventBus::new()`, `publish(key, event)` (auto-creates per-kind channel on first publish), `subscribe(key) -> impl Stream<WatchEvent>`
- [ ] T23: Write unit test: publish an event, subscriber receives it
- [ ] T24: Write unit test: publish an event, multiple subscribers all receive it
- [ ] T25: Write unit test: dropped subscriber does not block publisher

### P4 — Schema Domain (Service + Handlers)

- [ ] T27: Implement `SchemaService` in `src/schema/service.rs` — wraps `Arc<dyn SchemaStore>`, delegates CRUD
- [ ] T28: Implement schema handlers in `src/schema/handler.rs` — Axum extractors for `State`, `Path`, `Json`, return `(StatusCode, Json<Schema>)`; include doc comments on each handler explaining the endpoint
- [ ] T29: Add jsonschema compilation on registration: attempt `jsonschema::validator_for(&schema.json_schema)`, return 422 on failure
- [ ] T30: Wire schema routes in `src/routes.rs`: `GET/POST /apis/kapi.io/v1/schemas`, `GET/DELETE /apis/kapi.io/v1/schemas/{group}/{version}/{kind}`
- [ ] T31: Write unit test: POST valid schema → 201
- [ ] T32: Write unit test: POST invalid JSON Schema → 422
- [ ] T33: Write unit test: GET existing schema → 200, GET missing → 404
- [ ] T34: Write unit test: DELETE schema → 200, DELETE missing → 404

### P5 — Object Domain (Service + Handlers + Validation + Watch)

- [ ] T35: Implement `ObjectService` in `src/object/service.rs` — wraps `Arc<dyn ObjectStore>` + `EventBus`, publishes events after mutations
- [ ] T36: Implement object handlers in `src/object/handler.rs` — create, get, update, delete, list; include doc comments on each handler
- [ ] T37: Implement `?watch=true` detection in list handler: if `watch=true`, return `Sse<impl Stream>`, else return `Json<ListResponse>`
- [ ] T38: Implement admission validation in `ObjectService::create` and `ObjectService::update`: fetch schema from `SchemaStore`, validate payload with `jsonschema`, return 422 on failure, 404 if kind not registered
- [ ] T39: Wire object routes in `src/routes.rs`: `GET/POST /apis/{group}/{version}/{kind}`, `GET/PUT/DELETE /apis/{group}/{version}/{kind}/{name}`
- [ ] T40: Write unit test: create valid object → 201, create with invalid data → 422, create for unregistered kind → 404
- [ ] T41: Write unit test: update with correct resourceVersion → 200, update with wrong version → 409
- [ ] T42: Write integration test: watch stream receives Added/Modified/Deleted events

### P6 — Middleware Stubs

- [ ] T43: Implement `AuthLayer` in `src/middleware/auth.rs` — Tower layer, logs "auth checked", passes through; add doc comment explaining pluggable auth contract
- [ ] T44: Implement `MetricsLayer` in `src/middleware/metrics.rs` — Tower layer, logs request count, passes through; add doc comment explaining metrics contract
- [ ] T45: Wire `TraceLayer` from `tower-http` in middleware stack
- [ ] T46: Compose full middleware stack: `ServiceBuilder::new().layer(AuthLayer).layer(MetricsLayer).layer(TraceLayer)`

### P7 — Application Wiring

- [ ] T47: Define `AppState` struct: `Arc<InMemoryStore>`, `EventBus`, `SchemaService`, `ObjectService`
- [ ] T48: Create router in `src/routes.rs` — compose schema routes + object routes + middleware; add doc comments on route groups
- [ ] T49: Wire everything in `src/main.rs` — construct `AppState`, build router, bind to port from env var or default 8080; add module-level doc comment
- [ ] T50: Verify: `cargo run` starts server, `curl http://localhost:8080/apis/kapi.io/v1/schemas` returns empty list

### P8 — OpenAPI

- [ ] T51: Add `utoipa::ToSchema` derives to all request/response types (`ResourceKey`, `Schema`, `StoredObject`, `AppError`, etc.)
- [ ] T52: Add `utoipa::OpenApi` derive tags and paths for all handlers
- [ ] T53: Wire `/openapi` endpoint and Swagger UI serve at `/swagger-ui/`
- [ ] T54: Verify: load `/swagger-ui/` in browser, all endpoints appear, try a request

### P9 — Integration Tests

- [ ] T55: Integration test: register schema → create object → get object → update object → delete object — full CRUD flow
- [ ] T56: Integration test: watch stream → create object → receive Added event → update → receive Modified → delete → receive Deleted
- [ ] T57: Integration test: concurrent update with wrong resourceVersion → 409 Conflict
- [ ] T57: Integration test: concurrent update with wrong resourceVersion → 409 Conflict
- [ ] T58: Integration test: create object with invalid data against schema → 422
- [ ] T59: Integration test: all 404 cases (get missing, delete missing, watch missing kind)
- [ ] T60: `cargo test` passes clean with no warnings
- [ ] T61: `cargo doc --no-deps` generates documentation without errors