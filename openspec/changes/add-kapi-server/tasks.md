## 1. Project Scaffold

- [ ] 1.1 Create `Cargo.toml` with all dependencies (axum, tokio, dashmap, jsonschema, utoipa, utoipa-swagger-ui, tower, tower-http, serde, serde_json, chrono, uuid, thiserror, async-trait, tracing, tracing-subscriber, tokio-stream, futures)
- [ ] 1.2 Create module directory tree (`src/store/`, `src/schema/`, `src/object/`, `src/event/`, `src/middleware/`)
- [ ] 1.3 Create `src/lib.rs` with module tree and re-exports
- [ ] 1.4 Create `src/main.rs` with tokio runtime bootstrap stub

## 2. Core Types and Errors

- [ ] 2.1 Define `AppError` enum in `src/error.rs` (NotFound, Conflict, SchemaValidation, Internal) with `thiserror`
- [ ] 2.2 Implement `IntoResponse` for `AppError` (404, 409, 422, 500 status codes)
- [ ] 2.3 Define shared types in `src/object/types.rs`: `ResourceKey`, `StoredObject`, `ListOptions`, `ListResponse`, `WatchEventType`, `WatchEvent`
- [ ] 2.4 Define `Schema` type in `src/schema/types.rs`

## 3. Storage Abstraction

- [ ] 3.1 Define `SchemaStore` trait in `src/store/mod.rs` with `register`, `get`, `list`, `delete` methods
- [ ] 3.2 Define `ObjectStore` trait in `src/store/mod.rs` with `create`, `get`, `list`, `update`, `delete` methods
- [ ] 3.3 Implement `InMemoryStore` in `src/store/memory.rs` using `DashMap` for both schemas and objects
- [ ] 3.4 Implement global monotonic `AtomicU64` version counter in `InMemoryStore`
- [ ] 3.5 Implement optimistic concurrency check in `update` and optional check in `delete`
- [ ] 3.6 Write unit tests for `InMemoryStore` (create, get, list, update with version, delete, conflict handling)

## 4. Event Bus

- [ ] 4.1 Define `EventBus` in `src/event/bus.rs` with per-resource-kind `tokio::broadcast` channels using `DashMap<ResourceKey, broadcast::Sender<WatchEvent>>`
- [ ] 4.2 Implement `EventBus::new(capacity: usize)`, `publish`, and `subscribe` methods
- [ ] 4.3 Write unit tests for event bus (publish/subscribe, multiple subscribers, dropped receiver cleanup)

## 5. Schema Domain

- [ ] 5.1 Implement `SchemaService` in `src/schema/service.rs` wrapping `SchemaStore`
- [ ] 5.2 Implement schema route handlers in `src/schema/handler.rs` (list, register, get, delete)
- [ ] 5.3 Add jsonschema compilation on registration to validate schemas are well-formed
- [ ] 5.4 Write unit tests for schema handlers (happy path, duplicate registration, get missing, delete missing)

## 6. Object Domain

- [ ] 6.1 Implement `ObjectService` in `src/object/service.rs` wrapping `ObjectStore` + `EventBus`
- [ ] 6.2 Implement object route handlers in `src/object/handler.rs` (list/get with `?watch=true`, create, get, update, delete)
- [ ] 6.3 Implement `maybe_watch` helper that branches between JSON list and SSE stream based on query param
- [ ] 6.4 Implement admission validation: on create/update, fetch schema from `SchemaStore` and validate payload with `jsonschema`
- [ ] 6.5 Ensure `ObjectService` publishes events after every successful mutation
- [ ] 6.6 Write unit tests for object handlers (create with validation, update with concurrency, watch stream)

## 7. Middleware

- [ ] 7.1 Create `AuthLayer` stub in `src/middleware/auth.rs` (pass-through, log "auth checked")
- [ ] 7.2 Create `MetricsLayer` stub in `src/middleware/metrics.rs` (pass-through, log request count)
- [ ] 7.3 Wire `TraceLayer` from `tower-http` for request tracing
- [ ] 7.4 Compose middleware stack in `src/routes.rs` or `src/main.rs`

## 8. Routes and Application Wiring

- [ ] 8.1 Implement `src/routes.rs` with kube-style router composition:
  - `/apis/kapi.io/v1/schemas` (list, register)
  - `/apis/kapi.io/v1/schemas/{group}/{version}/{kind}` (get, delete)
  - `/apis/{group}/{version}/{kind}` (list+watch, create)
  - `/apis/{group}/{version}/{kind}/{name}` (get, update, delete)
- [ ] 8.2 Wire `AppState` with `InMemoryStore`, `EventBus`, and services in `src/main.rs`
- [ ] 8.3 Start axum server on configurable port (default 8080)

## 9. OpenAPI

- [ ] 9.1 Add `utoipa` derive macros to request/response types and handler functions
- [ ] 9.2 Implement `/openapi.json` endpoint returning generated spec
- [ ] 9.3 Serve Swagger UI at `/swagger-ui` using `utoipa-swagger-ui`
- [ ] 9.4 Verify OpenAPI spec is valid by loading it in Swagger UI

## 10. Integration Tests

- [ ] 10.1 Write integration test: full CRUD flow for schema + object
- [ ] 10.2 Write integration test: watch stream receives events on object mutations
- [ ] 10.3 Write integration test: optimistic concurrency conflict returns 409
- [ ] 10.4 Write integration test: invalid schema/object returns 422
- [ ] 10.5 Write integration test: missing schema/object returns 404
- [ ] 10.6 Ensure `cargo test` passes cleanly

## 11. Documentation

- [ ] 11.1 Add `README.md` with build instructions, API overview, and example curl commands
- [ ] 11.2 Update `AGENTS.md` (if exists) with project conventions and architecture notes
