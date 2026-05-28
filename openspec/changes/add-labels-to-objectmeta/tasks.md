## 1. Core Types and Error Handling

- [x] 1.1 Add `labels: HashMap<String, String>` field to `ObjectMeta` in `src/object/types.rs` with `#[serde(default)]` for deserialization
- [x] 1.2 Add `InvalidLabel(String)` variant to `AppError` enum in `src/error.rs` with HTTP 400 mapping
- [x] 1.3 Run `cargo check` to verify type changes compile

## 2. Label Validation

- [x] 2.1 Implement `validate_labels(labels: &HashMap<String, String>) -> Result<(), AppError>` function in `src/object/service.rs`
- [x] 2.2 Add validation logic for label keys: non-empty, max 256 chars, pattern `[a-zA-Z0-9][-_.a-zA-Z0-9]*` with optional `/` prefix separator
- [x] 2.3 Add validation logic for label values: max 256 chars, pattern `[a-zA-Z0-9][-_.a-zA-Z0-9]*` or empty string
- [x] 2.4 Add validation for prefix format (DNS subdomain, max 253 chars) when key contains `/`
- [x] 2.5 Write unit tests for `validate_labels()` covering valid keys, invalid keys, valid values, invalid values, prefixed keys, empty maps
- [x] 2.6 Run `cargo test` to verify validation logic

## 3. Handler Changes

- [x] 3.1 Update create handler in `src/object/handler.rs` to extract `labels` from `metadata.labels` for regular objects
- [x] 3.2 Update create handler to extract `labels` from `metadata.labels` for Schema objects
- [x] 3.3 Refactor handler to extract labels once, regardless of object kind (unify extraction logic)
- [x] 3.4 Handle case where `metadata.labels` is missing (default to empty HashMap)
- [x] 3.5 Handle case where `metadata.labels` is not an object type (return appropriate error)
- [x] 3.6 Update update handler to pass through labels in `StoredObject.metadata.labels` (no changes needed, just verify)
- [x] 3.7 Run `cargo check` to verify handler changes compile

## 4. Service Layer Integration

- [x] 4.1 Call `validate_labels()` in `ObjectService::validate_and_create_object()` before store persistence
- [x] 4.2 Call `validate_labels()` in `ObjectService::validate_and_create_schema()` before store persistence
- [x] 4.3 Call `validate_labels()` in `ObjectService::validate_and_update_object()` before store persistence
- [x] 4.4 Call `validate_labels()` in `ObjectService::validate_and_update_schema()` before store persistence
- [x] 4.5 Run `cargo check` to verify service integration compiles

## 5. InMemoryStore Implementation

- [x] 5.1 Verify `InMemoryStore::create()` stores labels as part of `ObjectMeta` (no changes needed, just verify)
- [x] 5.2 Verify `InMemoryStore::update()` replaces labels correctly (fixed: added `guard.metadata.labels = object.metadata.labels`)
- [x] 5.3 Verify `InMemoryStore::get()` and `list()` return labels correctly (no changes needed, just verify)
- [x] 5.4 Write integration tests for InMemoryStore with labels: create with labels, update labels, get with labels, list with labels

## 6. SQLiteStore Schema

- [x] 6.1 Add `CREATE TABLE IF NOT EXISTS labels` statement to `init_schema()` in `src/store/sqlite.rs`
- [x] 6.2 Add `CREATE INDEX IF NOT EXISTS idx_labels_gvkn` statement to `init_schema()`
- [x] 6.3 Verify schema initialization is idempotent (test on fresh and existing databases)
- [x] 6.4 Run `cargo check` to verify schema changes compile

## 7. SQLiteStore Create with Labels

- [x] 7.1 Update `SQLiteStore::create()` to insert label rows into `labels` table for each key-value pair in `ObjectMeta.labels`
- [x] 7.2 Wrap object insert and label inserts in a single transaction for atomicity
- [x] 7.3 Handle case where labels map is empty (no label inserts needed)
- [x] 7.4 Write integration tests for SQLiteStore create with labels

## 8. SQLiteStore Read with Labels

