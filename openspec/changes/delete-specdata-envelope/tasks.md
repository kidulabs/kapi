## 1. Update Core Types

- [ ] 1.1 In `src/object/types.rs`: remove the `SpecData` struct definition (currently lines 114-117)
- [ ] 1.2 In `src/object/types.rs`: change `StoredObject.spec` from `SpecData` to `serde_json::Value`
- [ ] 1.3 In `src/object/types.rs`: change `StoredObject.status` from `Option<SpecData>` to `Option<serde_json::Value>`
- [ ] 1.4 In `src/object/types.rs`: update the `test_stored_object` helper to take `spec: serde_json::Value` and store it directly in `StoredObject.spec`
- [ ] 1.5 In `src/object/types.rs`: update all unit-test fixtures that construct `StoredObject` with `spec: SpecData { value: ... }` to use `spec: ...` directly
- [ ] 1.6 In `src/object/types.rs`: add `use serde_json::Value;` to the imports if not already present

## 2. Update ObjectService

- [ ] 2.1 In `src/object/service.rs`: remove `SpecData` from the import list at the top of the file
- [ ] 2.2 In `src/object/service.rs`: update `validate_and_create_schema` to construct `StoredObject { spec, ... }` instead of `StoredObject { spec: SpecData { value: spec }, ... }`
- [ ] 2.3 In `src/object/service.rs`: update `validate_and_create_object` similarly
- [ ] 2.4 In `src/object/service.rs`: update `validate_and_update_schema` to pass `Value` directly (no `.value` unwrap on the incoming object)
- [ ] 2.5 In `src/object/service.rs`: update `validate_and_update_object` similarly
- [ ] 2.6 In `src/object/service.rs`: in `apply_with_metadata`, change `new_obj.spec.value != existing.spec.value` to `new_obj.spec != existing.spec`
- [ ] 2.7 In `src/object/service.rs`: in `update_status`, update the `updated.status = Some(SpecData { value: status })` to `updated.status = Some(status)`
- [ ] 2.8 In `src/object/service.rs`: change `get_status` return type from `Result<Option<SpecData>, AppError>` to `Result<Option<Value>, AppError>`
- [ ] 2.9 In `src/object/service.rs`: update all unit-test fixtures and assertions that use `SpecData` or `.spec.value` / `.status.unwrap().value`

## 3. Update Handlers

- [ ] 3.1 In `src/object/handler.rs`: remove `SpecData` from the import list
- [ ] 3.2 In `src/object/handler.rs`: change the `get_status` handler return type from `Result<Json<Option<SpecData>>, AppError>` to `Result<Json<Option<Value>>, AppError>`
- [ ] 3.3 In `src/object/handler.rs`: verify that the `create` handler body extraction is unchanged (it was already treating the request body as a `Value`)

## 4. Update InMemoryStore

- [ ] 4.1 In `src/store/memory.rs`: remove `SpecData` from the import list in the test module
- [ ] 4.2 In `src/store/memory.rs`: update test fixtures that construct `StoredObject` to use the unwrapped `spec: Value` form
- [ ] 4.3 In `src/store/memory.rs`: verify that store operations (insert, get, list) work on the `Value` directly without envelope handling — they should, since the store operates on `StoredObject` as a whole

## 5. Update SQLiteStore

- [ ] 5.1 In `src/store/sqlite.rs`: remove `SpecData` from the import list
- [ ] 5.2 In `src/store/sqlite.rs`: update the row-to-object conversion to read the `spec` column as `serde_json::Value` directly (no `SpecData { value: ... }` wrapping on read)
- [ ] 5.3 In `src/store/sqlite.rs`: update the object-to-row conversion to serialize `object.spec` and `object.status` as JSON strings directly (no `.value` access on serialize)
- [ ] 5.4 In `src/store/sqlite.rs`: update test fixtures that construct `StoredObject` to use the unwrapped `spec: Value` form
- [ ] 5.5 In `src/store/sqlite.rs`: update test assertions that use `.spec.value` or `.status.unwrap().value`
- [ ] 5.6 Verify SQLite column types and the `init_schema` SQL are unchanged (column names and types are `TEXT` storing JSON; no schema migration needed)

