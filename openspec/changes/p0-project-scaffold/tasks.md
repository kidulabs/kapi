## 1. Project Configuration

- [x] 1.1 Create `Cargo.toml` with package name `kapi`, edition `2021`, and all production dependencies (axum 0.8, tokio 1/full, serde 1/derive, serde_json 1, jsonschema 0.46, dashmap 6, tokio-stream 0.1, futures-util 0.3, tracing 0.1, tracing-subscriber 0.3/env-filter, utoipa 5, utoipa-swagger-ui 9, async-trait 0.1, chrono 0.4/serde, uuid 1/v4+serde, thiserror 2, tower 0.5, tower-http 0.6/trace+cors) and dev-dependencies (http-body-util 0.1)
- [x] 1.2 Verify `cargo build` succeeds with empty source files

## 2. Module Tree

- [x] 2.1 Create `src/lib.rs` declaring modules: `error`, `event`, `middleware`, `object`, `openapi`, `routes`, `schema`, `store` with a `#[cfg(test)]` block containing a baseline `it_works` test
- [x] 2.2 Create `src/error.rs` with a `//! TODO` doc comment
- [x] 2.3 Create `src/routes.rs` with a `//! TODO` doc comment
- [x] 2.4 Create `src/openapi.rs` with a `//! TODO` doc comment
- [x] 2.5 Create `src/store/mod.rs` with `pub mod memory;` declaration and `ResourceKey` struct (fields: `group: String`, `version: String`, `kind: String`; derives: `Debug`, `Clone`, `Hash`, `Eq`, `PartialEq`)
- [x] 2.6 Create `src/store/memory.rs` with a `//! TODO` doc comment
- [x] 2.7 Create `src/schema/mod.rs` declaring submodules: `types`, `service`, `handler`
- [x] 2.8 Create `src/schema/types.rs`, `src/schema/service.rs`, `src/schema/handler.rs` each with a `//! TODO` doc comment
- [x] 2.9 Create `src/object/mod.rs` declaring submodules: `types`, `service`, `handler`
- [x] 2.10 Create `src/object/types.rs`, `src/object/service.rs`, `src/object/handler.rs` each with a `//! TODO` doc comment
- [x] 2.11 Create `src/event/mod.rs` declaring submodule: `bus`
- [x] 2.12 Create `src/event/bus.rs` with a `//! TODO` doc comment
- [x] 2.13 Create `src/middleware/mod.rs` declaring submodules: `auth`, `metrics`
- [x] 2.14 Create `src/middleware/auth.rs`, `src/middleware/metrics.rs` each with a `//! TODO` doc comment
- [x] 2.15 Verify `cargo build` succeeds with all modules declared

## 3. Minimal Server

- [ ] 3.1 Create `src/main.rs` with `#[tokio::main]` that initializes `tracing_subscriber::fmt()`, creates an empty `Router::new()`, binds `TcpListener` to `0.0.0.0:8080`, and serves via `axum::serve`
- [ ] 3.2 Verify `cargo build` succeeds

## 4. Verification

- [ ] 4.1 Run `cargo test` and confirm the baseline test passes
- [ ] 4.2 Run `cargo build` and confirm no warnings or errors
- [ ] 4.3 Start the server (`cargo run`) and confirm it binds to port 8080 (returns 404 on GET /)