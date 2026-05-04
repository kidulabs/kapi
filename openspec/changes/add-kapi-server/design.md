## Context

This is a new project. There is no existing codebase to modify. The goal is to build a kube-apiserver-inspired API server from scratch in Rust. The server will expose a REST API for registering JSON schemas and performing CRUD operations on objects validated against those schemas, with real-time change notification via event streaming.

## Goals / Non-Goals

**Goals:**
- Build an extensible API server ("kapi") using Axum
- Support schema registration (JSON Schema) with CRUD operations
- Support object CRUD with automatic schema validation on create/update
- Implement kube-native API paths: `/apis/{group}/{version}/{kind}`
- Implement optimistic concurrency using monotonic `resourceVersion`
- Implement watch semantics with `?watch=true` and SSE streaming
- Generate OpenAPI spec dynamically
- Provide pluggable storage via async trait abstractions
- Include Tower middleware stubs for auth, metrics, and tracing
- Support in-memory storage as the default v1 engine

**Non-Goals:**
- Authentication or authorization implementation (stubs only)
- Persistent storage engines (SQLite, Postgres, etcd deferred to future changes)
- Multi-node clustering or consensus
- Webhook admission controllers
- Kubernetes compatibility (inspired by, not compatible with)
- UI or CLI client

## Decisions

### Framework: Axum (not Actix-web, Warp, or Rocket)

**Rationale:** Axum is built on Tower, which provides composable middleware chains. The admission middleware pattern (auth → metrics → validation → handler) maps exactly to Tower's `ServiceBuilder::layer()` model. No other Rust web framework provides this level of middleware composability. Axum also has first-class SSE support for the watch API, and `Router::nest()` naturally supports kube-style API group routing.

**Alternatives considered:** Actix-web (mature but non-Tower middleware), Warp (powerful filters but steep debugging cost), Rocket (fairings too coarse for admission chains).

### Storage: Split Traits (not unified Store trait)

**Rationale:** `SchemaStore` and `ObjectStore` are kept separate to provide type safety at the handler level. Schema handlers declare `SchemaStore` dependency; object handlers declare `ObjectStore`. This prevents accidental misuse where object handlers might call schema operations, or vice versa.

**Alternatives considered:** Single unified `Store` trait. Rejected because it loses type-level separation of concerns.

### Event Publishing: Service Layer Publishes (not Store owns bus)

**Rationale:** The store's single responsibility is persistence. A service layer (`ObjectService`, `SchemaService`) wraps the store and guarantees that every mutation is followed by an event publication. This makes the store easier to test in isolation and keeps eventing as an orchestration concern, not a storage concern.

**Risk mitigation:** Forgot-to-publish is impossible by construction because handlers never call the store directly; they only call the service.

### v1 Storage: In-Memory (not SQLite)

**Rationale:** In-memory storage using `DashMap` provides immediate feedback and zero operational overhead for development. The storage trait abstraction means swapping to SQLite/Postgres/etc. in a future change requires only a new `impl Store` — zero handler changes.

### API Paths: Kube-Style Namespacing

**Rationale:** Using `/apis/{group}/{version}/{kind}` and `/apis/{group}/{version}/{kind}/{name}` follows Kubernetes conventions. This makes the API structure familiar to anyone who has worked with kube APIs and supports multiple API groups naturally.

### Watch: `?watch=true` Query Parameter (not separate endpoint)

**Rationale:** Kubernetes uses `?watch=true` on the list endpoint. This keeps the API surface small and follows the kube-native pattern. The handler inspects the query parameter and either returns a JSON list or an SSE stream.

### EventBus: Per-Resource-Kind Channels (not global)

**Rationale:** Each `kind` gets its own `tokio::broadcast` channel. This prevents watchers from receiving irrelevant events and matches Kubernetes semantics where you watch a specific resource type.

### Optimistic Concurrency: Global Monotonic Counter

**Rationale:** A single `AtomicU64` counter provides resource versions across all objects. This is sufficient for an in-memory store and enables the watch resume semantics ("give me events since version N").

## Risks / Trade-offs

- **[Risk]** In-memory store loses all data on restart → **Mitigation:** Explicitly documented as v1/dev-only. Storage trait abstraction makes swapping to persistent storage straightforward in a future change.
- **[Risk]** Global version counter is not per-resource-kind → **Mitigation:** For in-memory this is fine. When moving to persistent storage, each kind would get its own counter or use database-generated versioning.
- **[Risk]** `?watch=true` on list endpoint adds branching logic to handlers → **Mitigation:** Abstracted behind a helper function; handlers call `maybe_watch()` which returns either `Json(list)` or `Sse(stream)`.
- **[Risk]** Schema registry does not validate that the provided JSON Schema is well-formed → **Mitigation:** Use the `jsonschema` crate to compile the schema on registration; reject invalid schemas with a 422 error.

## Migration Plan

Not applicable — this is a new project with no existing deployment.

## Open Questions

- Should the schema registry itself be protected by the admission validation pipeline? (Currently out of scope — schemas are registered directly.)
- Should `delete` require a resourceVersion unconditionally, or keep it optional for convenience? (Current design: optional.)
- Should we add a `PATCH` endpoint with strategic merge patch semantics, or defer to future changes? (Current design: defer.)
