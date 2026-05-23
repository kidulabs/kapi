## 1. Core Type Changes

- [x] 1.1 In `src/object/types.rs`: Replace `ObjectMetadata` with `ObjectMeta` (fields: `name: String`) and `SystemMetadata` (fields: `resource_version: u64`, `created_at: DateTime<Utc>`, `updated_at: DateTime<Utc>`). Both derive `Debug`, `Clone`, `Serialize`, `Deserialize` with `#[serde(rename_all = "camelCase")]`. Remove `ObjectMetadata`.
- [x] 1.2 In `src/object/types.rs`: Update `StoredObject` to have four fields: `key: ResourceKey`, `metadata: ObjectMeta`, `system: SystemMetadata`, `data: UserData`. Remove the old `metadata: ObjectMetadata` field.
- [x] 1.3 Run `cargo check` to confirm type errors surface across all dependent files before proceeding to fixes.

## 2. Store Layer Updates

- [x] 2.1 In `src/store/mod.rs`: Update `ObjectStore::create` signature from `create(&self, key: &ResourceKey, name: &str, data: Value)` to `create(&self, key: &ResourceKey, meta: ObjectMeta, data: Value)`. Update the `use` import to include `ObjectMeta`.
- [x] 2.2 In `src/store/memory.rs`: Update `InMemoryStore::create` to accept `ObjectMeta` instead of `name: &str`. Assemble `SystemMetadata` internally (same as before: version from counter, timestamps from `now()`). Update all `metadata` field accesses to use `metadata.name` (unchanged path) and move `resource_version`/`created_at`/`updated_at` into `system` field construction. Update `list` continue token to use `metadata.name`. Update `update` to use `system.resource_version` for OCC check and `metadata.name` for key.
- [x] 2.3 In `src/store/sqlite.rs`: Same changes as memory store — update `create` to accept `ObjectMeta`, assemble `SystemMetadata` internally, update all field accesses from `.metadata.resource_version` to `.system.resource_version`, `.metadata.name` stays as `.metadata.name`, `.metadata.created_at`/`.updated_at` move to `.system.created_at`/`.system.updated_at`. Update `row_to_object` / `deserialize_row` to construct `ObjectMeta` and `SystemMetadata` separately. Update update query to use `object.system.resource_version` for OCC and `object.metadata.name` for key.
- [x] 2.4 Run `cargo check` to verify store layer compiles.

## 3. Service Layer Updates

- [x] 3.1 In `src/object/service.rs`: Update `create` method signature from `create(&self, key: ResourceKey, name: String, data: Value)` to `create(&self, key: ResourceKey, meta: ObjectMeta, data: Value)`. Update all callers within the service. Change `self.schema_cache.insert(name.clone(), compiled)` to `self.schema_cache.insert(meta.name.clone(), compiled)`. Update all `metadata.name` references (unchanged path) and `metadata.resource_version` → `system.resource_version` references.
- [x] 3.2 In `src/object/service.rs`: Update `delete_schema` method — field access changes from `metadata.name` (unchanged) and ensure `ListOptions` construction still works.
- [x] 3.3 In `src/object/service.rs`: Update all tests in the `mod tests` block to use `ObjectMeta` instead of bare name strings, and `SystemMetadata` / `system` field access where needed.
- [x] 3.4 Run `cargo check` to verify service layer compiles.

## 4. Handler Layer Updates

- [x] 4.1 In `src/object/handler.rs`: Update `create` handler to extract `ObjectMeta` from request body. For non-Schema objects: read `body["metadata"]["name"]`, construct `ObjectMeta { name }`, remove `metadata` key from body, call `service.create(key, meta, body)`. For Schema objects: construct `ObjectMeta { name: schema_name }` from extracted target fields, call `service.create(key, meta, data)`.
- [x] 4.2 In `src/object/handler.rs`: Update `update` handler — field access changes from `body.metadata.name` (unchanged path) and `body.metadata.resource_version` → `body.system.resource_version`. Update the validation that URL name matches body name to use `body.metadata.name`.
- [x] 4.3 Run `cargo check` to verify handler layer compiles.

## 5. OpenAPI Schema Updates

