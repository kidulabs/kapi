## 1. Error Type

- [ ] 1.1 Add `StoredSchemaCompilationFailed { schema_name: String, reason: String }` variant to `AppError` in `src/error.rs`
- [ ] 1.2 Map the new variant to HTTP 500 in `impl IntoResponse for AppError`
- [ ] 1.3 Run `cargo check` to verify compilation

## 2. Core Logic: Lazy Compilation

- [ ] 2.1 Update `lookup_object_validator()` in `src/object/service.rs` to compile on cache miss when schema exists in store
- [ ] 2.2 On compilation failure in `lookup_object_validator()`, return `AppError::StoredSchemaCompilationFailed`
- [ ] 2.3 Remove any eager warmup logic from `ObjectService::new()` (if present)
- [ ] 2.4 Update doc comments on `ObjectService::new()` to reflect empty cache startup
- [ ] 2.5 Run `cargo check` and `cargo clippy` to verify

## 3. Tests

- [ ] 3.1 Add test: object creation succeeds after simulated restart (new service sharing same store, empty cache)
- [ ] 3.2 Add test: cache miss triggers compilation and subsequent requests use cached validator
- [ ] 3.3 Add test: stored schema with invalid jsonSchema returns `StoredSchemaCompilationFailed` on cache miss
- [ ] 3.4 Update existing tests if any assume eager warmup or panic behavior
- [ ] 3.5 Run `cargo test` and `cargo test --test integration_tests` to verify all tests pass

## 4. Documentation

- [ ] 4.1 Update `src/object/service.rs` doc comments for `schema_cache` and `lookup_object_validator()`
- [ ] 4.2 Check `README.md` or other docs for references to schema warmup and update or remove them
- [ ] 4.3 Check `AGENTS.md` for any relevant context to update
- [ ] 4.4 Review `openspec/specs/schema-cache-warmup/spec.md` and `openspec/specs/object-service/spec.md` to ensure they reflect the final implementation
