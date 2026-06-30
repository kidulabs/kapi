# kapi

Kubernetes-apiserver-inspired API server in Rust. **Not** a Kubernetes compatibility layer.

## Architecture

```
Request → TraceLayer → CorsLayer → Handler ──┬── SchemaService ──┐
                                              │                    │
                                              └── ObjectService ──┤
                                                                   ▼
                                                                 Store
                                                   │
                                          EventPublisher + SchemaValidator
                                          (trait objects via Arc<dyn>)
```

- **Handlers** (`object/handler.rs`): pure translation, no business logic. Dispatches to `SchemaService` or `ObjectService` based on `kind == "Schema"`. Supports both cluster-scoped and namespace-scoped URL patterns.
  - Cluster-scoped: `GET/POST /apis/{group}/{version}/{kind}` and `GET/PUT/DELETE /apis/{group}/{version}/{kind}/{name}`
  - Namespace-scoped: `GET/POST /apis/{group}/{version}/namespaces/{ns}/{kind}` and `GET/PUT/DELETE /apis/{group}/{version}/namespaces/{ns}/{kind}/{name}`
  - Handlers extract `namespace` from the URL path (or use `None` for cluster-scoped routes).
- **ObjectService** (`object/service.rs`): orchestrator for regular object CRUD — validation, storage, event publishing. All mutations use `transaction()` with callbacks. Performs **scope validation**: cluster-scoped kinds reject namespace in URL, namespaced kinds default to `"default"` namespace on cluster-scoped URLs. Performs **namespace existence validation** on object CREATE — see [Namespace resource](#namespace-resource). Shared helpers in `object/helpers.rs`, finalizer state machine in `object/finalizer.rs`.
- **SchemaService** (`object/schema_service.rs`): dedicated service for Schema lifecycle management (create, update, delete). Schema is **always cluster-scoped** — all operations pass `namespace: None` to the store. Uses the same store and event bus as `ObjectService`.
- **Store** (`store/`): pluggable `ObjectStore` trait — `InMemoryStore` (DashMap) and `SQLiteStore` (rusqlite + spawn_blocking). All store methods accept `namespace: Option<&str>` for namespace-aware operations. Single `transaction()` method replaces `update()`, `delete()`, `update_status()`.
  - `TransactionOp::Apply` — persist with automatic `resource_version` bump
  - `TransactionOp::Delete` — hard-delete from storage
  - `TransactionOp::Abort` — reject with error, no changes
  - The store is a dumb persistence layer — all business logic (generation bumping, finalizer checks) lives in service callbacks.
- **EventBus** (`event/bus.rs`): per-kind `Vec<Watcher>` with `WatchFilter` + `mpsc::Sender` per watcher (predicate routing), `EventPublisher` trait. Events carry `StoredObject` with `namespace` field. `WatchFilter::Namespace(String)` filters events to a specific namespace.
- **Schema** (`schema/`): `SchemaValidator` trait + `SchemaRegistry` — manages validation, compilation, and caching of JSON schemas. Caches `scope` (Namespaced/Cluster) alongside each compiled validator for use by ObjectService scope validation.

## Namespace resource

Namespace is a first-class **cluster-scoped** core type (`kind: "Namespace"`, `group: "kapi.io"`, `version: "v1"`). The schema is registered at server startup; the `"default"` namespace is auto-created and is undeletable.

### Lifecycle rules

- **`"default"` namespace**: auto-created at startup, undeletable (DELETE returns 403 Forbidden via `AppError::ProtectedNamespace`).
- **Other namespaces**: created via the normal object API; can be deleted only when empty (DELETE returns 409 Conflict with `AppError::NamespaceNotEmpty` if any objects exist in the namespace).
- **Namespace existence validation**: on object CREATE, namespaced kinds check that the target namespace exists. Returns 404 Not Found with `AppError::NotFound { what: "namespace", identifier }` if missing.
- **Cluster-scoped kinds** skip namespace existence checks (they have no namespace).

### Namespace-scoped watch

When watching a namespaced kind via `/apis/{g}/{v}/namespaces/{ns}/{kind}?watch=true`, the watch handler creates `WatchFilter::Namespace(ns)`. Cross-namespace watch (no namespace in URL) uses `WatchFilter::All`. Both can be combined with `WatchFilter::And` for field/label selectors.

### API

- Cluster-scoped: `GET/POST /apis/kapi.io/v1/Namespace` and `GET/PUT/DELETE /apis/kapi.io/v1/Namespace/{name}`
- All Namespace operations are cluster-scoped — namespace in the URL (if any) is ignored.

## Bootstrap

`create_app` runs `bootstrap_builtins` (in `bootstrap.rs`) after constructing the store and services but before building the router:

1. Registers the built-in `Namespace` schema (idempotent — no-op if already present).
2. Creates the `"default"` Namespace object if it doesn't exist (idempotent).

Bootstrap failure causes server startup to fail fast with a clear error message.

## Workspace structure

- Root `Cargo.toml`: single package `kapi` + workspace with `tests` member
- `tests/`: `kapi-tests` package — integration test binary run against both InMemory and SQLite stores
- Root Cargo.toml has `autotests = false` — integration tests are the binary, not Cargo test harness

# instructions

Prioritize retrieval-led reasoning over pretrained-knowledge-led reasoning.