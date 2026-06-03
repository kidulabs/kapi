## 1. Core Types

- [x] 1.1 Rename `UserData` struct to `SpecData` in `src/object/types.rs`
- [x] 1.2 Rename `StoredObject.data` field to `StoredObject.spec` in `src/object/types.rs`
- [x] 1.3 Update all `WatchEvent` test helpers and test fixtures in `src/object/types.rs` that reference `.data` to use `.spec`
- [x] 1.4 Verify `cargo check` passes after types changes

## 2. Store Layer

- [x] 2.1 Rename `data` parameter to `spec` in `ObjectStore::create()` trait method in `src/store/mod.rs`
- [x] 2.2 Update `InMemoryStore` in `src/store/memory.rs`: rename `data` parameter, `.data` → `.spec` field access, `UserData` → `SpecData` in struct literals, and all test fixtures
- [x] 2.3 Update `SQLiteStore` in `src/store/sqlite.rs`: rename SQL column `data` → `spec` in `init_schema()`, update all SQL queries, rename `data` parameter, `.data` → `.spec` field access, `UserData` → `SpecData` in struct literals, and all test fixtures
- [x] 2.4 Verify `cargo check` passes after store changes

## 3. Service Layer

- [x] 3.1 Update `src/object/service.rs`: rename `data` variables to `spec`, `.data` → `.spec` field access, `UserData` → `SpecData`, and all test fixtures
- [x] 3.2 Verify `cargo check` passes after service changes

## 4. Handler Layer

- [x] 4.1 Update `src/object/handler.rs`: rename `data` variables to `spec` where they refer to the object's spec payload (keep `schema_data` as-is since it refers to `SchemaData`)
- [x] 4.2 Verify `cargo check` passes after handler changes

## 5. Schema Layer

- [x] 5.1 Update `src/schema/registry.rs`: rename `data` variables to `spec` where they refer to the object's spec payload (keep `schema_data` as-is)
- [x] 5.2 Verify `cargo check` passes after schema changes

## 6. OpenAPI

- [x] 6.1 Update `src/openapi/components.rs`: rename `UserData` → `SpecData`, `"data"` → `"spec"` in JSON schemas, `build_kind_data_component` → `build_kind_spec_component`
- [x] 6.2 Update `src/openapi/paths.rs`: rename `.data.value` → `.spec.value` access, update comments referencing "data"
- [x] 6.3 Update `src/openapi/mod.rs`: update test assertions from `"data"` → `"spec"` key
- [x] 6.4 Verify `cargo check` passes after OpenAPI changes

## 7. Integration Tests

- [x] 7.1 Update `tests/src/object_crud.rs`: rename `"data"` → `"spec"` in JSON request/response payloads
- [x] 7.2 Update `tests/src/optimistic_concurrency.rs`: rename `"data"` → `"spec"` in JSON request/response payloads
- [x] 7.3 Update `tests/src/watch_events.rs`: rename `"data"` → `"spec"` in JSON payloads
- [x] 7.4 Update `tests/src/lib.rs`: update any `data` field references in test helpers

## 8. Documentation

- [x] 8.1 Update `docs/architecture.md`: rename `data` → `spec` in request flow diagrams, StoredObject description, and all references
- [x] 8.2 Update `docs/data-model.md`: rename `UserData` → `SpecData`, `data` → `spec` in StoredObject, wire format example, and all type definitions
- [x] 8.3 Update `docs/api-reference.md`: rename `"data"` → `"spec"` in JSON examples, request/response descriptions, and all references
- [x] 8.4 Update `docs/storage.md`: rename `data` → `spec` in ObjectStore trait signature, SQLite column description, and all references
- [x] 8.5 Update `roadmap.md`: update the "Add Spec and Status" item to reflect the rename is complete

## 9. Verification

- [x] 9.1 Run `cargo test` and ensure all unit tests pass
- [x] 9.2 Run `cargo test -p kapi-tests` and ensure all integration tests pass
- [x] 9.3 Run `cargo clippy` and ensure no warnings
- [x] 9.4 Delete any existing SQLite database files and verify fresh creation works
- [x] 9.5 Check roadmap in `openspec/specs/roadmap-update/` for any items referencing `data` or `UserData` and update them