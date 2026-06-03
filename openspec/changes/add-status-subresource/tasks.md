## 1. Core Types

- [x] 1.1 Add `status: Option<SpecData>` field to `StoredObject` in `src/object/types.rs`
- [x] 1.2 Add `StatusModified` variant to `WatchEventType` in `src/object/types.rs`
- [x] 1.3 Add `status_schema: Option<serde_json::Value>` field to `SchemaData` in `src/object/types.rs` (with `#[serde(rename_all = "camelCase")]`)
- [x] 1.4 Add `StatusSubresourceNotEnabled { kind: String }` variant to `AppError` in `src/error.rs` (maps to 404 Not Found)
- [x] 1.5 Verify `cargo check` passes after types changes

## 2. Meta-Schema

- [x] 2.1 Add `"statusSchema": { "type": "object" }` as optional property to `META_SCHEMA_JSON` in `src/schema/meta_schema.rs`
- [x] 2.2 Update `SchemaData` deserialization tests to cover `status_schema` field
- [x] 2.3 Verify meta-schema validation passes with and without `statusSchema`

## 3. Schema Registry

- [x] 3.1 Add `get_status_validator(&self, key: &ResourceKey)` method to `SchemaRegistry` that returns `Result<Arc<dyn SchemaValidator>, AppError>` — on cache miss, fetches Schema from store, parses `status_schema`, compiles, caches under `{kind}.{group}.status`
- [x] 3.2 Add `insert_status(&self, name: &str, validator: Arc<dyn SchemaValidator>)` method to cache status validators
- [x] 3.3 Update `validate_and_compile` to also compile `status_schema` when present and return it alongside the spec validator
- [x] 3.4 Update `evict` to also remove `{kind}.{group}.status` cache entry
- [x] 3.5 Update `ObjectService::validate_and_create_schema` to cache status validator when `status_schema` is present
- [x] 3.6 Update `ObjectService::validate_and_update_schema` to cache status validator when `status_schema` is present
- [x] 3.7 Verify `cargo check` passes after registry changes

## 4. Store Layer

- [x] 4.1 Add `async fn update_status(&self, key: &ResourceKey, name: &str, status: Value) -> Result<StoredObject, AppError>` to `ObjectStore` trait in `src/store/mod.rs`
- [x] 4.2 Implement `update_status` in `InMemoryStore` — read object, replace `status` field, bump `resource_version`, set `updated_at`, return updated object
- [x] 4.3 Implement `update_status` in `SQLiteStore` — UPDATE status column, resource_version, updated_at WHERE key/name (no CAS), return updated object
- [x] 4.4 Add `status TEXT` column to SQLite `objects` table in `init_schema()`
- [x] 4.5 Update `SQLiteStore::create` to insert `NULL` for status column
- [x] 4.6 Update `SQLiteStore::get` and `SQLiteStore::list` to read status column
- [x] 4.7 Update `SQLiteStore::update` to write status column
- [x] 4.8 Update `SQLiteStore::deserialize_row` to handle status column
- [x] 4.9 Verify `cargo check` passes after store changes

## 5. Service Layer

- [x] 5.1 Add `update_status(&self, key: ResourceKey, name: String, status: Value) -> Result<StoredObject, AppError>` method to `ObjectService` — check statusSchema exists, validate status, call store, publish `StatusModified` event
- [x] 5.2 Add `get_status(&self, key: ResourceKey, name: String) -> Result<Option<SpecData>, AppError>` method to `ObjectService` — check statusSchema exists, fetch object, return status field
- [x] 5.3 Update `ObjectService::create` to strip `status` from request body before storing (status starts as `None`)
- [x] 5.4 Verify `cargo check` passes after service changes

## 6. Handler Layer

- [x] 6.1 Add `get_status` handler in `src/object/handler.rs` — extract path params, call `object_service.get_status()`, return status value as JSON
- [x] 6.2 Add `update_status` handler in `src/object/handler.rs` — extract path params, deserialize body, extract `status` field, call `object_service.update_status()`, return full `StoredObject`
- [x] 6.3 Add `/status` routes to `src/routes.rs` — `GET` and `PUT` on `/apis/{group}/{version}/{kind}/{name}/status`
- [x] 6.4 Verify `cargo check` passes after handler changes

## 7. Unit Tests

- [x] 7.1 Add tests for `StoredObject` serialization/deserialization with `status: Some(...)` and `status: None`
- [x] 7.2 Add tests for `SchemaData` with and without `status_schema`
- [x] 7.3 Add tests for `WatchEventType::StatusModified`
- [x] 7.4 Add tests for `InMemoryStore::update_status` — success, not found, status replaces correctly, resource_version bumps, spec unchanged
- [x] 7.5 Add tests for `SQLiteStore::update_status` — success, not found, status replaces correctly, resource_version bumps, spec unchanged
- [x] 7.6 Add tests for `ObjectService::update_status` — with statusSchema, without statusSchema (error), invalid status (validation error), not found
- [x] 7.7 Add tests for `ObjectService::get_status` — with statusSchema, without statusSchema (error)
- [x] 7.8 Add tests for `ObjectService::create` — status field in body is ignored
- [x] 7.9 Add tests for `SchemaRegistry::get_status_validator` — cache hit, cache miss, no statusSchema (error)
- [x] 7.10 Add tests for meta-schema validation with and without `statusSchema`

## 8. Integration Tests

- [x] 8.1 Add integration test: register Schema with `statusSchema`, create object, update status via `/status`, verify status is set
- [x] 8.2 Add integration test: register Schema without `statusSchema`, attempt `/status` GET/PUT, verify 404 `StatusSubresourceNotEnabled`
- [x] 8.3 Add integration test: update status with invalid data, verify 422 `SchemaValidation`
- [x] 8.4 Add integration test: concurrent spec update and status update succeed without conflict
- [x] 8.5 Add integration test: create object with status in body, verify status is null (ignored)

## 9. Documentation

- [x] 9.1 Update `docs/architecture.md`: add status subresource to request flow diagrams, update StoredObject description to include `status: Option<SpecData>`, add status update flow diagram, update module tree if needed
- [x] 9.2 Update `docs/data-model.md`: add `status: Option<SpecData>` to `StoredObject`, add `status_schema` to `SchemaData`, add `StatusModified` to `WatchEventType`, add `StatusSubresourceNotEnabled` to error model, update wire format example to show `status` field
- [x] 9.3 Update `docs/api-reference.md`: add `GET/PUT /status` endpoint documentation, add `statusSchema` to Schema registration example, add `StatusSubresourceNotEnabled` error, update StoredObject JSON examples to include `status` field
- [x] 9.4 Update `docs/storage.md`: add `update_status` method to `ObjectStore` trait documentation, add `status TEXT` column to SQLite schema, update `InMemoryStore` and `SQLiteStore` descriptions for status handling
- [x] 9.5 Update `roadmap.md`: update the "Add Spec and Status" item to reflect status subresource is complete, add future work items for Schema object status and generation field

## 10. Verification

- [x] 10.1 Run `cargo test` and ensure all unit tests pass
- [x] 10.2 Run `cargo test -p kapi-tests` and ensure all integration tests pass
- [x] 10.3 Run `cargo clippy` and ensure no warnings
- [x] 10.4 Delete any existing SQLite database files and verify fresh creation works with status column
- [x] 10.5 Check roadmap for items impacted by this change and update accordingly