## 6. Update SchemaRegistry

- [ ] 6.1 In `src/schema/registry.rs`: remove `SpecData` from any references
- [ ] 6.2 In `src/schema/registry.rs`: in `get_or_compile`, change `serde_json::from_value(schema_obj.spec.value)` to `serde_json::from_value(schema_obj.spec.clone())`
- [ ] 6.3 In `src/schema/registry.rs`: in any other site that reads `schema_obj.spec.value`, change to read `schema_obj.spec` directly
- [ ] 6.4 In `src/schema/registry.rs`: update test fixtures that construct `StoredObject` to use the unwrapped `spec: Value` form

## 7. Update EventBus Tests

- [ ] 7.1 In `src/event/bus.rs`: remove `SpecData` from the import list in the test module
- [ ] 7.2 In `src/event/bus.rs`: update test fixtures that construct `StoredObject` to use the unwrapped `spec: Value` form

## 8. Update OpenAPI Components

- [ ] 8.1 In `src/openapi/components.rs`: remove the `SpecData` component entry (lines 88-98)
- [ ] 8.2 In `src/openapi/components.rs`: in the `StoredObject` component, change `"spec": { "$ref": "#/components/schemas/SpecData" }` to `"spec": { "description": "User-defined spec payload, validated against the kind's registered jsonSchema" }`
- [ ] 8.3 In `src/openapi/components.rs`: in the `StoredObject` component, change `"status": { "$ref": "#/components/schemas/SpecData", "nullable": true, "description": "..." }` to `"status": { "nullable": true, "description": "Status subresource, managed via /status endpoint. Null for kinds without statusSchema." }`
- [ ] 8.4 In `src/openapi/components.rs`: in `build_kind_spec_component`, change the output schema from `{ "type": "object", "properties": { "value": <userSchema> }, "required": ["value"] }` to `<userSchema>` directly. The kind-specific spec component (e.g. `WidgetExampleIo`) is now the user's specSchema, not a wrapper containing it.

## 9. Update OpenAPI Paths

