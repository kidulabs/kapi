## Context

The kapi workspace has a client-server architecture with a shared `kapi-core` crate for types. Currently, error handling is fragmented:

- **Server** (`kapi-server`): `AppError` enum with 17 variants, serialized to HTTP via a 142-line `IntoResponse` match. Includes `Internal(anyhow::Error)` which is server-only.
- **Client** (`kapi-client`): `ClientError` with an `ApiError { status, code, message, details: Value }` variant that captures HTTP errors as untyped JSON. `TypedError` attempts to provide typed variants but only covers 4 of 17 server errors; the rest fall into a catch-all.
- **Controller** (`kapi-controller`): Matches on raw HTTP status codes (`404`, `409`) for control flow. The CAS retry in `finalizer.rs` retries on *any* 409, including `ObjectBeingDeleted` and `NamespaceNotEmpty`, which is incorrect.
- **Wire format**: `{"error": "...", "code": "...", "details": ...}` — manually constructed in `IntoResponse`, manually parsed in client.

The `kapi-core` crate is currently lightweight (StoredObject, WatchEvent, selectors, CoreError with 2 variants). It already contains `ValidationError` which is used by the server.

## Goals / Non-Goals

**Goals:**
- Create a single `ApiError` enum in `kapi-core` that represents the wire contract between server and client
- Eliminate variant drift, magic-string destructuring, and duplicate type definitions
- Fix the CAS retry correctness bug in `kapi-controller`
- Simplify server `IntoResponse` from 142 lines to ~30 lines
- Delete `TypedError` and the 32-line `From<ClientError>` conversion
- Maintain forward compatibility via `Unknown` catch-all variant

**Non-Goals:**
- Changing the `Reconciler` trait's error type from `Box<dyn Error>` (separate follow-up)
- Adding new error variants beyond what `AppError` already covers
- Stabilizing the wire format for external consumers (still pre-1.0)
- Modifying the `WorkQueue` or watch stream logic

## Decisions

### Decision 1: Place `ApiError` in `kapi-core`

**Choice:** Define `ApiError` in `kapi-core/src/error.rs` as a shared enum.

**Rationale:** 
- `kapi-core` is already the shared types crate (StoredObject, WatchEvent, ValidationError)
- Both server and client depend on `kapi-core`, so it's the natural home
- No new dependencies needed (serde, thiserror already present)
- Makes the wire contract explicit and compiler-enforced

**Alternatives considered:**
- *Move full `AppError` to `kapi-core`*: Rejected because `Internal(anyhow::Error)` would pull anyhow into kapi-core, and `IntoResponse` would pull axum.
- *Keep separate types with codegen*: Rejected as over-engineered for current project size.

### Decision 2: Use tagged serde for serialization

**Choice:** `#[serde(tag = "code", content = "details", rename_all = "PascalCase")]`

**Rationale:**
- Eliminates manual JSON construction in `IntoResponse`
- Eliminates manual JSON parsing in client
- The `code` field becomes the variant name, `details` becomes the variant fields
- Cleaner than current `{"error": "...", "code": "...", "details": ...}` format

**Alternatives considered:**
- *Preserve current wire format with custom Serialize/Deserialize*: Rejected as ~30 lines of boilerplate for no benefit. The wire format is not stable at v0.4.0.
- *Use `#[serde(tag = "code")]` (adjacently tagged)*: Rejected because variant fields would be flattened into the top level, making the structure less clear.

### Decision 3: Wire format change

**Choice:** Accept the format change from `{"error": "...", "code": "...", "details": ...}` to `{"code": "...", "details": ...}`.

**Rationale:**
- The `error` field is redundant — the `code` + `details` already convey the error
- Human-readable messages can be generated client-side via `thiserror::Display`
- Simpler serialization/deserialization
- Project is at v0.4.0, wire format is not stable

**Alternatives considered:**
- *Keep `error` field for backward compatibility*: Rejected as unnecessary complexity for a pre-1.0 project.

### Decision 4: `Unknown` catch-all variant

**Choice:** `Unknown { code: String, message: String, details: serde_json::Value }` for forward compatibility.

**Rationale:**
- If the server adds a new variant, older clients can still deserialize it
- Preserves the full payload for logging/debugging
- `#[non_exhaustive]` on the enum provides additional compile-time safety

