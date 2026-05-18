## Why

Schema registration currently requires the client to supply the object name via `metadata.name` in the request body, and the name must follow the convention `{targetKind}.{targetGroup}`. This is redundant — the name is fully derivable from the payload's own `targetKind` and `targetGroup` fields. It also creates a latent bug: if the client sends a name that doesn't match the convention, the schema cache key (generated from payload fields) won't match the stored name, causing subsequent object lookups to fail.

## What Changes

- **Schema registration (POST)** becomes nameless — the client no longer sends `metadata.name`. The backend generates the name as `{targetKind}.{targetGroup}` and stores it in `metadata` internally.
- **Schema fetch, update, delete (GET/PUT/DELETE)** continue to use the name as the URL path parameter — the generated name is returned in response `metadata`.
- **Handler name extraction** branches on `kind == "Schema"`: for Schema, extract `targetKind` and `targetGroup` from the body and generate the name; for regular objects, continue extracting from `metadata.name`.
- **Missing fields error early** — if `targetKind` or `targetGroup` is absent from a Schema registration payload, return `AppError::InvalidSchema` immediately.
- **Schema validation** is deferred to a future change — the current change focuses only on name generation in the registration flow.

## Capabilities

### New Capabilities
- `schema-name-generation`: Backend-generated schema names from `{targetKind}.{targetGroup}` during registration

### Modified Capabilities
- `object-handlers`: Create handler requirement changes — for Schema kind, name is generated from payload fields instead of extracted from `metadata.name`
- `object-service`: Schema create path no longer receives a client-supplied name; the name is always derived from payload fields

## Impact

- `src/object/handler.rs` — create handler name extraction logic
- `src/object/service.rs` — test updates (tests currently pass hardcoded names)
- `openspec/specs/object-handlers/spec.md` — delta spec for create handler
- `openspec/specs/object-service/spec.md` — delta spec for schema create flow
