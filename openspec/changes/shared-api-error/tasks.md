## 1. kapi-core: Define ApiError

- [x] 1.1 Create `ApiError` enum in `kapi-core/src/error.rs` with 15 named variants + `Unknown` catch-all. Derive `Debug, Clone, Serialize, Deserialize, thiserror::Error, PartialEq, Eq`. Use `#[serde(tag = "code", content = "details", rename_all = "PascalCase")]` and `#[non_exhaustive]`. Add `#[error("...")]` display attributes per spec.
- [x] 1.2 Add `pub fn http_status(&self) -> u16` method to `ApiError` mapping each variant to its HTTP status code.
- [x] 1.3 Deprecate `CoreError` with `#[deprecated]` attribute. Add `From<CoreError> for ApiError` converting variants to `ApiError::InvalidRequest`.
- [x] 1.4 Update `kapi-core/src/lib.rs` to export `ApiError` alongside `CoreError`.

## 2. kapi-server: Refactor AppError

- [x] 2.1 Refactor `AppError` in `kapi-server/src/error.rs` to: `Api(ApiError)` for domain errors, `Internal(anyhow::Error)` for unexpected failures, `InvalidSchema(String)`, `StoredSchemaCompilationFailed { schema_name, reason }`. Remove all 17 old variants.
- [x] 2.2 Rewrite `IntoResponse` for `AppError` (~30 lines). Delegate to `ApiError::http_status()` and serde serialization for `AppError::Api`. Map `Internal` to 500 with `{"code": "Internal", "details": {"message": "internal error"}}`. Map `InvalidSchema` to 422. Map `StoredSchemaCompilationFailed` to 500.
- [x] 2.3 Update `From<CoreError>` impl in `kapi-server/src/error.rs` to produce `AppError::Api(ApiError::InvalidRequest { .. })` instead of old variants.
- [x] 2.4 Update `kapi-server/src/validation/mod.rs`: replace all `AppError::InvalidLabel(msg)` → `AppError::Api(ApiError::InvalidRequest { what: "label".into(), message: msg })`, same for `InvalidAnnotation` → `"annotation"`, `InvalidFinalizer` → `"finalizer"`. Update test assertions.
- [x] 2.5 Update `kapi-server/src/object/service.rs`: replace all `AppError::NotFound { .. }` → `AppError::Api(ApiError::NotFound { .. })`, same for `AlreadyExists`, `Conflict`, `ProtectedNamespace`, `NamespaceNotEmpty`, `ObjectBeingDeleted`, `SchemaValidation`, `InvalidRequest`, `StatusSubresourceNotEnabled`. Update test assertions.
- [x] 2.6 Update `kapi-server/src/object/schema_service.rs`: replace `AppError::Conflict` → `AppError::Api(ApiError::Conflict { .. })`, `InvalidSchema` stays as `AppError::InvalidSchema`, `SchemaHasObjects` → `AppError::Api(ApiError::SchemaHasObjects { .. })`.
- [x] 2.7 Update `kapi-server/src/store/sqlite.rs` and `store/continue_token.rs`: `AppError::Internal` stays unchanged (no migration needed). Verify compilation.
- [x] 2.8 Update `kapi-server/src/lib.rs` re-exports: add `ApiError`, keep `CoreError` (deprecated).

## 3. kapi-client: Refactor ClientError

- [x] 3.1 Refactor `ClientError` in `kapi-client/src/error.rs`: replace `ApiError { status, code, message, details }` with `Api(kapi_core::error::ApiError)`. Keep `HttpError`, `SerializationError`, `StreamError`.
- [x] 3.2 Rewrite `check_response()` in `kapi-client/src/client.rs` to deserialize error body directly into `ApiError` via serde. Return `ClientError::Api(api_error)`.
- [x] 3.3 Delete `kapi-client/src/typed.rs` (TypedError, TypedClient, TypedResource). Remove `pub mod typed` and `pub use typed::{...}` from `kapi-client/src/lib.rs`.
- [x] 3.4 Update `kapi-client/src/lib.rs` re-exports: add `ApiError`, remove `TypedError`, `TypedClient`, `TypedResource`.

## 4. kapi-controller: Update error matching

- [x] 4.1 Update `kapi-controller/src/finalizer.rs`: CAS retry in `ensure_finalizer` and `remove_finalizer` must match `ClientError::Api(ApiError::Conflict { .. })` instead of `ClientError::ApiError { status: 409, .. }`. Non-conflict errors (ObjectBeingDeleted, NamespaceNotEmpty) must NOT retry.
- [x] 4.2 Update `kapi-controller/src/controller.rs`: match `ClientError::Api(ApiError::NotFound { .. })` instead of `ClientError::ApiError { status: 404, .. }`.

## 5. kapi-cli: Update error handling

- [x] 5.1 Update `kapi-cli/src/error.rs`: rewrite `From<ClientError> for CliError` to match on `ClientError::Api(ApiError::*)` variants instead of status codes. Update `from_not_found` helper similarly.
- [x] 5.2 Update `kapi-cli/src/main.rs`: replace `ClientError::ApiError { status: 404, .. }` pattern with `ClientError::Api(ApiError::NotFound { .. })` in upsert logic.

## 6. Verification

- [x] 6.1 Run `cargo check --workspace` and fix any compilation errors.
- [x] 6.2 Run `cargo clippy --workspace` and fix warnings.
- [x] 6.3 Run integration tests (`cargo run -p kapi-tests`) and fix failures.
- [x] 6.4 Check `docs/` directory and `roadmap` for items impacted by error handling changes; update if needed.

## 7. Restore TypedClient

- [x] 7.1 Restore `kapi-client/src/typed.rs` with `TypedResource` trait and `TypedClient<T>` (methods: `new`, `inner`, `create`, `get`, `update`, `delete`, `list`, `stored_to_typed`, `typed_to_stored`). NO `TypedError` — all methods return `Result<T, ClientError>`. `TypedResource::to_stored_object()` returns `Result<StoredObject, ClientError>` wrapping serde errors via `ClientError::SerializationError`.
- [x] 7.2 Update `kapi-client/src/lib.rs` to re-export `TypedClient` and `TypedResource` (NOT `TypedError`).
- [x] 7.3 Restore `pub mod typed` in `kapi-controller/src/finalizer.rs` with `typed::ensure_finalizer` and `typed::remove_finalizer` accepting `&T where T: TypedResource`.
- [x] 7.4 Restore `TypedResource` codegen in `kapibuild/src/generate.rs` (import + `impl TypedResource for {kind}` block for both with-status and without-status resources).
- [x] 7.5 Restore `TypedClient` codegen in `kapibuild/src/controller_generate.rs` (import + typed reconcile template + `finalizer::typed` helpers). Update kapibuild tests to match new output.
- [x] 7.6 Recreate `kapi-server/tests/src/typed_client.rs` using `ClientError`/`ApiError` for error assertions; re-register module in `lib.rs` and tests in `main.rs`.
- [x] 7.7 Restore `TypedClient`/`TypedResource` references in `docs/controller-runtime.md`, `docs/kapibuild/workflow.md`, `docs/kapibuild/controller-patterns.md`, `docs/kapibuild/project-structure.md`.
- [x] 7.8 Verify: `cargo check --workspace`, `cargo test -p kapi-client`, `cargo test -p kapibuild`, `cargo run -p kapi-tests`. Also fix pre-existing `api_error_from_unknown_code_falls_back_to_unknown` failure in `kapi-client/src/client.rs` (`parse_error_body` fallback now preserves the unknown server code).
