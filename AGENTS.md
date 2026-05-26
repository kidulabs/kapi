# kapi

Kubernetes-apiserver-inspired API server in Rust. **Not** a Kubernetes compatibility layer.

## Architecture

```
Request → TraceLayer → CorsLayer → Handler → ObjectService → Store
                                                   │
                                          EventPublisher + SchemaValidator
                                          (trait objects via Arc<dyn>)
```

- **Handlers** (`object/handler.rs`): pure translation, no business logic
- **ObjectService** (`object/service.rs`): single orchestrator — validation, storage, event publishing
- **Store** (`store/`): pluggable `ObjectStore` trait — `InMemoryStore` (DashMap) and `SQLiteStore` (rusqlite + spawn_blocking)
- **EventBus** (`event/bus.rs`): per-kind `Vec<Watcher>` with `WatchFilter` + `mpsc::Sender` per watcher (predicate routing), `EventPublisher` trait
- **Schema** (`schema/`): `SchemaValidator` trait wrapping `jsonschema` crate, compiled validators cached in `DashMap`

## Workspace structure

- Root `Cargo.toml`: single package `kapi` + workspace with `tests` member
- `tests/`: `kapi-tests` package — integration test binary run against both InMemory and SQLite stores
- Root Cargo.toml has `autotests = false` — integration tests are the binary, not Cargo test harness

# instructions

Prioritize retrieval-led reasoning over pretrained-knowledge-led reasoning.