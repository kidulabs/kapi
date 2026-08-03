## Context

The kapi typed client (`kapi-client/src/typed.rs`) provides strongly-typed CRUD methods over the raw `KapiClient`. Errors currently flow through a two-layer generic chain:

```
AppError (server, 18 variants) → HTTP JSON { error, code, details }
  → ClientError::ApiError { status, code, message }  [details DISCARDED]
    → TypedError::Client(ClientError::ApiError { ... })
```

The typed client's `TypedError` has only two variants — `Client(ClientError)` and `Serialization(serde_json::Error)` — so callers must dig through two layers to detect a 404. Meanwhile, the server sends rich structured `details` (e.g., `{ "what": "pod", "identifier": "x" }` for NotFound) that are thrown away by `check_response`.

This design eliminates the double-unwrapping problem by introducing first-class error variants in `TypedError` for errors that drive control flow, while retaining a generic catch-all for display-only errors.

## Goals / Non-Goals

**Goals:**
- Callers can pattern-match on `TypedError::NotFound`, `TypedError::AlreadyExists`, `TypedError::Conflict`, `TypedError::Forbidden` directly
- The `?` operator in typed methods automatically maps errors — no per-method error handling
- Structured `details` from the server are preserved end-to-end
- Minimal blast radius: changes confined to `kapi-client`

**Non-Goals:**
- Mirroring all 18 `AppError` variants client-side
- Moving `ValidationError` to `kapi-core` (future work)
- Changing server-side `AppError` or `IntoResponse` — the wire format is fine

## Decisions

### Decision 1: Enrich `ClientError::ApiError` with `details: Value`

Add `details: serde_json::Value` field to `ClientError::ApiError`. The `check_response` method in `kapi-client/src/client.rs` already reads the `details` field from the JSON body — just retain it instead of discarding.

**Alternatives considered:**
- *Typed client parses raw `reqwest::Response` directly*: Duplicates parsing logic, risks drift between raw and typed clients.
- *Add a new `ApiErrorDetailed` variant alongside existing `ApiError`*: Two code paths, confusing which to use.

**Rationale**: Additive change to `ClientError` (new field on existing variant). Zero-cost for callers who don't need `details` — they just ignore the field. One-line change in `check_response`.

### Decision 2: First-class `TypedError` variants for branch-worthy errors

Replace `TypedError::Client` with specific variants:

```rust
pub enum TypedError {
    NotFound { what: String, identifier: String },
    AlreadyExists { kind: String, name: String },
    Conflict { expected: u64, actual: u64 },
    Forbidden { message: String },
    ApiError(ClientError),       // everything else
    Serialization(#[from] serde_json::Error),
}
```

**Selection criteria**: An error gets a first-class variant only if callers realistically write `if/let` branches on it. NotFound (get-or-create), AlreadyExists (create-or-get), Conflict (retry with new version), and Forbidden (auth flow) all drive control flow. Everything else (validation errors, bad requests, internal errors) is displayed to the user — the caller just needs the message.

**Alternatives considered:**
- *Mirror all 18 `AppError` variants*: Maintenance burden, most errors are display-only. Every server change needs a client match.
- *Add `is_not_found()` convenience methods on existing `TypedError::Client`*: Doesn't improve the type — caller still deals with a generic blob. First-class variants give exhaustive matching and IDE autocompletion.

### Decision 3: Manual `From<ClientError> for TypedError` impl

Implement `From<ClientError> for TypedError` manually. It pattern-matches on `(status, code)` and extracts fields from `details: Value`:

```rust
impl From<ClientError> for TypedError {
    fn from(err: ClientError) -> Self {
        match &err {
            ClientError::ApiError { status: 404, code, details, .. }
                if code == "NotFound" => {
                TypedError::NotFound {
                    what: details["what"].as_str().unwrap_or("unknown").to_string(),
                    identifier: details["identifier"].as_str().unwrap_or("unknown").to_string(),
                }
            }
            // AlreadyExists, Conflict, Forbidden — same pattern
            _ => TypedError::ApiError(err),
        }
    }
}
```

Remove the `#[from]` attribute on `ApiError(ClientError)` since manual `From` and `#[from]` can't coexist.

**Why not use `status` alone (without checking `code`)?**: HTTP status codes are ambiguous — 404 could be NotFound or StatusSubresourceNotEnabled. Checking both `status` and `code` ensures correct variant selection.

### Decision 4: Field extraction with defensive defaults

Extract structured fields from `details: Value` using `.as_str().unwrap_or("unknown")`. This ensures the typed client never panics even if the server sends unexpected detail shapes.

**Why `unwrap_or("unknown")` and not `Option<String>`?**: The `NotFound { what, identifier }` variant fields are `String` (not `Option<String>`). Making them `Option` would push the burden to every caller. The `details` format is server-controlled — if the server sends `what` and `identifier`, they'll always be strings. The `"unknown"` default is a defensive fallback that should never trigger in practice.

## Risks / Trade-offs

- **[Breaking change to `ClientError::ApiError`]** → All callers constructing or matching on `ApiError { status, code, message }` must add `details` to their patterns. Mitigation: This is a compile-time error — no silent breakage.
- **[Breaking change to `TypedError`]** → Callers matching `TypedError::Client(...)` must update to `TypedError::ApiError(...)`. Mitigation: Same — compile-time error.
- **[Stringly-typed `code` matching]** → The `From` impl matches on string codes like `"NotFound"`. If the server renames a code, the mapping silently falls through to `ApiError`. Mitigation: The code strings are defined in `AppError::IntoResponse` and unlikely to change. Add a comment documenting the dependency.
- **[Partial detail extraction]** → `Conflict { expected, actual }` extracts from `details["expected"]` and `details["actual"]` which are JSON numbers. If the server changes these to strings, `as_u64()` returns `None` and the fallback is `0`. Mitigation: Same defensive default strategy; server contract is stable.
