## Why

The typed client's `TypedError` has only two variants (`Client` and `Serialization`), making it impossible to branch on common API errors without unwrapping two layers of generic types. Callers must write `Err(TypedError::Client(ClientError::ApiError { status: 404, .. }))` to detect a not-found — and the server's structured `details` field is discarded before it reaches the client.

## What Changes

- **Enrich `ClientError::ApiError`** with a `details: Value` field so the raw client retains the structured error context the server already sends (`kapi-client/src/error.rs`, `kapi-client/src/client.rs`).
- **Redesign `TypedError`** with first-class variants for errors callers branch on: `NotFound { what, identifier }`, `AlreadyExists { kind, name }`, `Conflict { expected, actual }`, `Forbidden { message }`. A generic `ApiError(ClientError)` catch-all covers everything else.
- **Add `From<ClientError> for TypedError`** that pattern-matches on `(status, code)` and extracts fields from `details`. The `?` operator in typed methods maps errors automatically — no per-method handling needed. Removes the `#[from]` attribute on the `ApiError` variant.
- **BREAKING**: `TypedError::Client` variant is replaced by `TypedError::ApiError`. Callers matching on `TypedError::Client(...)` must update.

## Capabilities

### New Capabilities

- `typed-client-errors`: Structured error types for the typed client with first-class variants for branch-worthy API errors (NotFound, AlreadyExists, Conflict, Forbidden) and automatic mapping from raw client errors.

### Modified Capabilities

- `kapi-client`: The `ClientError::ApiError` variant gains a `details: Value` field. This is a **BREAKING** change to the `ClientError` enum — callers constructing or matching on `ApiError` must update.
- `error-handling`: The client-side error chain now preserves the structured `details` from server `AppError` responses through to `TypedError` variants.

## Non-goals

- Mirroring all 18 `AppError` variants on the client side — most errors are displayed, not branched on.
- Defining a shared error type between server and client — they remain separate crates with separate error enums.
- Changing the server-side `AppError` or `IntoResponse` — the wire protocol already sends everything we need.

## Impact

- **Code**: `kapi-client/src/error.rs` (ClientError struct), `kapi-client/src/client.rs` (check_response), `kapi-client/src/typed.rs` (TypedError enum, From impl).
- **BREAKING**: Both `ClientError::ApiError` and `TypedError` are public types. Existing callers matching on these variants need to update.
- **Dependencies**: No new crates — `serde_json::Value` is already available.
- **Tests**: Existing integration tests matching on `TypedError::Client(...)` need updating. New tests for error variant mapping.

## Future Work

- Surface `SchemaValidation` errors with structured `Vec<ValidationError>` if `ValidationError` is moved to `kapi-core` (shared types crate).
- Add typed error variants for additional server errors (e.g., `NamespaceNotEmpty`, `ObjectBeingDeleted`) if callers need to branch on them.
