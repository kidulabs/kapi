## 1. Add AlreadyExists error variant

- [ ] 1.1 Add `AlreadyExists { kind: String, name: String }` variant to `AppError` enum in `src/error.rs` with `#[error("{kind} '{name}' already exists")]` derive
- [ ] 1.2 Add `AlreadyExists` match arm in `IntoResponse` impl mapping to HTTP 409 with `code: "AlreadyExists"` and `details: { kind, name }`
- [ ] 1.3 Run `cargo check` to verify the enum change compiles

## 2. Update InMemoryStore to return AlreadyExists on duplicate

- [ ] 2.1 In `src/store/memory.rs`, change the duplicate check in `create()` (line ~53) from `AppError::Conflict { expected: 0, actual: 0 }` to `AppError::AlreadyExists { kind, name }` using the `ResourceKey.kind` and object name
- [ ] 2.2 Update the duplicate-create test in `src/store/memory.rs` (line ~224) to assert `AppError::AlreadyExists` instead of `AppError::Conflict`
- [ ] 2.3 Review any other `Conflict` assertions in memory.rs tests — only the OCC version-mismatch test (line ~383) should remain as `Conflict`

## 3. Update SQLiteStore to return AlreadyExists on duplicate

- [ ] 3.1 In `src/store/sqlite.rs`, change the `ConstraintViolation` mapping in `create()` (line ~180) from `AppError::Conflict` to `AppError::AlreadyExists { kind, name }` — extract kind from the `ResourceKey` and name from the insert parameters
- [ ] 3.2 Update the duplicate-create test in `src/store/sqlite.rs` (line ~505) to assert `AppError::AlreadyExists` instead of `AppError::Conflict`
- [ ] 3.3 Verify the OCC version-mismatch test (line ~662) remains as `AppError::Conflict` — no change needed there

## 4. Update ObjectService tests

- [ ] 4.1 In `src/object/service.rs`, update the duplicate-create test (line ~606) to assert `AppError::AlreadyExists`
- [ ] 4.2 In `src/object/service.rs`, update the other duplicate-create test (line ~738) to assert `AppError::AlreadyExists`
- [ ] 4.3 Verify OCC-related tests still assert `AppError::Conflict` — no changes needed for version mismatch scenarios

## 5. Update OpenAPI error documentation

- [ ] 5.1 In `src/openapi/paths.rs` or wherever error responses are documented, add `AlreadyExists` as a documented 409 response for POST (create) operations
- [ ] 5.2 Ensure `Conflict` documentation remains for PUT (update) operations — it still represents version mismatches
- [ ] 5.3 Verify the `AppError` component schema in the OpenAPI generator includes the new `AlreadyExists` shape

## 6. Verify and test

- [ ] 6.1 Run `cargo clippy` — no new warnings
- [ ] 6.2 Run `cargo test` — all tests pass
- [ ] 6.3 Run integration tests in `tests/` if they exist and cover error scenarios
- [ ] 6.4 Manually verify a duplicate create returns `{ "code": "AlreadyExists", "details": { "kind": "...", "name": "..." } }` with HTTP 409

## 7. Update documentation

- [ ] 7.1 Check if any items in the roadmap are impacted by this change and update accordingly
- [ ] 7.2 Review any API documentation or README that mentions error responses and update to reflect the new `AlreadyExists` variant
