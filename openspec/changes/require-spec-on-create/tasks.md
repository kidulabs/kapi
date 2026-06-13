## 1. Error Handling

- [x] 1.1 Add `InvalidRequestBody(String)` variant to `AppError` enum in `src/error.rs` with `#[error("invalid request body: {0}")]`
- [x] 1.2 Implement `IntoResponse` for `InvalidRequestBody` mapping to HTTP 400 with JSON body `{ "error": "...", "code": "InvalidRequestBody", "details": { "message": "..." } }`

## 2. Handler Implementation

- [x] 2.1 Update create handler in `src/object/handler.rs` to extract `spec` field from body instead of stripping metadata/status
- [x] 2.2 Add validation: reject if `spec` field is missing → `InvalidRequestBody("'spec' field is required")`
- [x] 2.3 Add validation: reject if `spec` is not a JSON object → `InvalidRequestBody("'spec' must be a JSON object")`
- [x] 2.4 Add validation: reject if `spec` is empty object `{}` → `InvalidRequestBody("'spec' must not be empty")`
- [x] 2.5 Add validation: reject if body contains top-level fields other than `metadata` and `spec` → `InvalidRequestBody` with message listing unknown field(s)
- [x] 2.6 Run `cargo check` to verify handler changes compile

## 3. Test Helper Updates

- [x] 3.1 Update `widget()` helper in `tests/src/lib.rs` to wrap `color` and `size` in `spec` field
- [x] 3.2 Update `widget_with_labels()` helper in `tests/src/lib.rs` to wrap `color` and `size` in `spec` field
- [x] 3.3 Run `cargo check` in tests crate to verify helper changes compile

## 4. Inline Test Body Updates

- [x] 4.1 Update inline create bodies in `tests/src/status_subresource.rs` (11 places) to wrap domain fields in `spec`
- [x] 4.2 Update any other inline create bodies across test files to use `spec` wrapper
- [x] 4.3 Run `cargo check` in tests crate to verify all test bodies compile

## 5. Test Updates for New Validation

- [x] 5.1 Update `test_create_ignores_status_in_body` to assert rejection (400) instead of silent drop, since `status` is now an unknown field
- [x] 5.2 Add test: create with missing `spec` returns 400 `InvalidRequestBody`
- [x] 5.3 Add test: create with empty `spec: {}` returns 400 `InvalidRequestBody`
- [x] 5.4 Add test: create with non-object `spec` (e.g., array, string) returns 400 `InvalidRequestBody`
- [x] 5.5 Add test: create with unknown top-level field returns 400 `InvalidRequestBody`

## 6. Verification

- [x] 6.1 Run `cargo clippy --all-targets` and fix any warnings
- [x] 6.2 Run `cargo test --all` and verify all tests pass
- [x] 6.3 Run integration tests against both InMemory and SQLite stores

## 7. Documentation

- [x] 7.1 Check `docs/` directory for API documentation that needs updating
- [x] 7.2 Check `openspec/specs/roadmap-update/` or roadmap files for items impacted by this breaking change
- [x] 7.3 Update any affected documentation

## 8. Update docs/testprompt.md

- [x] 8.1 Update all curl POST commands in `docs/testprompt.md` to wrap domain fields in `spec` (e.g., `"color":"blue","size":1` → `"spec":{"color":"blue","size":1}`)
- [x] 8.2 Update Test 36 ("Create object with status in body — status is ignored") to test that unknown top-level fields are rejected with 400 `InvalidRequestBody` instead of silently ignored
- [x] 8.3 Verify all curl commands in `docs/testprompt.md` use the new `spec` format
