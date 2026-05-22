## Context

Integration tests live in `tests/` as a separate crate. `TestApp::new()` hardcodes `InMemoryStore`. The `ObjectStore` trait (defined in `src/store/mod.rs`) is already polymorphic — both `InMemoryStore` and `SQLiteStore` implement it. The test functions (`object_crud`, `watch_events`, etc.) each create their own `TestApp` internally, making it impossible to swap stores from the top level.

## Goals / Non-Goals

**Goals:**
- Every test function receives a `&TestApp` from the caller
- `main.rs` iterates over a list of stores, running all tests against each
- SQLite uses a temp file that auto-cleans on suite exit
- Fail fast on first test failure
- Future stores (etcd) can be added by appending to `all_test_stores()`

**Non-Goals:**
- No etcd implementation now
- No changes to `ObjectStore` trait or store implementations
- No changes to production code

## Decisions

### 1. Test functions take `&TestApp` instead of `Arc<dyn ObjectStore>`

Tests already call `app.client()` to get an HTTP client. Passing `&TestApp` keeps the test signatures minimal and encapsulates the store behind the app. If we ever add test helpers (event bus inspection, direct store access), they live on `TestApp`.

**Alternative considered:** Pass `Arc<dyn ObjectStore>` and have each test build its own `TestApp`. Rejected — more boilerplate per test, and tests shouldn't know about store construction.

### 2. Factory pattern via `TestStore` struct

```rust
pub struct TestStore {
    pub name: &'static str,
    pub factory: fn() -> Arc<dyn ObjectStore>,
}
```

Each store registers a name and a factory function. `all_test_stores()` returns `Vec<TestStore>`. Adding etcd later is one line.

**Alternative considered:** Enum of store types with a `build()` method. Rejected — would require the test crate to know about every store variant, creating a compile-time dependency on store crates.

### 3. Remove `TestApp::new()` entirely

Forces explicitness. Every test caller must choose a store. No hidden defaults.

**Alternative considered:** Keep `TestApp::new()` as an alias for `InMemoryStore`. Rejected — the user explicitly asked to remove it.

### 4. TempDir for SQLite, held by `main.rs`

`main.rs` creates a `tempfile::TempDir` at startup, passes the path to the SQLite factory. The `TempDir` is held until the suite exits, then RAII cleanup deletes it.

**Alternative considered:** Each test creates its own temp file. Rejected — slower (many SQLite file creates) and harder to clean up on failure.

### 5. Fail-fast in `main.rs`

The test loop checks each result and `std::process::exit(1)` on first failure. No point running remaining tests or stores when something is broken.

## Risks / Trade-offs

| Risk | Mitigation |
|------|-----------|
| SQLite tests are slower (disk I/O, `spawn_blocking`) | Acceptable for integration tests; correctness > speed |
| `&TestApp` lifetime in async test functions | `TestApp` lives in the `main.rs` loop body; references are valid for the duration of each test |
| `tempfile` dependency added to test crate | Already a common testing dependency; `dev-dependency` scope |
| Future etcd store needs external service | Factory can return `Option` or skip when unavailable; not a concern for this change |