**Alternatives considered:**
- *`Internal { message: String }`*: Rejected because it loses the `details` payload, which may contain useful debugging information.
- *No catch-all (fail on unknown variants)*: Rejected because it breaks forward compatibility.

### Decision 5: Collapse 7 `Invalid*` variants into `InvalidRequest`

**Choice:** `InvalidRequest { what: String, message: String }` replaces `InvalidFieldSelector`, `InvalidLabel`, `InvalidAnnotation`, `InvalidFinalizer`, `InvalidLabelSelector`, `InvalidRequestBody`, `InvalidRequest`.

**Rationale:**
- Clients don't need to match on these for control flow — they all mean "fix your input"
- The `what` field provides human-readable context: `"invalid {what}: {message}"` → `"invalid label: 'foo' is not valid"`
- Reduces enum size from 17 to 15 named variants
- Simpler for controller authors — one arm instead of seven

**Alternatives considered:**
- *Keep all 7 variants separate*: Rejected as unnecessary granularity for errors that don't require different handling.
- *Use `field: Option<String>` instead of `what: String`*: Rejected because `"field selector"` isn't a field path, it's a query parameter. `what` is more natural.

### Decision 6: Server wraps `ApiError` with `AppError`

**Choice:** Server defines `AppError { Api(ApiError), Internal(anyhow::Error), InvalidSchema(String), StoredSchemaCompilationFailed { .. } }`.

**Rationale:**
- `Internal(anyhow::Error)` is server-only (DB errors, panics) — doesn't belong in shared type
- `InvalidSchema` and `StoredSchemaCompilationFailed` are server configuration errors, not client-actionable
- `IntoResponse` for `AppError::Api` becomes a one-liner: serialize + status from `api_error.http_status()`
- Keeps server-specific concerns out of `kapi-core`

**Alternatives considered:**
- *Put all variants in `ApiError`, including `Internal`*: Rejected because it would pull anyhow into kapi-core.

### Decision 7: Delete `TypedError`

**Choice:** Remove `TypedError` entirely. `ClientError::Api(ApiError)` is already fully typed.

**Rationale:**
- `TypedError` was a workaround for `ClientError::ApiError` being untyped
- With `ClientError::Api(ApiError)`, the typed variants are already available
- Eliminates the 32-line `From<ClientError> for TypedError` conversion
- Simpler API surface — one error type instead of two

**Alternatives considered:**
- *Keep `TypedError` as a thin wrapper*: Rejected as unnecessary indirection.
- *Make `TypedError` a type alias*: Rejected because it adds no value over using `ClientError` directly.

### Decision 8: `http_status()` method on `ApiError`

**Choice:** Add `pub fn http_status(&self) -> u16` to `ApiError` in `kapi-core`.

**Rationale:**
- Maps each variant to its HTTP status code (404, 409, 422, etc.)
- Avoids pulling `axum::http::StatusCode` into `kapi-core`
- Server uses it in `IntoResponse`: `StatusCode::from_u16(api_err.http_status()).unwrap()`
- Client can use it for logging or filtering if needed

**Alternatives considered:**
- *Use `axum::http::StatusCode` directly*: Rejected because it would pull axum into kapi-core.
- *Store status code in each variant*: Rejected as redundant — the status is deterministic based on the variant.

## Risks / Trade-offs

**[Risk] Breaking change to `kapi-client` public API** → Mitigation: Project is at v0.4.0, consumers are internal (controller, CLI). Bump to v0.5.0 with clear changelog.

**[Risk] Wire format change breaks existing clients** → Mitigation: Wire format is not stable at v0.4.0. Document the change in release notes.

**[Risk] `Unknown` variant loses type safety for new errors** → Mitigation: `#[non_exhaustive]` on the enum forces wildcard arms. `Unknown` preserves full payload for debugging. When a new variant is added to `ApiError`, clients can update their code to handle it explicitly.

**[Risk] CAS retry correctness fix changes behavior** → Mitigation: The current behavior (retrying on any 409) is a bug. The fix (retrying only on `Conflict`) is the correct behavior. Document in changelog.

**[Trade-off] `InvalidRequest { what, message }` loses compile-time distinction between error kinds** → Acceptable because clients don't match on these for control flow. The `what` field provides sufficient context for error messages.

**[Trade-off] Server `AppError` has fewer variants than before** → Acceptable because the removed variants (`InvalidFieldSelector`, etc.) are now represented as `InvalidRequest { what: "field selector", .. }`. The semantic information is preserved.
