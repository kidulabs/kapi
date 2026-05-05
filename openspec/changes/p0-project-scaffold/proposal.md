## Why

The kapi project has a detailed roadmap and empty module directories but no actual source code, no `Cargo.toml`, and no working build. Before any domain logic can be implemented, we need a compilable project scaffold with all dependencies declared, the module tree established, and a minimal server that starts — proving the foundation is sound.

## What Changes

- Create `Cargo.toml` with all required dependencies (axum, tokio, dashmap, jsonschema, utoipa, utoipa-swagger-ui, serde, serde_json, tower, tower-http, chrono, uuid, thiserror, async-trait, tokio-stream, futures-util, tracing, tracing-subscriber)
- Create `src/lib.rs` declaring all modules (error, event, middleware, object, openapi, routes, schema, store)
- Create `src/main.rs` with a minimal tokio+axum stub binding to `0.0.0.0:8080`
- Create `src/error.rs` as an empty module (types arrive in P1)
- Create `src/routes.rs` as an empty module (routes arrive in P4+)
- Create submodule directories with empty `mod.rs` files for: `store/` (with `memory.rs`), `schema/` (with `types.rs`, `service.rs`, `handler.rs`), `object/` (with `types.rs`, `service.rs`, `handler.rs`), `event/` (with `bus.rs`), `middleware/` (with `auth.rs`, `metrics.rs`)
- Create `src/openapi.rs` as an empty module (arrives in P8)
- Place `ResourceKey` struct in `src/store/mod.rs` — the shared location since both `SchemaStore` and `ObjectStore` depend on it
- Add a minimal `#[test] fn it_works()` in `lib.rs` as a test framework sanity check
- Add `http-body-util` to dev-dependencies for future integration tests

## Capabilities

### New Capabilities
- `project-scaffold`: Project structure, build system, module tree, dependency declarations, minimal server startup, and test framework baseline

### Modified Capabilities
<!-- No existing specs to modify -->

## Impact

- **Code**: Creates the entire `src/` file tree from scratch and `Cargo.toml`
- **Dependencies**: Introduces all crate dependencies listed in the roadmap — pinned to latest stable versions (axum 0.8, dashmap 6, utoipa 5, etc.)
- **Build**: Establishes `cargo build` and `cargo test` as green from day one
- **API**: No API surface yet — server returns 404 on all routes (expected, routes arrive in P4-P5)