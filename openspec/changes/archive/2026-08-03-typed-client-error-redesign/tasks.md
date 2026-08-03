## 1. Enrich ClientError::ApiError with structured details

- [x] 1.1 Add `details: serde_json::Value` field to `ClientError::ApiError` in `kapi-client/src/error.rs`. Add a comment explaining that this preserves the server's structured error context from the `details` field in error responses.
- [x] 1.2 Update `check_response` in `kapi-client/src/client.rs` to retain the `details` field from the JSON error body instead of discarding it. The `details` key is already read in the current code — keep it and pass it to the `ApiError` variant.
- [x] 1.3 Run `cargo check -p kapi-client` and fix any compilation errors from the new field (callers constructing or matching on `ApiError` without `details`).

## 2. Redesign TypedError with first-class variants

- [x] 2.1 Replace `TypedError::Client(ClientError)` with first-class variants in `kapi-client/src/typed.rs`: `NotFound { what: String, identifier: String }`, `AlreadyExists { kind: String, name: String }`, `Conflict { expected: u64, actual: u64 }`, `Forbidden { message: String }`, and a generic `ApiError(ClientError)` catch-all. Remove the `#[from]` attribute from the `ApiError` variant. Add doc comments on each variant explaining when it is produced.
- [x] 2.2 Implement `From<ClientError> for TypedError` manually. Match on `(status, code)` pairs: `(404, "NotFound")` → `NotFound`, `(409, "AlreadyExists")` → `AlreadyExists`, `(409, "Conflict")` → `Conflict`, `(403, "ProtectedNamespace")` → `Forbidden`. All other errors → `ApiError(err)`. Extract fields from `details` using defensive defaults (`as_str().unwrap_or("unknown")` for strings, `as_u64().unwrap_or(0)` for numbers). Add a comment documenting the server code-string dependency.
- [x] 2.3 Verify the `#[from] serde_json::Error` on `TypedError::Serialization` still works and that `?` conversion of `serde_json::Error` is unaffected.

## 3. Fix breaking changes across the workspace

- [x] 3.1 Run `cargo check` on the full workspace and fix all compilation errors from callers matching on `TypedError::Client(...)` (now `TypedError::ApiError(...)`) and `ClientError::ApiError { status, code, message }` (now needs `..` or explicit `details` field).
- [x] 3.2 Update any existing integration tests in `kapi-server/tests/` that match on old `TypedError` or `ClientError` variants.

## 4. Add tests for error variant mapping

- [x] 4.1 Add integration tests that verify each first-class `TypedError` variant is produced correctly: trigger NotFound, AlreadyExists, Conflict, and Forbidden from the typed client and assert the correct variant and field values.
- [x] 4.2 Add a test that verifies non-mapped errors (e.g., `InvalidLabel`, `SchemaValidation`) fall through to `TypedError::ApiError` and that the `code` field is accessible via the wrapped `ClientError`.
- [x] 4.3 Add a test for defensive defaults: if the server sends a 404 with empty `details`, the `NotFound` variant uses `"unknown"` defaults without panicking.

## 5. Verification

- [x] 5.1 Run `cargo check --workspace` — all crates compile cleanly.
- [x] 5.2 Run `cargo clippy --workspace -- -D warnings` — no clippy warnings.
- [x] 5.3 Run the full e2e test suite (`cargo run -p kapi-tests`) — all existing tests pass with the updated error types.
- [x] 5.4 Check `openspec/specs/` for any docs that reference old `TypedError` or `ClientError` variant names and update them if found.
- [x] 5.5 Check the roadmap for any items impacted by this change and update accordingly.

## 6. DO NOT auto-commit

- [x] 6.1 Do not auto-commit any changes. Leave all modifications staged or unstaged for user review.
