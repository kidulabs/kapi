## Context

The `AppError` enum in `src/error.rs` currently uses a single `Conflict { expected: u64, actual: u64 }` variant for two distinct failure modes:

1. **Duplicate resource creation** — `create()` called with a name that already exists. Stores return `Conflict { expected: 0, actual: 0 }`.
2. **Optimistic concurrency mismatch** — `update()` called with a stale `resourceVersion`. Stores return `Conflict { expected: N, actual: M }` where N ≠ M.

Both map to HTTP 409 with `code: "Conflict"`. The `expected`/`actual` fields are meaningful for OCC but meaningless for duplicates (both zero), making client-side error handling ambiguous.

## Goals / Non-Goals

**Goals:**
- Introduce `AlreadyExists { kind: String, name: String }` as a distinct 409 error variant
- Reserve `Conflict { expected, actual }` exclusively for OCC version mismatches
- Update all stores and tests to use the correct variant
- Update OpenAPI documentation to reflect the new error shape
- Maintain backward compatibility on the HTTP status level (both remain 409)

**Non-Goals:**
- No new HTTP status codes
- No changes to error handling architecture
- No changes to `SchemaHasObjects` (already a distinct 409 variant)
- No `BadRequest` or `InvalidInput` additions

## Decisions

### Decision 1: `AlreadyExists` carries `kind` and `name` fields

**Rationale:** The client needs to know *what* already exists to take corrective action (e.g., GET the existing resource, PATCH instead of POST). The `kind` (e.g., "Widget", "Schema") and `name` (e.g., "my-widget") fields provide this context directly in the error response.

**Alternatives considered:**
- `AlreadyExists { key: ResourceKey, name: String }` — more precise but exposes internal types in error responses
- `AlreadyExists { message: String }` — simpler but unstructured, harder for clients to parse
- `AlreadyExists` with no fields — minimal but forces clients to guess from request context

**Chosen:** `{ kind, name }` — structured, client-friendly, matches the `NotFound { what, identifier }` pattern already in use.

### Decision 2: Both variants remain HTTP 409

**Rationale:** Both are genuine state conflicts per RFC 7231. Using different status codes would imply one is "more correct" than the other, which isn't the case. The distinction is in the `code` field of the JSON response body.

### Decision 3: `Display` impl for `AlreadyExists` produces a human-readable message

Format: `"{kind} '{name}' already exists"` — consistent with how `NotFound` produces `"{what} '{identifier}' not found"`.

### Decision 4: SQLite `ConstraintViolation` on INSERT maps to `AlreadyExists`

The `rusqlite::ErrorCode::ConstraintViolation` in `sqlite.rs:176-184` is currently mapped to `Conflict`. It will be remapped to `AlreadyExists` since this code path only fires on INSERT (create), never on UPDATE.

## Risks / Trade-offs

| Risk | Mitigation |
|------|-----------|
| Clients that only check HTTP 409 (not the `code` field) see no behavior change | This is intentional — the status code contract is preserved |
| Clients that parse `expected`/`actual` from 409 responses to detect duplicates will break | These clients were already relying on an implementation detail (`0/0` signal). The new `code` field is the correct signal. |
| `AlreadyExists` adds a new variant to `IntoResponse` match, requiring exhaustiveness | Compiler enforces this — no risk of silent omission |
| OpenAPI spec change: clients generating from the spec will see a new error type | This is additive, not breaking. Existing `Conflict` documentation remains. |