- [x] 8.1 Implement helper function to query labels from `labels` table for a given object
- [x] 8.2 Update `deserialize_row()` or `row_to_object()` to accept labels parameter and populate `ObjectMeta.labels`
- [x] 8.3 Update `SQLiteStore::get()` to query labels and reconstruct `ObjectMeta.labels`
- [x] 8.4 Update `SQLiteStore::list()` to query labels for each object and reconstruct `ObjectMeta.labels`
- [x] 8.5 Optimize label queries: batch fetch labels for multiple objects in `list()` to avoid N+1 queries
- [x] 8.6 Write integration tests for SQLiteStore get/list with labels

## 9. SQLiteStore Update with Labels

- [x] 9.1 Implement diff-based label update logic: read existing labels, compute diff (to_delete, to_upsert)
- [x] 9.2 Update `SQLiteStore::update()` to apply label diff (DELETE for removed keys, INSERT OR REPLACE for changed/new keys)
- [x] 9.3 Wrap object update and label diff operations in a single transaction
- [x] 9.4 Handle case where labels are unchanged (no label table writes)
- [x] 9.5 Write integration tests for SQLiteStore update with label changes (add, modify, remove labels)

## 10. SQLiteStore Delete with Labels

- [x] 10.1 Verify `ON DELETE CASCADE` automatically removes label rows when object is deleted
- [x] 10.2 Write integration test to confirm labels are deleted when object is deleted

## 11. Integration Tests

- [x] 11.1 Add integration test: create object with labels, verify labels in response
- [x] 11.2 Add integration test: create object without labels, verify empty labels map in response
- [x] 11.3 Add integration test: update object with changed labels, verify diff-based update
- [x] 11.4 Add integration test: create Schema with labels, verify labels persisted
- [x] 11.5 Add integration test: invalid label key format returns 400 error
- [x] 11.6 Add integration test: invalid label value format returns 400 error
- [x] 11.7 Add integration test: label key exceeds length limit returns 400 error
- [x] 11.8 Add integration test: label value exceeds length limit returns 400 error
- [x] 11.9 Run full integration test suite: `cargo test --package kapi-tests`

## 12. OpenAPI Spec Updates

- [x] 12.1 Update OpenAPI spec generation to include `labels` field in `ObjectMeta` schema
- [x] 12.2 Define `labels` as `type: object` with `additionalProperties: { type: string }`
- [x] 12.3 Verify generated OpenAPI spec includes labels in all endpoints that return objects
- [x] 12.4 Test OpenAPI spec generation: `cargo run --bin kapi -- --print-openapi` (or equivalent)

## 13. Swagger UI Updates

- [x] 13.1 Verify Swagger UI displays `labels` field in request body schemas
- [x] 13.2 Verify Swagger UI displays `labels` field in response schemas
- [x] 13.3 Test Swagger UI manually or via automated browser test

## 14. Documentation Review

- [x] 14.1 Review `docs/` directory for any documentation that describes `ObjectMeta` or object structure
- [x] 14.2 Update documentation to mention `labels` field and its purpose
- [x] 14.3 Add examples showing how to create objects with labels
- [x] 14.4 Document label validation rules (key format, value format, length limits)
- [x] 14.5 Check if any README files need updates

## 15. Roadmap Updates

- [x] 15.1 Review `roadmap.md` for items impacted by this change
- [x] 15.2 Mark "Label filtering" item as partially complete (labels on ObjectMeta done, selector pending)
- [x] 15.3 Add future work items from proposal: "Full label selector syntax parity (set-based operators: in, notin)"
- [x] 15.4 Add future work item: "Label indexing for high-cardinality label queries at scale"
- [x] 15.5 Add future work item: "Annotations (free-form key-value metadata without selection semantics)"

## 16. Final Verification

- [x] 16.1 Run `cargo clippy --all-targets --all-features` and fix any warnings
- [x] 16.2 Run `cargo fmt --check` and format code if needed
- [x] 16.3 Run full test suite: `cargo test --workspace`
- [x] 16.4 Manual smoke test: start server, create object with labels via curl, verify response
- [x] 16.5 Verify SQLite persistence: restart server, verify labels are still present after restart
