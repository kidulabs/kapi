## 1. SchemaRegistry Module

- [ ] 1.1 Create `src/schema/registry.rs` with `SchemaRegistry` struct holding `store: Arc<dyn ObjectStore>`, `meta_validator: Arc<dyn SchemaValidator>`, `cache: DashMap<String, Arc<dyn SchemaValidator>>`
- [ ] 1.2 Implement `SchemaRegistry::new(store, meta_validator)` — constructs with empty cache
- [ ] 1.3 Implement `validate_and_compile(data: &Value) -> Result<(SchemaData, Arc<dyn SchemaValidator>), AppError>` — meta-schema validate, parse SchemaData, compile jsonSchema, return both (no cache insertion)
- [ ] 1.4 Implement `get_validator(key: &ResourceKey) -> Result<Arc<dyn SchemaValidator>, AppError>` — cache lookup with lazy compilation on miss (fetch from store, compile, insert into cache)
- [ ] 1.5 Implement `insert(name: &str, validator: Arc<dyn SchemaValidator>)` — cache insertion
- [ ] 1.6 Implement `evict(name: &str)` — cache removal
- [ ] 1.7 Update `src/schema/mod.rs` to declare `pub mod registry` and re-export `SchemaRegistry`
- [ ] 1.8 Run `cargo check` and `cargo clippy` to verify compilation

## 2. ObjectService Refactoring

- [ ] 2.1 Replace `meta_validator: Arc<dyn SchemaValidator>` and `schema_cache: DashMap<...>` fields with `schema_registry: SchemaRegistry`
- [ ] 2.2 Update `ObjectService::new(store, event_bus, meta_validator)` to construct `SchemaRegistry` internally from `store` and `meta_validator`
- [ ] 2.3 Remove private methods: `validate_meta_schema`, `compile_jsonschema`, `lookup_object_validator`, `map_validation_errors`
- [ ] 2.4 Refactor `validate_and_create_schema` to use `schema_registry.validate_and_compile()` + `schema_registry.insert()`
- [ ] 2.5 Refactor `validate_and_update_schema` to use `schema_registry.validate_and_compile()` + `schema_registry.insert()`
- [ ] 2.6 Refactor `validate_and_create_object` to use `schema_registry.get_validator()`
- [ ] 2.7 Refactor `validate_and_update_object` to use `schema_registry.get_validator()`
- [ ] 2.8 Refactor `delete_schema` to use `schema_registry.evict()`
- [ ] 2.9 Run `cargo check` and `cargo clippy` to verify compilation

## 3. Call Site Updates

- [ ] 3.1 Update `src/routes.rs` (or wherever `ObjectService::new()` is called) — verify the constructor signature is compatible
- [ ] 3.2 Run `cargo check` to verify all call sites compile

## 4. Tests

- [ ] 4.1 Update `make_service()` test helper in `src/object/service.rs` — verify it constructs `ObjectService` correctly with the new internal `SchemaRegistry`
- [ ] 4.2 Update tests that access `service.schema_cache` directly to access `service.schema_registry.cache` (or add a test helper method)
- [ ] 4.3 Add unit tests for `SchemaRegistry` in `src/schema/registry.rs`:
  - `validate_and_compile` with valid data returns `(SchemaData, validator)` without modifying cache
  - `validate_and_compile` with invalid meta-schema returns `InvalidSchema`
  - `validate_and_compile` with uncompilable jsonSchema returns `InvalidSchema`
  - `get_validator` cache hit returns cached validator without store access
  - `get_validator` cache miss fetches, compiles, caches, and returns
  - `get_validator` cache miss with no schema in store returns `NotFound`
  - `get_validator` cache miss with uncompilable schema returns `StoredSchemaCompilationFailed`
  - `insert` adds new entry and replaces existing entry
  - `evict` removes existing entry and is no-op for non-existent entry
- [ ] 4.4 Verify existing `ObjectService` tests (T19–T33) still pass — these test the orchestration flow end-to-end
- [ ] 4.5 Run `cargo test` to verify all unit tests pass
- [ ] 4.6 Run `cargo test -p kapi-tests` to verify integration tests pass

## 5. Documentation

- [ ] 5.1 Update doc comments on `ObjectService` struct and `new()` method to reference `SchemaRegistry`
- [ ] 5.2 Add doc comments to `SchemaRegistry` struct and all public methods
- [ ] 5.3 Check `docs/` directory for any references to `schema_cache` or `meta_validator` and update
- [ ] 5.4 Check `AGENTS.md` for any relevant architecture descriptions to update

## 6. Roadmap and Spec Sync

- [ ] 6.1 Verify roadmap items in `roadmap.md` are not impacted (publish framework and validations remain open explorations)
- [ ] 6.2 After implementation, sync delta specs to main specs:
  - `openspec/specs/schema-registry/spec.md` (new)
  - `openspec/specs/object-service/spec.md` (update)
