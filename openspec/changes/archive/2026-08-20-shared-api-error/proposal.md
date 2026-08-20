## Why

The server's `AppError` (17 variants) and the client's `TypedError` (4 typed variants + catch-all) are independently maintained, causing variant drift, magic-string destructuring of JSON `details`, and duplicate type definitions. The controller matches on raw HTTP status codes (`404`, `409`) instead of semantic error types, and the CAS retry in `finalizer.rs` incorrectly retries on *any* 409 — including `ObjectBeingDeleted` and `NamespaceNotEmpty`, which should not be retried. This refactor creates a single shared error type in `kapi-core` that both server and client use, making the wire contract explicit and compiler-enforced.

## What Changes

- **Add `ApiError` enum to `kapi-core`** — a serializable, `#[non_exhaustive]` enum representing the wire contract between server and client. 15 named variants + 1 `Unknown` catch-all for forward compatibility. Derives `Serialize`, `Deserialize`, `thiserror::Error`. Includes `fn http_status(&self) -> u16` to map variants to HTTP codes without pulling Axum into `kapi-core`.
- **Refactor `AppError` in `kapi-server`** — becomes a thin wrapper: `Api(ApiError)` for all structured domain errors, `Internal(anyhow::Error)` for server-only unexpected failures, plus server-only variants (`InvalidSchema`, `StoredSchemaCompilationFailed`). The 142-line `IntoResponse` match collapses to ~30 lines.
- **Refactor `ClientError` in `kapi-client`** — `ClientError::Api(ApiError)` replaces `ClientError::ApiError { status, code, message, details: Value }`. The client deserializes error responses directly into `ApiError` via serde. **BREAKING** change to `kapi-client` public API.
- **Delete `TypedError`** — becomes redundant when `ClientError::Api(ApiError)` is already fully typed. The 32-line `From<ClientError> for TypedError` magic-string destructuring is eliminated.
- **Update `kapi-controller`** — matches on `ApiError::NotFound { .. }`, `ApiError::Conflict { .. }`, `ApiError::ObjectBeingDeleted { .. }` instead of raw status codes. Fixes the CAS retry correctness bug.
- **Collapse 7 `Invalid*` variants** — `InvalidFieldSelector`, `InvalidLabel`, `InvalidAnnotation`, `InvalidFinalizer`, `InvalidLabelSelector`, `InvalidRequestBody`, `InvalidRequest` become one `InvalidRequest { what, message }` variant.
- **Absorb `CoreError`** — the existing 2-variant `CoreError` in `kapi-core` is absorbed into `ApiError::InvalidRequest`.

## Capabilities

### New Capabilities
- `shared-api-error`: The shared `ApiError` enum in `kapi-core` — its variants, serialization format, HTTP status mapping, and forward-compatibility contract.

### Modified Capabilities
- `error-handling`: Server-side `AppError` becomes a wrapper around `ApiError`. `IntoResponse` implementation changes. `CoreError` is absorbed.
- `typed-client-errors`: `TypedError` is deleted. `ClientError` wraps `ApiError` directly. Error deserialization uses serde instead of manual string matching.
- `controller-runtime-core`: Controller error matching changes from status codes to `ApiError` variants. CAS retry correctness fix.

## Impact

- **`kapi-core`**: New `ApiError` enum (~80 lines). No new dependencies (serde, thiserror already present). `CoreError` deprecated/removed.
- **`kapi-server`**: `AppError` refactored to wrap `ApiError`. `IntoResponse` simplified. All call sites constructing `AppError` variants updated.
- **`kapi-client`**: **BREAKING** — `ClientError::ApiError` struct variant replaced with `ClientError::Api(ApiError)`. `TypedError` deleted. `check_response()` deserializes `ApiError` directly.
- **`kapi-controller`**: Error matching updated from status codes to `ApiError` variants. CAS retry in `finalizer.rs` fixed.
- **`kapi-cli`**: `CliError` may need updating to wrap `ApiError` instead of matching on status codes.
- **Wire format**: Changes from `{"error": "...", "code": "...", "details": ...}` to `{"code": "...", "details": ...}` via tagged serde. Acceptable at v0.4.0 (not stable).
- **Integration tests**: Error assertion patterns need updating across test suite.

## Non-goals

- Changing the `Reconciler` trait's error type from `Box<dyn Error>` — this is a separate follow-up.
- Adding new error variants beyond what `AppError` already covers.
- Stabilizing the wire format for external consumers (still pre-1.0).

## Future Work

- Typed `ReconcileError` wrapping `ClientError` for controller authors (replaces `Box<dyn Error>`).
- Error code registry / documentation for external API consumers.
