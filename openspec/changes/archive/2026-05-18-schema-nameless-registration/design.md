## Context

The current schema registration flow requires clients to supply a `metadata.name` field in the request body, and that name must follow the convention `{targetKind}.{targetGroup}`. This name is used as the storage key and cache key. The convention is implicit — not enforced by the server — which creates a latent bug: if a client sends a mismatched name, the schema cache (keyed by payload-derived `{targetKind}.{targetGroup}`) won't match the stored name, and subsequent object validation lookups will fail.

The meta-schema (`META_SCHEMA_JSON`) already does not require `metadata` — it only requires `targetGroup`, `targetVersion`, `targetKind`, and `jsonSchema`. The `metadata.name` extraction is a handler-layer concern, not a schema validation concern.

## Goals / Non-Goals

**Goals:**
- Eliminate redundant client-supplied name in Schema registration
- Generate schema name deterministically as `{targetKind}.{targetGroup}` in the handler
- Fix the latent cache key mismatch bug by ensuring name and cache key always agree
- Return the generated name in response `metadata` so clients can reference it
- Fail fast with `InvalidSchema` error if `targetKind` or `targetGroup` is missing

**Non-Goals:**
- No changes to regular object CRUD — objects still require user-supplied `metadata.name`
- No changes to Schema update, delete, or get semantics — they still use name as URL parameter
- No changes to the meta-schema validation rules
- Schema validation (the feature itself) is deferred to a future change

## Decisions

### Decision 1: Name generation in handler, not service

The name is generated in `handler.rs:create()` before calling `ObjectService::create()`. This keeps the service interface unchanged — it still receives a `name: String` parameter. The handler is the natural place to branch on `kind == "Schema"` because it already has access to the path `kind` and the request body.

**Alternatives considered:**
- Generate name in the service: Would require changing the service signature or adding a special case inside `create()`. The handler already branches on kind for routing logic, so keeping it there is more cohesive.
- Accept `Option<String>` for name: Would complicate the service API and push the "when is it None?" question downstream.

### Decision 2: Early validation with `InvalidSchema` error

If `targetKind` or `targetGroup` is missing from a Schema registration body, the handler returns `AppError::InvalidSchema` immediately. This is consistent with the meta-schema validation that happens later — both are schema registration errors.

**Alternatives considered:**
- Return `Internal` error: Would conflate a client error with a server error.
- Let it flow to meta-schema validation: The meta-schema doesn't know about name generation, so it wouldn't catch this specific case.

### Decision 3: Name format `{targetKind}.{targetGroup}`

The format matches the existing convention used throughout the codebase (cache keys, schema lookups). No change to the naming convention — just who generates it.

## Risks / Trade-offs

- **[Risk]** Existing clients that send `metadata.name` in Schema registration payloads will have that field stripped (removed from body before passing to service). The generated name will be used instead. **Mitigation:** This is a behavioral change but produces the same result if the client was following the convention correctly.
- **[Risk]** If `targetKind` or `targetGroup` contains characters that are invalid for URL path segments (e.g., `/`, `?`), the generated name could be problematic. **Mitigation:** The meta-schema already requires `minLength: 1`. Additional character validation could be added later if needed.
- **[Trade-off]** The handler now needs to understand Schema-specific logic (extracting `targetKind`/`targetGroup`). This couples the handler to the Schema payload shape, but this is already the case with the meta-schema validation in the service.
