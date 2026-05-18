## Context

Currently `main.rs` (60 lines) declares 7 modules, imports 10+ types, constructs the meta-schema validator, store, event bus, `ObjectService`, `AppState`, builds the router, parses the port, binds a `TcpListener`, and calls `axum::serve`. The `lib.rs` already declares all modules as `pub mod`, making the `mod` declarations in `main.rs` redundant.

The only user-facing choices are which `ObjectStore` and `EventPublisher` implementation to use. Everything else (meta-schema compilation, service wiring, router construction) is fixed.

## Goals / Non-Goals

**Goals:**
- Reduce `main.rs` to ~15 lines: tracing init, port parsing, `AppConfig` construction, `kapi::run()` call
- Move all application construction logic into `lib.rs`
- Introduce `AppConfig` struct in a dedicated `config` module as the single configuration entry point
- Provide both `create_app(&AppConfig) -> Router` (for testing/embedding) and `run(AppConfig) -> Result<()>` (for full lifecycle)
- Remove redundant `mod` declarations from `main.rs`

**Non-Goals:**
- No changes to existing public APIs (`ObjectStore`, `EventPublisher`, routes, handlers)
- No new store or event bus implementations
- No changes to test infrastructure
- No CLI argument parsing (port still from `PORT` env var)

## Decisions

### 1. `AppConfig` as a struct with required fields (not builder, not Option)

All three fields (`port`, `store`, `event_bus`) are required. Using a struct with non-optional fields means incomplete configs are a compile-time error, not a runtime check. This is simpler than a builder pattern and sufficient for the current scope.

**Alternatives considered:**
- Builder pattern: Overkill for 3 required fields, adds complexity
- `Option` fields with `validate()`: Defers error to runtime, worse ergonomics

### 2. `create_app` takes `&AppConfig`, `run` takes `AppConfig` by value

`create_app` borrows the config because it only needs to read values to construct the `Router`. `run` takes ownership because it consumes the config and manages the full server lifecycle. This allows `run` to call `create_app` internally without cloning.

### 3. `config` as a dedicated module, not inline in `lib.rs`

Even though `AppConfig` is a single struct, a dedicated `src/config/mod.rs` module:
- Keeps `lib.rs` focused on re-exports and entry points
- Provides a natural place for future config additions (logging config, TLS, etc.)
- Follows the existing module pattern (`event/`, `store/`, `schema/`)

### 4. Tracing init stays in `main.rs`

Logging configuration is a bootstrapping concern, not an application concern. Keeping `tracing_subscriber::fmt::init()` in `main.rs` follows the convention that the binary owns process-level initialization.

### 5. Port parsing stays in `main.rs`

The port is passed to `AppConfig` by `main.rs`. The library does not read environment variables — it fails if the config is incomplete. This makes the library testable and embeddable without side effects.

## Risks / Trade-offs

| Risk | Mitigation |
|------|-----------|
| `AppConfig` fields are `Arc<dyn Trait>` — users must construct `Arc` themselves | Acceptable; the trait objects are needed for `ObjectService` anyway. Can add convenience constructors later. |
| `run()` blocks until server stops — no graceful shutdown signal handling | Acceptable for now. Can add shutdown signal parameter to `run()` in a future change. |
| Moving `mod` declarations out of `main.rs` could break if any module has `main`-specific code | Reviewed: all modules are already `pub mod` in `lib.rs` and have no `main`-specific dependencies. |
