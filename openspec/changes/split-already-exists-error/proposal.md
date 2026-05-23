## Why

The `AppError::Conflict` variant currently conflates two semantically different errors: **duplicate resource creation** ("this object already exists") and **optimistic concurrency version mismatch** ("the resource changed since you read it"). Both return HTTP 409 with `expected`/`actual` fields, which makes sense for version conflicts but is misleading for duplicates (where `expected: 0, actual: 0`). API consumers cannot easily distinguish the two scenarios without parsing numeric fields, leading to confusing error messages and incorrect client retry logic.

## What Changes

- Introduce a new `AlreadyExists { kind: String, name: String }` error variant mapped to HTTP 409 with response code `"AlreadyExists"`
- Reserve `Conflict { expected: u64, actual: u64 }` exclusively for optimistic concurrency version mismatches
- Update all stores (InMemory, SQLite) to return `AlreadyExists` on duplicate `create` instead of `Conflict`
- Update the `SchemaHasObjects` variant to remain 409 (unchanged — it's a different conflict type)
- Update OpenAPI error documentation to reflect the new error variant

## Capabilities

### New Capabilities
- `already-exists-error`: A distinct error variant for duplicate resource creation with clear `kind` and `name` fields in the response

### Modified Capabilities
- `error-handling`: The `Conflict` error variant's requirement changes — it no longer covers duplicate creation, only version mismatches. The `NotFound` requirement for `create` on missing schema stays. A new requirement for `AlreadyExists` is added.
- `object-store`: The `create` method's duplicate scenario changes from returning `Conflict` to returning `AlreadyExists`
- `object-service`: The "create duplicate object" scenario changes from `Conflict` to `AlreadyExists`
- `openapi-spec`: Error response documentation updated to include `AlreadyExists` as a documented 409 response

## Impact

- `src/error.rs`: New `AlreadyExists` variant, updated `Display`, updated `IntoResponse`
- `src/store/memory.rs`: Duplicate check returns `AlreadyExists`
- `src/store/sqlite.rs`: Constraint violation on INSERT returns `AlreadyExists`
- `src/object/service.rs`: Test assertions updated
- `src/openapi/paths.rs`: OpenAPI error responses updated
- `openspec/specs/error-handling/spec.md`: Spec updated
- `openspec/specs/object-store/spec.md`: Spec updated
- `openspec/specs/object-service/spec.md`: Spec updated
- Client code handling 409 errors must now handle both `Conflict` and `AlreadyExists` codes

## Non-goals

- Adding new HTTP status codes — both variants remain 409
- Changing error handling architecture or adding new error types beyond `AlreadyExists`
- Adding `BadRequest` or `InvalidInput` variants (separate concern)
- Changing how `SchemaHasObjects` works
