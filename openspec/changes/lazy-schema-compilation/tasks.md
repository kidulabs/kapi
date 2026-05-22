## 1. Schema Cache Warmup at Startup

- [ ] 1.1 Add `warmup_schema_cache()` private method to `ObjectService` that lists all Schema objects from the store, compiles each `jsonSchema`, and inserts into `schema_cache`; add intent comments explaining the warmup flow
- [ ] 1.2 Call `warmup_schema_cache()` from `ObjectService::new()` after initializing the empty `schema_cache`; add comment explaining that warmup must complete before the service accepts requests
- [ ] 1.3 Handle warmup compilation failure by panicking with a descriptive message including the schema name and error; add comment explaining why panic is the correct behavior
- [ ] 1.4 Run `cargo check` and `cargo clippy` to verify compilation

## 2. Lazy Compilation Fallback in lookup_object_validator

- [ ] 2.1 Modify `lookup_object_validator()` to compile the schema on cache miss instead of returning `NotFound`; add comment explaining the lazy compilation fallback pattern
- [ ] 2.2 Insert the compiled validator into `schema_cache` after successful compilation; add comment explaining the insert-before-return pattern
- [ ] 2.3 Panic with a descriptive message if compilation fails during lazy compile; add comment explaining why panic is correct for this failure mode
- [ ] 2.4 Run `cargo check` and `cargo clippy` to verify compilation

## 3. Tests

- [ ] 3.1 Add test: `warmup_loads_schemas_from_store` — create schemas in store, construct service, verify cache is populated
- [ ] 3.2 Add test: `warmup_empty_store_no_error` — construct service with empty store, verify no panic and cache is empty
- [ ] 3.3 Add test: `lazy_compile_on_cache_miss` — register schema, manually evict from cache, create object, verify schema is re-compiled and cached
- [ ] 3.4 Add test: `warmup_panics_on_bad_schema` — insert a schema with uncompilable jsonSchema into store, verify service construction panics
- [ ] 3.5 Run `cargo test` to verify all tests pass

## 4. Documentation Review

- [ ] 4.1 Review `roadmap.md` and add schema cache warmup to Completed section if appropriate
- [ ] 4.2 Review existing code documentation (doc comments on `ObjectService`, `lookup_object_validator`) and update if they reference the old cache-miss behavior
- [ ] 4.3 Only update documentation if it adds value and corrects deviations — do not add redundant or obvious documentation

## 5. Final Verification

- [ ] 5.1 Run `cargo clippy --all-targets` with no warnings
- [ ] 5.2 Run `cargo test` with all tests passing
