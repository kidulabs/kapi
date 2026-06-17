## 1. Extract Shared Helpers

- [ ] 1.1 Create `src/object/helpers.rs` module with `pub(crate)` free functions: `apply_with_metadata`, `publish_event`, `map_validation_errors`. Extract these from `ObjectService` private methods. Add doc comments explaining each helper's intent.
- [ ] 1.2 Update `src/object/mod.rs` to declare the `helpers` module.
- [ ] 1.3 Update `ObjectService` to call the free functions from `helpers.rs` instead of `self.method()`. Run `cargo check` to verify compilation.

## 2. Move Selector Parsing to types.rs

- [ ] 2.1 Move `parse_field_selector`, `parse_label_selector`, `parse_label_requirement`, and `validate_label_key` from `handler.rs` to `types.rs` as `impl FieldSelector { pub fn parse(...) }` and `impl LabelSelector { pub fn parse(...) }` with private helper methods. Add doc comments explaining parsing rules.
- [ ] 2.2 Update `handler.rs` to call `FieldSelector::parse(raw)?` and `LabelSelector::parse(raw)?` instead of the old free functions. Remove the old function definitions.
- [ ] 2.3 Run `cargo check` and `cargo clippy` to verify no regressions.

## 3. Extract Finalizer State Machine

- [ ] 3.1 Create `src/object/finalizer.rs` with `DeleteAction` enum and pure functions: `evaluate_delete(existing: &StoredObject) -> DeleteAction` and `evaluate_update(existing: &StoredObject, incoming_finalizers: &[String]) -> FinalizerDecision`. Add doc comments explaining the state machine transitions.
- [ ] 3.2 Update `src/object/mod.rs` to declare the `finalizer` module.
- [ ] 3.3 Refactor `ObjectService::delete()` to use `finalizer::evaluate_delete()` for the decision, then execute via `store.transaction()`. Refactor `validate_and_update_object` to use `finalizer::evaluate_update()` for the deletion guard and finalizer-only-change checks.
- [ ] 3.4 Run `cargo check` and `cargo clippy` to verify compilation.

## 4. Create SchemaService

- [ ] 4.1 Create `src/object/schema_service.rs` with `SchemaService` struct holding `store: Arc<dyn ObjectStore>`, `event_bus: Arc<dyn EventPublisher>`, `schema_registry: SchemaRegistry`. Implement `new()`, `create()`, `update()`, `delete()` methods by moving `validate_and_create_schema`, `validate_and_update_schema`, `delete_schema` from `ObjectService`. Use shared helpers from `helpers.rs`. Add doc comments explaining Schema lifecycle orchestration.
- [ ] 4.2 Expose `get_validator()` on `SchemaService` (or provide access to `SchemaRegistry`) so `ObjectService` can look up validators for object validation.
- [ ] 4.3 Update `src/object/mod.rs` to declare the `schema_service` module and re-export `SchemaService`.
- [ ] 4.4 Run `cargo check` to verify SchemaService compiles.

## 5. Refactor ObjectService

- [ ] 5.1 Remove Schema-specific methods from `ObjectService`: `validate_and_create_schema`, `validate_and_update_schema`, `delete_schema`. Remove `if key.kind == SCHEMA_KIND` dispatch from `create()`, `update()`, `delete()`.
- [ ] 5.2 Update `ObjectService::new()` to accept `SchemaRegistry` as a parameter (shared with SchemaService) instead of constructing it internally.
- [ ] 5.3 Update `ObjectService::create()` to only handle regular objects: get validator from registry, validate spec, set metadata, store, publish.
- [ ] 5.4 Update `ObjectService::update()` to only handle regular objects: get validator, validate spec, transaction with OCC + finalizer checks via `finalizer` module, publish.
- [ ] 5.5 Update `ObjectService::delete()` to only handle regular objects: use `finalizer::evaluate_delete()` for decision, execute via transaction, publish.
- [ ] 5.6 Run `cargo check` and `cargo clippy` to verify compilation.

## 6. Update Handler and Routes

- [ ] 6.1 Update `AppState` in `src/routes.rs` to include `SchemaService` alongside `ObjectService`. Update `build_router()` and `create_app()` to construct both services, sharing the `SchemaRegistry`.
- [ ] 6.2 Update `create` handler to dispatch: `if kind == SCHEMA_KIND { schema_service.create(...) } else { object_service.create(...) }`. Remove handler-level `validate_labels`, `validate_annotations`, `validate_finalizers` calls.
- [ ] 6.3 Update `update` handler to dispatch: `if kind == SCHEMA_KIND { schema_service.update(...) } else { object_service.update(...) }`. Remove handler-level validation calls.
- [ ] 6.4 Update `delete` handler to dispatch: `if kind == SCHEMA_KIND { schema_service.delete(...) } else { object_service.delete(...) }`.
- [ ] 6.5 Update handler module doc comment to reflect new principle: handlers do parameter extraction and structural validation, not domain format validation.
- [ ] 6.6 Run `cargo check` and `cargo clippy` to verify compilation.

## 7. Extract SQLite Query Builder

- [ ] 7.1 Create a private `ListQueryBuilder` struct within `src/store/sqlite.rs` that encapsulates `where_clauses: Vec<String>`, `params: Vec<Box<dyn rusqlite::types::ToSql>>`, and `param_idx: usize`. Implement methods: `new(key)`, `add_continue_token(skip)`, `add_field_selector(selector)`, `add_label_selector(selector)`, `build() -> (String, Vec<...>)`. Add doc comments explaining the builder pattern and param_idx management.
- [ ] 7.2 Refactor `SQLiteStore::list()` to use `ListQueryBuilder` instead of inline SQL construction. Verify the generated SQL and params are identical.
- [ ] 7.3 Run `cargo check` and `cargo clippy` to verify compilation.

## 8. Verification

- [ ] 8.1 Run `cargo test` (unit tests) to verify all existing tests pass.
- [ ] 8.2 Run integration tests against both InMemory and SQLite stores to verify behavioral parity.
- [ ] 8.3 Run `cargo clippy -- -D warnings` to ensure no lint regressions.
- [ ] 8.4 Verify `ObjectService` line count is reduced (target: ~1200 lines from ~1885).

## 9. Documentation and Roadmap

- [ ] 9.1 Check `docs/` directory for any architecture documentation that references `ObjectService` managing Schema lifecycle. Update to reflect `SchemaService` extraction.
- [ ] 9.2 Check `roadmap.md` for any items impacted by this refactoring. Update or remove items that are now addressed. Add a note about the SchemaService extraction if not already present.
- [ ] 9.3 Update `AGENTS.md` architecture section if it references the old ObjectService responsibilities.
