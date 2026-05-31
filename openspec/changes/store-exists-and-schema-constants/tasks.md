## 1. Add exists() to ObjectStore trait

- [ ] 1.1 Add `async fn exists(&self, key: &ResourceKey) -> Result<bool, AppError>` to `ObjectStore` trait in `src/store/mod.rs`
- [ ] 1.2 Implement `exists()` for `InMemoryStore` in `src/store/memory.rs` — iterate DashMap entries, check if any match the key
- [ ] 1.3 Implement `exists()` for `SQLiteStore` in `src/store/sqlite.rs` — use `SELECT EXISTS(SELECT 1 FROM objects WHERE ...)` query via `spawn_blocking`
- [ ] 1.4 Add unit tests for `InMemoryStore::exists()` — test with objects present and absent
- [ ] 1.5 Add unit tests for `SQLiteStore::exists()` — test with objects present and absent
- [ ] 1.6 Run `cargo check` to verify trait and implementations compile

## 2. Define schema constants

- [ ] 2.1 Add `SCHEMA_KIND`, `SCHEMA_GROUP`, `SCHEMA_VERSION` constants and `schema_key()` helper function to `src/schema/mod.rs`
- [ ] 2.2 Run `cargo check` to verify constants compile

## 3. Replace magic strings

- [ ] 3.1 Replace `"Schema"` with `SCHEMA_KIND` in `src/object/service.rs` (3 logic occurrences + 13 test occurrences)
- [ ] 3.2 Replace `ResourceKey { group: "kapi.io", version: "v1", kind: "Schema" }` constructions with `schema_key()` in `src/object/service.rs`
- [ ] 3.3 Replace `"Schema"` with `SCHEMA_KIND` in `src/schema/registry.rs` (1 logic + 2 test occurrences)
- [ ] 3.4 Replace `ResourceKey { group: "kapi.io", version: "v1", kind: "Schema" }` constructions with `schema_key()` in `src/schema/registry.rs`
- [ ] 3.5 Replace `"Schema"` with `SCHEMA_KIND` in `src/object/handler.rs` (1 logic occurrence + update comment)
- [ ] 3.6 Replace `"Schema"` with `SCHEMA_KIND` in `src/openapi/paths.rs` (1 occurrence)
- [ ] 3.7 Replace `"Schema"` with `SCHEMA_KIND` in `src/openapi/mod.rs` (2 test occurrences)
- [ ] 3.8 Replace `"Schema"` with `SCHEMA_KIND` in `src/event/bus.rs` (1 test occurrence)
- [ ] 3.9 Run `cargo check` to verify all replacements compile

## 4. Update SchemaHasObjects error

- [ ] 4.1 Remove `count` field from `AppError::SchemaHasObjects` variant in `src/error.rs`
- [ ] 4.2 Update error Display implementation for `SchemaHasObjects` to not reference count
- [ ] 4.3 Run `cargo check` to identify all call sites that need updating

## 5. Update deletion guard to use exists()

- [ ] 5.1 Refactor `delete_schema()` in `src/object/service.rs` to use `store.exists(&target_key)` instead of `store.list()` with limit 1 and full list
- [ ] 5.2 Update `delete_schema()` to use `SchemaHasObjects { kind }` without count field
- [ ] 5.3 Update test `delete_schema_with_objects_returns_conflict` to match new error shape (no count assertion)
- [ ] 5.4 Run `cargo check` to verify deletion guard compiles

## 6. Verification

- [ ] 6.1 Run `cargo test` to verify all existing tests pass
- [ ] 6.2 Run `cargo clippy` and fix any warnings
- [ ] 6.3 Run integration tests via `cargo test -p kapi-tests` to verify both InMemory and SQLite store paths work
- [ ] 6.4 Check `openspec/specs/` for any documentation that references the old `SchemaHasObjects { kind, count }` shape and update if needed
- [ ] 6.5 Check roadmap in `openspec/specs/roadmap-update/` for any items impacted by this change
