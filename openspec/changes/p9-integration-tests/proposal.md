## Why

Unit tests cover individual components in isolation (store, service, event bus, OpenAPI generation), but do not exercise the full HTTP request path. Integration tests verify that the router, handlers, service layer, store, and event bus work together correctly — catching issues like miswired routes, incorrect response types, or handler-service contract mismatches.

## What Changes

- New `tests/` binary with `TestApp` and `TestClient` infrastructure
- `object_crud.rs`: Full CRUD flow + pagination scenarios (T56 + pagination edge cases)
- `watch_events.rs`: SSE watch with subscribe-before-create pattern and timeout reliability (T57)
- `schema_deletion.rs`: Schema deletion with/without existing objects (T58, T59)
- `schema_validation.rs`: Schema registration with invalid jsonSchema (T60)
- `optimistic_concurrency.rs`: Update with wrong resourceVersion (T61)
- Verification that `cargo test`, `cargo clippy`, and `cargo doc --no-deps` pass cleanly (T62, T63)

## Capabilities

### New Capabilities
- `integration-tests`: Integration test suite for the kapi HTTP API, covering object CRUD, watch events, schema management, and optimistic concurrency.

### Modified Capabilities
(None — integration tests verify existing behavior without changing requirements.)

## Impact

- New `tests/` directory with Rust binary
- Depends on `kapi` crate via path dependency
- No changes to `src/` code
- No changes to public API surface