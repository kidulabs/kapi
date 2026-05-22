## Why

Integration tests are hardcoded to `InMemoryStore` via `TestApp::new()`. With `SQLiteStore` now available (and `EtcdStore` planned), we need modular store injection so the same test suite can verify all store implementations against the same behavioral requirements.

## What Changes

- Remove `TestApp::new()` — all tests must explicitly construct with a store
- Add `TestApp::with_store(Arc<dyn ObjectStore>)` for store injection
- Refactor all test functions to accept `&TestApp` instead of creating their own app
- Add `TestStore` factory struct and `all_test_stores()` to enumerate stores to test
- Update `main.rs` to loop over stores, creating a `TestApp` per store and passing it to each test
- Use `tempfile::TempDir` for SQLite, cleaned up automatically when the suite exits
- Fail fast: terminate the suite on first test failure

## Capabilities

### New Capabilities

(none — this is infrastructure, not new behavior)

### Modified Capabilities

- `integration-tests`: Test harness now supports running the same test scenarios against multiple store implementations. Requirements remain unchanged — only the test infrastructure gains store polymorphism.

## Impact

- `tests/src/lib.rs`: `TestApp` gains `with_store()`, loses `new()`; adds `TestStore` and `all_test_stores()`
- `tests/src/main.rs`: Store iteration loop, temp directory lifecycle
- `tests/src/object_crud.rs`: All functions accept `&TestApp` param
- `tests/src/optimistic_concurrency.rs`: Same
- `tests/src/schema_deletion.rs`: Same
- `tests/src/schema_validation.rs`: Same
- `tests/src/watch_events.rs`: Same
- `tests/Cargo.toml`: Add `tempfile` dependency

## Non-goals

- Etcd store support is not added now — only the hook for future addition
- No changes to `ObjectStore` trait or store implementations
- No changes to production code (`src/`)
