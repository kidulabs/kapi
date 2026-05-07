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
                                    │ ObjectStore │  (trait — all objects, including schemas)
                                    │ EventBus    │
                                    └─────────────┘
```

**Layers:**

1. **Tower middleware** — composable chain (auth, metrics, trace, future: admission webhook)
2. **Handlers** — thin Axum extractors + response, no business logic
3. **Services** — orchestrate store + event bus; guarantee publish on every mutation
4. **Store** — pluggable via a single `ObjectStore` async trait; v1 = in-memory (DashMap). Schema are objects too, stored in the same store (kind `"Schema"` in group `kapi.io`).
5. **EventBus** — per-kind `tokio::broadcast` channels; subscribers watch a specific kind and receive all CUD events for that kind

---

## API Surface

### Schema Registry (`/apis/kapi.io/v1/`)

Schema is itself an object kind. Schemas live at the same paths as any other object.

| Method | Path | Action |
|--------|------|--------|
| GET | `/Schema` | List all registered schemas (object-style list) |
| POST | `/Schema` | Register a new JSON Schema (validated against meta-schema) |
| GET | `/Schema/{name}` | Get a specific schema by name (e.g. `Widget.example.io`) |
| DELETE | `/Schema/{name}` | Delete a schema (409 if objects of that kind exist) |

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

struct ValidationError { path: String, message: String }

// Schema is not a separate struct. It is a StoredObject
// with kind="Schema" in group "kapi.io" and a name like
// "{TargetKind}.{TargetGroup}" (e.g. "Widget.example.io").
// The data field contains targetGroup, targetVersion,
// targetKind, and jsonSchema.
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
| Storage abstraction | Single ObjectStore trait | Schema is also an object (kind `"Schema"`); one store for everything simplifies backends |
| Event publishing | Service layer publishes, store is pure data | Impossible to "forget to publish" — handlers only call service, never store directly |
| v1 storage | In-memory (DashMap) | Zero ops overhead, perfect for dev; trait abstraction makes swapping trivial later |
| API paths | Kube-style `/apis/{group}/{version}/{kind}` | Familiar to kube users, supports multiple API groups naturally |
| Watch semantics | `?watch=true` on list endpoint | Kube-native pattern, single URL, handler branches on query param |
| Event bus | Per-resource-kind broadcast channels | Each kind gets its own channel; `?watch=true` subscribes to all CUD events for that specific kind |
| Concurrency | Global monotonic `AtomicU64` counter | Enables "give me events since version N" for watch resume; sufficient for in-memory |
| Schema validation | Builtin meta-schema compiled at startup | Schema objects validated against hardcoded meta-schema; avoids infinite recursion of Schema validating Schema |
| Schema deletion | Block if objects exist (409 Conflict) | Prevent accidental data loss — user must delete all objects of a kind before removing its schema |
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
│   ├── mod.rs             # ObjectStore trait definition (single trait)
│   └── memory.rs          # InMemoryStore (DashMap, AtomicU64)
├── schema/
│   ├── mod.rs
│   └── meta_schema.rs     # Builtin meta-schema constant + validator
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
  │   ├── if kind == "Schema": validate against builtin meta-schema
  │   │   └── compile nested jsonSchema → 422 on failure
  │   ├── if kind != "Schema": look up Schema object → validate payload
  │   ├── store.create(key, name, data)           → StoredObject
  │   └── event_bus.publish(key, WatchEvent::Added(obj)) → per-kind watchers
  │
  ▼ Response: 201 Created + StoredObject JSON

POST /apis/kapi.io/v1/Schema
  │  (Schema objects go through the exact same pipeline)
  │
  ▼ ObjectService::create(key = {kapi.io, v1, Schema}, name, data)
  │   ├── kind == "Schema" → validate against meta-schema
  │   ├── store.create() → StoredObject
  │   └── event_bus.publish() → per-kind watchers
  │
  ▼ Response: 201 Created

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

- Should schema registration itself have additional admission webhooks? (Meta-schema validation is builtin.)
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
- [x] T12: Define core types in `src/object/types.rs` — `ResourceKey`, `StoredObject`, `UserData`, `ListOptions`, `ListResponse`, `WatchEventType`, `WatchEvent`, `ValidationError`

### P2 — Storage Trait and In-Memory Implementation

- [ ] T13: Define single `ObjectStore` async trait — `create`, `get`, `list`, `update` (with `expected_version`), `delete` (with optional `expected_version`)
- [ ] T14: Implement `InMemoryStore` using `DashMap<(ResourceKey, name), ObjectEntry>` for all objects (including schemas)
- [ ] T15: Add `AtomicU64` version counter, auto-increment on every create/update
- [ ] T16: Implement optimistic concurrency in `update`: compare versions, return `Err(AppError::Conflict)` on mismatch
- [ ] T17: Implement optional version check in `delete`
- [ ] T18: Write unit tests: create+get, list, update success, update conflict, delete, get missing

### P3 — Event Bus

- [ ] T21: Define `EventBus` struct in `src/event/bus.rs` with `DashMap<ResourceKey, broadcast::Sender<WatchEvent>>` for per-kind channels
- [ ] T22: Implement `EventBus::new()`, `publish(key, event)` (auto-creates per-kind channel on first publish), `subscribe(key) -> impl Stream<WatchEvent>`
- [ ] T23: Write unit test: publish an event, subscriber receives it
- [ ] T24: Write unit test: publish an event, multiple subscribers all receive it
- [ ] T25: Write unit test: dropped subscriber does not block publisher

### P4 — Meta-Schema

- [ ] T27: Create `src/schema/meta_schema.rs` with hardcoded meta-schema JSON constant defining valid Schema object payloads (`targetGroup`, `targetVersion`, `targetKind`, `jsonSchema`)
- [ ] T28: Add meta-schema compilation function returning `jsonschema::Validator`, called at server startup
- [ ] T29: Update `src/schema/mod.rs` to declare only `pub mod meta_schema` (remove handler, service, types declarations)
- [ ] T30: Delete `src/schema/types.rs`, `src/schema/service.rs`, `src/schema/handler.rs`

### P5 — Object Domain (Service + Handlers + Validation + Watch)

- [ ] T35: Implement `ObjectService` in `src/object/service.rs` — wraps `Arc<dyn ObjectStore>` + `EventBus`, publishes events after mutations
- [ ] T36: Add meta-schema validator field to `ObjectService` (compiled at construction, used for Schema objects)
- [ ] T37: Implement validation dispatch in `ObjectService::create`/`update`: if `kind == "Schema"`, validate against meta-schema + compile nested jsonSchema; else look up Schema object from store and validate payload
- [ ] T38: Implement `ObjectService::delete` with Schema guard: if deleting a Schema object, check if objects of the target kind exist; if so, return 409 Conflict with object count
- [ ] T39: Implement object handlers in `src/object/handler.rs` — create, get, update, delete, list; include doc comments on each handler
- [ ] T40: Implement `?watch=true` detection in list handler: if `watch=true`, return `Sse<impl Stream>`, else return `Json<ListResponse>`
- [ ] T41: Wire object routes in `src/routes.rs`: `GET/POST /apis/{group}/{version}/{kind}`, `GET/PUT/DELETE /apis/{group}/{version}/{kind}/{name}`
- [ ] T42: Write unit tests: create valid object → 201, create with invalid data → 422, create for unregistered kind → 404; update with correct/wrong resourceVersion → 200/409; create valid Schema → 201, create invalid Schema → 422

### P6 — Middleware Stubs

- [ ] T43: Implement `AuthLayer` in `src/middleware/auth.rs` — Tower layer, logs "auth checked", passes through; add doc comment explaining pluggable auth contract
- [ ] T44: Implement `MetricsLayer` in `src/middleware/metrics.rs` — Tower layer, logs request count, passes through; add doc comment explaining metrics contract
- [ ] T45: Wire `TraceLayer` from `tower-http` in middleware stack
- [ ] T46: Compose full middleware stack: `ServiceBuilder::new().layer(AuthLayer).layer(MetricsLayer).layer(TraceLayer)`

### P7 — Application Wiring

- [ ] T47: Define `AppState` struct: `InMemoryStore`, `EventBus`, `ObjectService` (no separate SchemaService)
- [ ] T48: Compile meta-schema at startup, inject into `ObjectService` during construction
- [ ] T49: Create router in `src/routes.rs` — compose object routes under `/apis/{group}/{version}`, add middleware stack
- [ ] T50: Wire everything in `src/main.rs` — construct `AppState`, build router, bind to port from env var or default 8080
- [ ] T51: Verify: `cargo run` starts server, `curl http://localhost:8080/apis/kapi.io/v1/Schema` returns empty list

### P8 — OpenAPI

- [ ] T52: Add `utoipa::ToSchema` derives to all request/response types (`ResourceKey`, `StoredObject`, `AppError`, etc.)
- [ ] T53: Add `utoipa::OpenApi` derive tags and paths for all handlers
- [ ] T54: Wire `/openapi` endpoint and Swagger UI serve at `/swagger-ui/`
- [ ] T55: Verify: load `/swagger-ui/` in browser, all endpoints appear, try a request

### P9 — Integration Tests

- [ ] T56: Integration test: register schema via `/apis/kapi.io/v1/Schema` → create object via `/apis/example.io/v1/Widget` → full CRUD flow
- [ ] T57: Integration test: watch Schema objects → create schema → receive Added event
- [ ] T58: Integration test: delete schema with existing objects → 409 Conflict with object_count
- [ ] T59: Integration test: delete schema with no objects → 200 OK
- [ ] T60: Integration test: create schema with invalid jsonSchema → 422
- [ ] T61: Integration test: concurrent update with wrong resourceVersion → 409 Conflict
- [ ] T62: `cargo test` passes clean with no warnings
- [ ] T63: `cargo doc --no-deps` generates documentation without errors