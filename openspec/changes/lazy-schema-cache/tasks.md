## 1. Error Type

- [x] 1.1 Add `StoredSchemaCompilationFailed { schema_name: String, reason: String }` variant to `AppError` in `src/error.rs`
- [x] 1.2 Map the new variant to HTTP 500 in `impl IntoResponse for AppError`
- [x] 1.3 Run `cargo check` to verify compilation

## 2. Core Logic: Lazy Compilation

- [x] 2.1 Update `lookup_object_validator()` in `src/object/service.rs` to compile on cache miss when schema exists in store
- [x] 2.2 On compilation failure in `lookup_object_validator()`, return `AppError::StoredSchemaCompilationFailed`
- [x] 2.3 Remove any eager warmup logic from `ObjectService::new()` (if present)
- [x] 2.4 Update doc comments on `ObjectService::new()` to reflect empty cache startup
- [x] 2.5 Run `cargo check` and `cargo clippy` to verify

## 3. Tests

- [x] 3.1 Add test: object creation succeeds after simulated restart (new service sharing same store, empty cache)
- [x] 3.2 Add test: cache miss triggers compilation and subsequent requests use cached validator
- [x] 3.3 Add test: stored schema with invalid jsonSchema returns `StoredSchemaCompilationFailed` on cache miss
- [x] 3.4 Update existing tests if any assume eager warmup or panic behavior
- [x] 3.5 Run `cargo test` and `cargo test --test integration_tests` to verify all tests pass

## 4. Documentation

- [x] 4.1 Update `src/object/service.rs` doc comments for `schema_cache` and `lookup_object_validator()`
- [x] 4.2 Check `README.md` or other docs for references to schema warmup and update or remove them
- [x] 4.3 Check `AGENTS.md` for any relevant context to update
- [x] 4.4 Review `openspec/specs/schema-cache-warmup/spec.md` and `openspec/specs/object-service/spec.md` to ensure they reflect the final implementation