- [ ] 9.1 In `src/openapi/paths.rs`: replace `let spec_data_ref = json!({ "$ref": "#/components/schemas/SpecData" });` with direct unconstrained JSON: `let status_ref = json!({ "nullable": true, "description": "Status subresource, or null if not set" });` (the `spec_data_ref` was only used in the status GET response, which now returns `Option<Value>` directly)
- [ ] 9.2 In `src/openapi/paths.rs`: update the `GET /apis/.../{name}/status` response schema to use the new unconstrained status shape (not wrapped, not $ref'd to a SpecData component)
- [ ] 9.3 In `src/openapi/paths.rs`: verify the `build_create_request_schema` function is unchanged (it already uses `schema_data.spec_schema` directly without a `value` wrapper, so the create request swagger display is already correct)
- [ ] 9.4 In `src/openapi/paths.rs`: verify the `build_status_update_request_schema` function is unchanged (it also uses the user's status schema directly)

## 10. Update OpenAPI Module Tests and Static-Component List

- [ ] 10.1 In `src/openapi/mod.rs`: remove `SpecData` from the static component list (line 79)
- [ ] 10.2 In `src/openapi/mod.rs`: update the `build_static_components_contains_all_twelve` test — rename to `build_static_components_contains_all_eleven` and remove `"SpecData"` from the expected array. The total count drops from 12 to 11.
- [ ] 10.3 In `src/openapi/mod.rs`: update the `build_kind_spec_component_wraps_user_schema` test — rename to `build_kind_spec_component_uses_user_schema_directly` and change the assertions to check that `color` and `size` are top-level properties on the schema, not nested under a `value` key.
- [ ] 10.4 In `src/openapi/mod.rs`: verify that `build_kind_stored_object_component_has_correct_refs` and other dynamic-component tests still pass (they should — they only check `$ref` and `required` fields, not the spec shape)

## 10a. Verify Swagger UI Coherence

- [ ] 10a.1 Manually inspect the generated OpenAPI spec at `/openapi` after the change: confirm there is no `SpecData` component anywhere in `#/components/schemas/`
- [ ] 10a.2 Manually inspect `GET /swagger-ui/` in a browser: confirm the schemas expand without any `value` indirection — for `WidgetExampleIo` the user sees `color` and `size` as top-level properties, not nested under a `value` key
- [ ] 10a.3 Manually inspect the per-kind `GET /apis/.../{name}` response in swagger UI: the `spec` property should expand to show the user's fields directly (e.g. `color`, `size`), not `{value: {color, size}}`
- [ ] 10a.4 Manually inspect the `GET /apis/.../{name}/status` response in swagger UI: the response should be the status object directly (e.g. `{phase, message}`), not wrapped in `{value: {...}}`

## 11. Update Integration Tests

- [ ] 11.1 In `tests/src/object_crud.rs`: change `"spec": { "value": { ... } }` to `"spec": { ... }` in request bodies, change `["spec"]["value"]["x"]` to `["spec"]["x"]` in assertions
- [ ] 11.2 In `tests/src/status_subresource.rs`: change `"status": { "value": { ... } }` to `"status": { ... }` in response assertions, change `["status"]["value"]["x"]` to `["status"]["x"]` (request bodies were already unwrapped, no change there)
- [ ] 11.3 In `tests/src/watch_events.rs`: change `"spec": { "value": { ... } }` to `"spec": { ... }` in request bodies, change `["spec"]["value"]["x"]` to `["spec"]["x"]` in assertions
- [ ] 11.4 In `tests/src/optimistic_concurrency.rs`: change `"spec": { "value": { ... } }` to `"spec": { ... }` in request bodies
- [ ] 11.5 In `tests/src/generation_semantics.rs`: change `"spec": { "value": { ... } }` to `"spec": { ... }` in request bodies
- [ ] 11.6 In `tests/src/watch_events.rs`: remove `SpecData` from the import list (line 295) and update the `StoredObject` construction at line 331 to use the unwrapped form

## 12. Update Documentation

- [ ] 12.1 In `docs/data-model.md`: remove the "### SpecData" section (lines 69-77)
- [ ] 12.2 In `docs/data-model.md`: update the `StoredObject` struct example to show `spec: serde_json::Value, status: Option<serde_json::Value>`
- [ ] 12.3 In `docs/data-model.md`: update the wire-format description to reflect the new shape (no `value` wrapper)
- [ ] 12.4 In `docs/data-model.md`: update the "Wire Format Example" (lines 222-253) — change `"status": { "value": { "phase": "Running", "availableReplicas": 3 } }` to `"status": { "phase": "Running", "availableReplicas": 3 }`. Note: the existing example is internally inconsistent — `spec` shows unwrapped (line 243-245) but `status` shows wrapped (line 246-251). After this fix both are unwrapped, which is the correct shape. The "spec": {...} part stays the same.
- [ ] 12.5 In `docs/api-reference.md`: update the `GET /status` response description to reflect that status is returned as an inline JSON value, not wrapped in `{value: ...}`
- [ ] 12.6 In `docs/api-reference.md`: update the GET /status example response body (lines 408-415) — change `{ "value": { "phase": "Running", "message": "All systems go" } }` to `{ "phase": "Running", "message": "All systems go" }`
- [ ] 12.7 In `docs/api-reference.md`: update the PUT /status example response body (lines 451-452) — change `"spec": { "value": { "color": "blue", "size": 10 } }` to `"spec": { "color": "blue", "size": 10 }`, and change `"status": { "value": { "phase": "Running", "message": "All systems go" } }` to `"status": { "phase": "Running", "message": "All systems go" }`
- [ ] 12.8 In `docs/api-reference.md`: line 208 currently says "Full StoredObject with updated `spec.value`" — change to "Full StoredObject with updated `spec`" (the example body below it was already correct)
- [ ] 12.9 In `docs/testprompt.md`: fix PUT request bodies that use the wrapped form. Update each `"spec":{"value":{...}}` to `"spec":{...}` at the following lines:
  - 140 (Test 4: watch lifecycle update)
  - 351 (Test 7: label update)
  - 590 (Test 14: persistence update)
  - 1519 (Test 39: spec update Modified event)
  - 1596 (Test 41: metadata-only generation)
  - 1644 (Test 42: spec change generation)
  - 1740 (Test 44 step 1: labels update)
  - 1761 (Test 44 step 2: spec update)
  - 1793 (Test 44 step 4: labels update again)
- [ ] 12.10 In `docs/testprompt.md`: fix python assertions that read `.value` from spec/status. Update each:
  - Line 1227 (Test 30 expected results): change `{"value":{"phase":"Running","message":"All systems go"}}` to `{"phase":"Running","message":"All systems go"}`
  - Line 1339 (Test 34): change `spec = obj['spec']['value']` to `spec = obj['spec']`
  - Line 1352 (Test 34 expected): change `status is {"value":{"phase":"Running"}}` to `status is {"phase":"Running"}`
  - Line 1402 (Test 36 assertion): remove the `status.get('value') is None` clause — the new shape is `status is None or status == 'null'`
  - Line 1439 (Test 37): change `status = json.load(sys.stdin)['value']` to `status = json.load(sys.stdin)` (the GET /status endpoint now returns the status directly)
  - Line 1708 (Test 43 expected): change `Status set to {"value":{"phase":"Running"}}` to `Status set to {"phase":"Running"}`
- [ ] 12.11 In `roadmap.md`: remove the line "- [ ] Should we rename the struct SpecData to UserData?" (line 29)
- [ ] 12.12 After updating testprompt.md, run through each affected test mentally to confirm the assertion still passes against the new wire shape. Note: most assertions are *positional* (e.g. `spec['color'] == 'blue'`) and don't depend on the wrapper, so the only edits needed are the ones listed above. Do not introduce logic changes.

**Note on terminology**: The change only deletes the `SpecData` envelope around `spec`/`status` payloads. The *error response envelope* (`{"error": ..., "code": ..., "details": ...}` described in `docs/data-model.md:155` as a "standard JSON envelope") is a separate, unrelated concept and is preserved unchanged. When editing docs, be careful not to confuse the two — both use the word "envelope" but only the SpecData one is being removed.

## 13. Final Verification

- [ ] 13.1 Run `cargo build` and confirm zero compilation errors
- [ ] 13.2 Run `cargo test` and confirm all tests pass (both unit tests in `src/` and integration tests in `tests/`)
- [ ] 13.3 Run `cargo test --package kapi-tests` to confirm the integration test binary passes against both InMemory and SQLite stores
- [ ] 13.4 Run `cargo clippy --all-targets --all-features -- -D warnings` and fix any lints
- [ ] 13.5 Run `grep -r "SpecData" src/ tests/ docs/ roadmap.md openspec/specs/ openspec/changes/delete-specdata-envelope/` and confirm zero matches outside of the change's own artifacts
- [ ] 13.6 Run `grep -r '\.spec\.value\|\["spec"\]\["value"\]\|"spec": { "value"' src/ tests/ docs/` and confirm zero matches (no leftover envelope accesses in Rust code, in test request/response bodies, or in the test prompt docs)
- [ ] 13.7 Run `grep -r '"value":\s*{' src/openapi/` and confirm zero matches (no OpenAPI component wraps a `value` property)
- [ ] 13.8 Verify `GET /openapi` returns a valid spec without a `SpecData` component and with `spec`/`status` as unconstrained JSON
- [ ] 13.9 Verify `GET /openapi` after registering a kind: confirm the kind-specific spec component (e.g. `WidgetExampleIo`) is the user's specSchema directly, not wrapped in `{value: <userSchema>}`
- [ ] 13.10 Review the change against the proposal: confirm no behavior changes, only structural refactor
