## 1. Test harness infrastructure

- [x] 1.1 Add `tempfile` dependency to `tests/Cargo.toml`
- [x] 1.2 Add `TestApp::with_store(Arc<dyn ObjectStore>)` constructor to `tests/src/lib.rs`
- [x] 1.3 Remove `TestApp::new()` and `Default` impl from `tests/src/lib.rs`
- [x] 1.4 Add `TestStore` struct and `all_test_stores()` function to `tests/src/lib.rs`
- [x] 1.5 Verify `tests/src/lib.rs` compiles (`cargo check -p kapi-tests`)

## 2. Refactor test modules to accept `&TestApp`

- [x] 2.1 Refactor `tests/src/object_crud.rs` — all functions take `app: &TestApp`, remove internal `TestApp::new()` calls
- [x] 2.2 Refactor `tests/src/optimistic_concurrency.rs` — same pattern
- [x] 2.3 Refactor `tests/src/schema_deletion.rs` — same pattern
- [x] 2.4 Refactor `tests/src/schema_validation.rs` — same pattern
- [x] 2.5 Refactor `tests/src/watch_events.rs` — same pattern
- [x] 2.6 Verify all test modules compile (`cargo check -p kapi-tests`)

## 3. Rewrite main.rs test runner

- [x] 3.1 Rewrite `main.rs` to iterate over `all_test_stores()`, creating `TestApp` per store
- [x] 3.2 Add temp directory lifecycle for SQLite store
- [x] 3.3 Add fail-fast behavior (exit on first failure)
- [x] 3.4 Add store-grouped output headers (`=== memory ===`, `=== sqlite ===`)
- [x] 3.5 Verify full suite compiles (`cargo check -p kapi-tests`)

## 4. Verification

- [x] 4.1 Run integration tests: `cargo run -p kapi-tests` — all pass against both stores
- [x] 4.2 Run `cargo clippy -p kapi-tests -- -D warnings` — no warnings
- [x] 4.3 Run `cargo doc -p kapi-tests --no-deps` — no errors
