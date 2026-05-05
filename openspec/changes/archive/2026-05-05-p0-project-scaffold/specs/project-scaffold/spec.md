## ADDED Requirements

### Requirement: Cargo.toml with all project dependencies
The project SHALL have a `Cargo.toml` at the workspace root declaring the package name `kapi`, edition `2024`, and all dependencies listed in the roadmap: axum, tokio (full), serde (derive), serde_json, jsonschema, dashmap, tokio-stream, futures-util, tracing, tracing-subscriber (env-filter), utoipa, utoipa-swagger-ui, async-trait, chrono (serde), uuid (v4, serde), thiserror, tower, tower-http (trace, cors). Dev-dependencies SHALL include http-body-util.

#### Scenario: Dependencies resolve and compile
- **WHEN** `cargo build` is run
- **THEN** all dependencies resolve from crates.io and the project compiles without errors

### Requirement: Module tree matches roadmap architecture
`src/lib.rs` SHALL declare modules: `error`, `event`, `middleware`, `object`, `openapi`, `routes`, `schema`, `store`. Each module directory SHALL contain the submodules specified in the roadmap: `store/memory`, `schema/types`, `schema/service`, `schema/handler`, `object/types`, `object/service`, `object/handler`, `event/bus`, `middleware/auth`, `middleware/metrics`. All module files SHALL exist and compile.

#### Scenario: All modules compile
- **WHEN** `cargo build` is run
- **THEN** no module-level compilation errors occur and all `mod` declarations resolve

### Requirement: ResourceKey defined in store module
`src/store/mod.rs` SHALL define a `ResourceKey` struct with fields `group: String`, `version: String`, `kind: String`, deriving `Debug`, `Clone`, `Hash`, `Eq`, `PartialEq`. No other types SHALL be defined in P0.

#### Scenario: ResourceKey is usable across modules
- **WHEN** another module (e.g., `schema`, `object`) imports `use crate::store::ResourceKey`
- **THEN** the import resolves and compiles

### Requirement: Minimal server startup
`src/main.rs` SHALL start a tokio runtime, initialize tracing, create an empty axum `Router`, and bind a `TcpListener` to `0.0.0.0:8080`. The server SHALL start and accept connections.

#### Scenario: Server starts and returns 404
- **WHEN** `cargo run` is executed and a GET request is sent to `http://localhost:8080/`
- **THEN** the server responds with HTTP 404 (no routes are registered yet)

### Requirement: Test framework baseline
`src/lib.rs` SHALL contain a `#[cfg(test)]` module with at least one test that compiles and passes.

#### Scenario: Tests run and pass
- **WHEN** `cargo test` is executed
- **THEN** at least one test is discovered, runs, and passes with no failures