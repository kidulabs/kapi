## 1. Core Types

- [ ] 1.1 Add `status: Option<SpecData>` field to `StoredObject` in `src/object/types.rs`
- [ ] 1.2 Add `StatusModified` variant to `WatchEventType` in `src/object/types.rs`
- [ ] 1.3 Add `status_schema: Option<serde_json::Value>` field to `SchemaData` in `src/object/types.rs` (with `#[serde(rename_all = "camelCase")]`)
- [ ] 1.4 Add `StatusSubresourceNotEnabled { kind: String }` variant to `AppError` in `src/error.rs` (maps to 404 Not Found)
- [ ] 1.5 Verify `cargo check` passes after types changes

## 2. Meta-Schema

- [ ] 2.1 Add `"statusSchema": { "type": "object" }` as optional property to `META_SCHEMA_JSON` in `src/schema/meta_schema.rs`
- [ ] 2.2 Update `SchemaData` deserialization tests to cover `status_schema` field
- [ ] 2.3 Verify meta-schema validation passes with and without `statusSchema`

## 3. Schema Registry

- [ ] 3.1 Add `get_status_validator(&self, key: &ResourceKey)` method to `SchemaRegistry` that returns `Result<Arc<dyn SchemaValidator>, AppError>` — on cache miss, fetches Schema from store, parses `status_schema`, compiles, caches under `{kind}.{group}.status`
- [ ] 3.2 Add `insert_status(&self, name: &str, validator: Arc<dyn SchemaValidator>)` method to cache status validators
- [ ] 3.3 Update `validate_and_compile` to also compile `status_schema` when present and return it alongside the spec validator
- [ ] 3.4 Update `evict` to also remove `{kind}.{group}.status` cache entry
- [ ] 3.5 Update `ObjectService::validate_and_create_schema` to cache status validator when `status_schema` is present
- [ ] 3.6 Update `ObjectService::validate_and_update_schema` to cache status validator when `status_schema` is present
- [ ] 3.7 Verify `cargo check` passes after registry changes

## 4. Store Layer

- [ ] 4.1 Add `async fn update_status(&self, key: &ResourceKey, name: &str, status: Value) -> Result<StoredObject, AppError>` to `ObjectStore` trait in `src/store/mod.rs`
- [ ] 4.2 Implement `update_status` in `InMemoryStore` — read object, replace `status` field, bump `resource_version`, set `updated_at`, return updated object
- [ ] 4.3 Implement `update_status` in `SQLiteStore` — UPDATE status column, resource_version, updated_at WHERE key/name (no CAS), return updated object
- [ ] 4.4 Add `status TEXT` column to SQLite `objects` table in `init_schema()`
- [ ] 4.5 Update `SQLiteStore::create` to insert `NULL` for status column
- [ ] 4.6 Update `SQLiteStore::get` and `SQLiteStore::list` to read status column
- [ ] 4.7 Update `SQLiteStore::update` to write status column
- [ ] 4.8 Update `SQLiteStore::deserialize_row` to handle status column
- [ ] 4.9 Verify `cargo check` passes after store changes

## 5. Service Layer

- [ ] 5.1 Add `update_status(&self, key: ResourceKey, name: String, status: Value) -> Result<StoredObject, AppError>` method to `ObjectService` — check statusSchema exists, validate status, call store, publish `StatusModified` event
- [ ] 5.2 Add `get_status(&self, key: ResourceKey, name: String) -> Result<Option<SpecData>, AppError>` method to `ObjectService` — check statusSchema exists, fetch object, return status field
- [ ] 5.3 Update `ObjectService::create` to strip `status` from request body before storing (status starts as `None`)
- [ ] 5.4 Verify `cargo check` passes after service changes

## 6. Handler Layer

- [ ] 6.1 Add `get_status` handler in `src/object/handler.rs` — extract path params, call `object_service.get_status()`, return status value as JSON
- [ ] 6.2 Add `update_status` handler in `src/object/handler.rs` — extract path params, deserialize body, extract `status` field, call `object_service.update_status()`, return full `StoredObject`
- [ ] 6.3 Add `/status` routes to `src/routes.rs` — `GET` and `PUT` on `/apis/{group}/{version}/{kind}/{name}/status`
- [ ] 6.4 Verify `cargo check` passes after handler changes

## 7. Unit Tests

- [ ] 7.1 Add tests for `StoredObject` serialization/deserialization with `status: Some(...)` and `status: None`
- [ ] 7.2 Add tests for `SchemaData` with and without `status_schema`
- [ ] 7.3 Add tests for `WatchEventType::StatusModified`
- [ ] 7.4 Add tests for `InMemoryStore::update_status` — success, not found, status replaces correctly, resource_version bumps, spec unchanged
- [ ] 7.5 Add tests for `SQLiteStore::update_status` — success, not found, status replaces correctly, resource_version bumps, spec unchanged
- [ ] 7.6 Add tests for `ObjectService::update_status` — with statusSchema, without statusSchema (error), invalid status (validation error), not found
- [ ] 7.7 Add tests for `ObjectService::get_status` — with statusSchema, without statusSchema (error)
- [ ] 7.8 Add tests for `ObjectService::create` — status field in body is ignored
- [ ] 7.9 Add tests for `SchemaRegistry::get_status_validator` — cache hit, cache miss, no statusSchema (error)
- [ ] 7.10 Add tests for meta-schema validation with and without `statusSchema`

## 8. Integration Tests

- [ ] 8.1 Add integration test: register Schema with `statusSchema`, create object, update status via `/status`, verify status is set
- [ ] 8.2 Add integration test: register Schema without `statusSchema`, attempt `/status` GET/PUT, verify 404 `StatusSubresourceNotEnabled`
- [ ] 8.3 Add integration test: update status with invalid data, verify 422 `SchemaValidation`
- [ ] 8.4 Add integration test: concurrent spec update and status update succeed without conflict
- [ ] 8.5 Add integration test: create object with status in body, verify status is null (ignored)

## 9. Documentation

- [ ] 9.1 Update `docs/architecture.md`: add status subresource to request flow diagrams, update StoredObject description to include `status: Option<SpecData>`, add status update flow diagram, update module tree if needed
- [ ] 9.2 Update `docs/data-model.md`: add `status: Option<SpecData>` to `StoredObject`, add `status_schema` to `SchemaData`, add `StatusModified` to `WatchEventType`, add `StatusSubresourceNotEnabled` to error model, update wire format example to show `status` field
- [ ] 9.3 Update `docs/api-reference.md`: add `GET/PUT /status` endpoint documentation, add `statusSchema` to Schema registration example, add `StatusSubresourceNotEnabled` error, update StoredObject JSON examples to include `status` field
- [ ] 9.4 Update `docs/storage.md`: add `update_status` method to `ObjectStore` trait documentation, add `status TEXT` column to SQLite schema, update `InMemoryStore` and `SQLiteStore` descriptions for status handling
- [ ] 9.5 Update `roadmap.md`: update the "Add Spec and Status" item to reflect status subresource is complete, add future work items for Schema object status and generation field

## 10. Verification

- [ ] 10.1 Run `cargo test` and ensure all unit tests pass
- [ ] 10.2 Run `cargo test -p kapi-tests` and ensure all integration tests pass
- [ ] 10.3 Run `cargo clippy` and ensure no warnings
- [ ] 10.4 Delete any existing SQLite database files and verify fresh creation works with status column
- [ ] 10.5 Check roadmap for items impacted by this change and update accordingly