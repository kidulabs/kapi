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

- **Handlers** (`object/handler.rs`): pure translation, no business logic. Dispatches to `SchemaService` or `ObjectService` based on `kind == "Schema"`.
- **ObjectService** (`object/service.rs`): orchestrator for regular object CRUD — validation, storage, event publishing. All mutations use `transaction()` with callbacks. Shared helpers in `object/helpers.rs`, finalizer state machine in `object/finalizer.rs`.
- **SchemaService** (`object/schema_service.rs`): dedicated service for Schema lifecycle management (create, update, delete). Uses the same store and event bus as `ObjectService`.
- **Store** (`store/`): pluggable `ObjectStore` trait — `InMemoryStore` (DashMap) and `SQLiteStore` (rusqlite + spawn_blocking). Single `transaction()` method replaces `update()`, `delete()`, `update_status()`.
  - `TransactionOp::Apply` — persist with automatic `resource_version` bump
  - `TransactionOp::Delete` — hard-delete from storage
  - `TransactionOp::Abort` — reject with error, no changes
  - The store is a dumb persistence layer — all business logic (generation bumping, finalizer checks) lives in service callbacks.
- **EventBus** (`event/bus.rs`): per-kind `Vec<Watcher>` with `WatchFilter` + `mpsc::Sender` per watcher (predicate routing), `EventPublisher` trait
- **Schema** (`schema/`): `SchemaValidator` trait + `SchemaRegistry` — manages validation, compilation, and caching of JSON schemas

## Workspace structure

- Root `Cargo.toml`: single package `kapi` + workspace with `tests` member
- `tests/`: `kapi-tests` package — integration test binary run against both InMemory and SQLite stores
- Root Cargo.toml has `autotests = false` — integration tests are the binary, not Cargo test harness

# instructions

Prioritize retrieval-led reasoning over pretrained-knowledge-led reasoning.