## 1. Core Types and Error Handling

- [ ] 1.1 Add `annotations: HashMap<String, String>` field to `ObjectMeta` in `src/object/types.rs` with `#[serde(default)]` attribute
- [ ] 1.2 Add `InvalidAnnotation(String)` variant to `AppError` enum in `src/error.rs` with HTTP 400 mapping
- [ ] 1.3 Update `test_stored_object` helper in `src/object/types.rs` to initialize empty annotations map
- [ ] 1.4 Run `cargo check` to verify type changes compile

## 2. Handler Layer

- [ ] 2.1 Add `extract_annotations()` function in `src/object/handler.rs` mirroring `extract_labels()` pattern — extract from `metadata.annotations`, return empty HashMap when absent, error on non-object or non-string values
- [ ] 2.2 Update `create` handler to call `extract_annotations()` and pass annotations to `ObjectMeta` construction for both Schema and regular object paths
- [ ] 2.3 Update `update` handler to call `extract_annotations()` and pass annotations to `ObjectMeta` construction
- [ ] 2.4 Run `cargo check` to verify handler changes compile

## 3. Service Layer Validation

- [ ] 3.1 Add `validate_annotation_key()` function in `src/object/service.rs` — check non-empty, max 256 chars, no character restrictions
- [ ] 3.2 Add `validate_annotations()` function in `src/object/service.rs` — iterate all annotations, validate keys, compute total serialized size, check 256KB limit
- [ ] 3.3 Call `validate_annotations()` in `ObjectService::create()` after label validation
- [ ] 3.4 Call `validate_annotations()` in `ObjectService::update()` after label validation
- [ ] 3.5 Add unit tests for `validate_annotations()` covering valid cases, empty keys, long keys, size limit exceeded
- [ ] 3.6 Run `cargo test` to verify validation logic

## 4. SQLite Storage

- [ ] 4.1 Add `annotations TEXT` column to `objects` table schema in `src/store/sqlite.rs` `init()` method
- [ ] 4.2 Update `deserialize_row()` to accept annotations parameter and deserialize from JSON to `HashMap<String, String>`, defaulting to empty map on NULL
- [ ] 4.3 Update `create_object()` to serialize annotations to JSON and insert into annotations column
- [ ] 4.4 Update `update_object()` to serialize annotations to JSON and update annotations column
- [ ] 4.5 Update `get_object()` to pass annotations column value to `deserialize_row()`
- [ ] 4.6 Update `list_objects()` to pass annotations column value to `deserialize_row()` for all returned objects
- [ ] 4.7 Update `delete_object()` — no changes needed (annotations column deleted with row)
- [ ] 4.8 Run `cargo test` to verify SQLite storage changes

## 5. InMemoryStore

- [ ] 5.1 Verify InMemoryStore requires no changes (annotations stored in `ObjectMeta` directly)
- [ ] 5.2 Run existing InMemoryStore tests to confirm no regressions

## 6. OpenAPI Specification

- [ ] 6.1 Update `ObjectMeta` component in `src/openapi/components.rs` to include `annotations` field with type `object`, additionalProperties `string`, default `{}`
- [ ] 6.2 Update create endpoint documentation in `src/openapi/paths.rs` to mention `metadata.annotations`
- [ ] 6.3 Update update endpoint documentation in `src/openapi/paths.rs` to mention `metadata.annotations`
- [ ] 6.4 Run `cargo check` to verify OpenAPI changes compile

## 7. Integration Tests

- [ ] 7.1 Create `tests/src/object_annotations.rs` test module mirroring `object_labels.rs` structure
- [ ] 7.2 Add `test_create_object_with_annotations` — create object with annotations, verify in response and GET
- [ ] 7.3 Add `test_create_object_without_annotations` — create object without annotations, verify empty map in response
- [ ] 7.4 Add `test_update_object_annotations` — update object with new annotations, verify changes persist
- [ ] 7.5 Add `test_create_schema_with_annotations` — create Schema with annotations, verify in response and GET
- [ ] 7.6 Add `test_invalid_annotation_key_empty` — verify empty key returns InvalidAnnotation error
- [ ] 7.7 Add `test_invalid_annotation_key_too_long` — verify key > 256 chars returns InvalidAnnotation error
- [ ] 7.8 Add `test_invalid_annotation_value_non_string` — verify non-string value returns error
- [ ] 7.9 Add `test_invalid_annotations_format` — verify non-object annotations returns error
- [ ] 7.10 Add `test_annotation_size_limit` — verify total size > 256KB returns InvalidAnnotation error
- [ ] 7.11 Register test module in `tests/src/lib.rs` and `tests/src/main.rs`
- [ ] 7.12 Run integration tests against both InMemory and SQLite stores

## 8. Documentation Updates

- [ ] 8.1 Update `docs/data-model.md` — add `annotations` field to `ObjectMeta` struct definition, update wire format examples, add `InvalidAnnotation` to error table
- [ ] 8.2 Update `docs/api-reference.md` — document `metadata.annotations` in create/update request bodies, document validation rules (256-char key limit, 256KB total limit, no character restrictions)
- [ ] 8.3 Review other docs in `docs/` directory (architecture.md, storage.md) for any references to metadata that need updating

## 9. End-to-End Test Prompts

- [ ] 9.1 Add Test 38 to `docs/testprompt.md`: "Annotations — create object with annotations, verify in response and GET" (mirror Test 5 pattern for labels)
- [ ] 9.2 Add Test 39 to `docs/testprompt.md`: "Annotations — create object without annotations, verify empty map" (mirror Test 6 pattern)
- [ ] 9.3 Add Test 40 to `docs/testprompt.md`: "Annotations — update with changed annotations" (mirror Test 7 pattern)
- [ ] 9.4 Add Test 41 to `docs/testprompt.md`: "Annotations — create Schema with annotations" (mirror Test 8 pattern)
- [ ] 9.5 Add Test 42 to `docs/testprompt.md`: "Annotations — invalid key (empty) returns 400" (mirror Test 9 pattern)
- [ ] 9.6 Add Test 43 to `docs/testprompt.md`: "Annotations — key exceeds length limit returns 400" (mirror Test 11 pattern)
- [ ] 9.7 Add Test 44 to `docs/testprompt.md`: "Annotations — list returns annotations for all objects" (mirror Test 13 pattern)
- [ ] 9.8 Add Test 45 to `docs/testprompt.md`: "Annotations — SQLite persistence survives restart" (mirror Test 14 pattern, verify annotations persist across restart)

## 10. Final Verification

- [ ] 10.1 Run `cargo clippy` and fix any warnings
- [ ] 10.2 Run `cargo fmt` to ensure code formatting
- [ ] 10.3 Run full test suite: `cargo test` and integration tests
- [ ] 10.4 Review `roadmap.md` — remove or update the annotations entry if present
- [ ] 10.5 Verify backward compatibility: existing objects without annotations work correctly
- [ ] 10.6 Test SQLite migration: verify existing databases can add annotations column without data loss
- [ ] 10.7 Run end-to-end test prompts from `docs/testprompt.md` to verify annotations work in real server
