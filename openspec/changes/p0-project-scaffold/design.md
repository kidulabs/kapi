## Context

The kapi project has a comprehensive roadmap (`roadmap.md`) defining a Kubernetes-apiserver-inspired API server in Rust. Currently, the `src/` directory contains empty subdirectories (`store/`, `schema/`, `object/`, `event/`, `middleware/`) but no source files, no `Cargo.toml`, and therefore no working build. This is a greenfield Rust project that needs its foundation laid before any domain logic can be implemented.

## Goals / Non-Goals

**Goals:**

- Establish a compilable Rust project with all 18+ crate dependencies declared
- Create the complete module tree matching the roadmap architecture
- Place `ResourceKey` in `store/mod.rs` as the shared type used by both `SchemaStore` and `ObjectStore`
- Provide a minimal `main.rs` that starts an axum server on `0.0.0.0:8080` (returns 404 on all routes — routes arrive later)
- Add a baseline test (`it_works`) proving the test harness is wired
- Verify `cargo build` and `cargo test` pass clean

**Non-Goals:**

- Implementing any domain types (`AppError`, `StoredObject`, `Schema`, `WatchEvent`, etc.) — those arrive in P1
- Implementing storage traits (`SchemaStore`, `ObjectStore`) — P2
- Implementing `EventBus` — P3
- Implementing handlers, services, or routes — P4-P6
- Implementing `AppState` wiring — P7
- Implementing OpenAPI — P8
- Integration tests — P9

## Decisions

### D1: Dependency version selection

| Crate | Version | Rationale |
|-------|---------|-----------|
| axum | 0.8 | Latest stable; SSE support built-in via `axum::response::Sse` |
| tokio | 1 (full) | Standard async runtime; `full` feature for `#[tokio::main]` |
| serde / serde_json | 1 | Standard serialization; `derive` feature on serde |
| jsonschema | 0.46 | Latest stable; compilation API for schema validation |
| dashmap | 6 | Latest stable; 7 is still RC |
| utoipa / utoipa-swagger-ui | 5 / 9 | Latest stable pair; swagger-ui needs ` ServeFile` feature later |
| tower / tower-http | 0.5 / 0.6 | Compatible with axum 0.8; `trace` + `cors` features on tower-http |
| thiserror | 2 | Latest stable; derive macro for `AppError` (P1) |
| async-trait | 0.1 | Required for `SchemaStore` / `ObjectStore` traits (P2) |
| chrono | 0.4 (serde) | Timestamp fields; serde feature for serialization |
| uuid | 1 (v4, serde) | Object IDs; serde feature for serialization |
| tokio-stream | 0.1 | Stream utilities for watch endpoints |
| futures-util | 0.3 | Stream combinators; replaces full `futures` crate — we only need stream adapters |
| tracing / tracing-subscriber | 0.1 / 0.3 (env-filter) | Structured logging; env-filter for RUST_LOG support |
| http-body-util | 0.1 (dev) | Needed for axum integration tests (ServiceExt::ready) |

### D2: ResourceKey location — `store/mod.rs`

`ResourceKey` is used by both `SchemaStore` and `ObjectStore`. Placing it in `store/mod.rs` means:

- No circular dependency between `schema` and `object` modules
- Both traits and their shared type live in the same module they're defined in
- Other modules import via `use crate::store::ResourceKey`

Alternatives considered:
- `object/types.rs` (roadmap original): Creates awkward dependency where `schema` imports from `object`
- Separate `types.rs` at crate root: Premature — `ResourceKey` is the only shared type in P0; can refactor later

### D3: Module stubs use `//! TODO` doc comments

Empty `.rs` files compile fine, but a doc comment makes intention clear and silences any future "file has no content" lint warnings.

### D4: Minimal `main.rs` — no AppState, no router content

P0's `main.rs` binds an empty `Router::new()` to port 8080. This proves:
- The tokio runtime starts
- The axum server binds
- All crate dependencies resolve

The real router wiring (AppState, schema routes, object routes, middleware) arrives in P7.

## Risks / Trade-offs

- **[dashmap 6 → 7 migration]** → dashmap 7 may change API when it stabilizes. Mitigation: the `SchemaStore`/`ObjectStore` traits abstract over dashmap, so the migration is isolated to `memory.rs`.
- **[utoipa version alignment]** → utoipa 5 and utoipa-swagger-ui 9 must stay in sync. Mitigation: pin minor versions if compatibility issues arise during P8.
- **[ResourceKey extensibility]** → Placing `ResourceKey` in `store/mod.rs` means any module that needs it depends on `store`. Mitigation: if more shared types emerge, refactor to a `common` or `types` module — but YAGNI for P0.

## Migration Plan

This is a greenfield change — no existing code to migrate. Deployment is `cargo run && curl localhost:8080` → expect 404.

Rollback: delete the generated files.

## Open Questions

- None for P0. All decisions are deferrable to later phases if they prove wrong.