- [x] 5.1 In `src/openapi/components.rs`: Replace `ObjectMetadata` component schema with two schemas: `ObjectMeta` (properties: `name` type string, required) and `SystemMetadata` (properties: `resourceVersion` type integer format int64, `createdAt` type string format date-time, `updatedAt` type string format date-time; all required). Remove the old `ObjectMetadata` component.
- [x] 5.2 In `src/openapi/components.rs`: Update `StoredObject` component schema to have four properties: `key` (ref ResourceKey), `metadata` (ref ObjectMeta), `system` (ref SystemMetadata), `data` (ref UserData). Update `required` array to include all four.
- [x] 5.3 In `src/openapi/components.rs`: Update `build_kind_stored_object_component` to include `system` field referencing `SystemMetadata` alongside `metadata` referencing `ObjectMeta`.
- [x] 5.4 In `src/openapi/mod.rs`: Update any references from `ObjectMetadata` to `ObjectMeta` and add `SystemMetadata` to the static component list.
- [x] 5.5 Run `cargo check` and `cargo test` to verify OpenAPI generation compiles and existing OpenAPI tests pass.

## 6. Integration Test Updates

- [x] 6.1 In `tests/src/lib.rs`: Update the `widget` helper function — the JSON body uses `metadata.name` which is unchanged, but verify it still matches the new wire format.
- [x] 6.2 In `tests/src/object_crud.rs`: Update all JSON path assertions from `metadata.resourceVersion` → `system.resourceVersion`, `metadata.createdAt` → `system.createdAt`, `metadata.updatedAt` → `system.updatedAt`. Update update request bodies to use `system.resourceVersion` instead of `metadata.resourceVersion` and `system.createdAt`/`system.updatedAt` instead of `metadata.createdAt`/`metadata.updatedAt`.
- [x] 6.3 In `tests/src/optimistic_concurrency.rs`: Same changes as 6.2 — update all JSON paths and request bodies for the new `system` field structure.
- [x] 6.4 In `tests/src/watch_events.rs`: Update assertions from `metadata.name` (unchanged), `metadata.resourceVersion` → `system.resourceVersion`, `metadata.createdAt` → `system.createdAt`, `metadata.updatedAt` → `system.updatedAt`. Update the update request body to use `system.resourceVersion` for OCC.
- [x] 6.5 Run `cargo test` to verify all integration tests pass.

## 7. Documentation Updates

- [x] 7.1 In `docs/data-model.md`: Replace `ObjectMetadata` section with `ObjectMeta` and `SystemMetadata` sections. Update `StoredObject` struct to show four fields (`key`, `metadata`, `system`, `data`). Update wire format example to use `"metadata": { "name": "..." }` and `"system": { "resourceVersion": 42, "createdAt": "...", "updatedAt": "..." }`. Update the "Optimistic Concurrency" section to reference `system.resourceVersion` instead of `metadata.resourceVersion`.
- [x] 7.2 In `docs/api-reference.md`: Update all JSON examples showing `StoredObject` responses and request bodies. Move `resourceVersion`, `createdAt`, `updatedAt` from `metadata` into a `system` field in every example. Update the "Create an Object" request/response examples. Update the "Update an Object" request body example to use `system.resourceVersion`. Update the "Register a Schema" response example.
- [x] 7.3 In `docs/storage.md`: Update `ObjectStore` trait signature for `create` to show `meta: ObjectMeta` instead of `name: &str`. Update the design note about `update` to reference `object.system.resource_version` instead of `object.metadata.resource_version`. Update `InMemoryStore` and `SQLiteStore` descriptions if they reference `metadata.resource_version`.
- [x] 7.4 In `docs/architecture.md`: Update the module tree entry for `types.rs` to say `Core types (StoredObject, ObjectMeta, SystemMetadata, etc.)` instead of `ObjectMetadata`. Update the "Create Object" flow to show `ObjectService::create(key, meta, data)` instead of `ObjectService::create(key, name, data)`. Update the "Update Object" flow to reference `system.resourceVersion` instead of `metadata.resourceVersion`. Update the design decisions table — change "Wire format" row to mention `metadata` (user) + `system` (server) instead of just "camelCase metadata fields". Update "Optistic concurrency" row to reference `system.resourceVersion` instead of `metadata.resourceVersion`.

## 8. Roadmap and Final Verification

- [x] 8.1 Check if any items in `roadmap.md` are impacted by this change and update or remove them accordingly. (The "Label filtering" item should still remain pending — this change is a prerequisite, not the implementation.)
- [x] 8.2 Run `cargo clippy -- -D warnings` to ensure no warnings.
- [x] 8.3 Run `cargo doc --no-deps` to verify documentation builds without errors.
- [x] 8.4 Sync the delta specs to the main spec files using `/opsx-sync`.