## Context

The create handler currently accepts a flat request body where domain fields sit at the top level alongside `metadata`. The handler strips `metadata` and `status`, treating the remainder as `spec`. This creates asymmetry with GET and PUT, which return/expect the full `StoredObject` shape with nested `spec`.

Current flow:
```
POST body: { metadata, color, size }
  → strip metadata/status
  → remainder becomes spec
  → StoredObject { metadata, spec: { color, size }, ... }
```

This also means typos in field names are silently accepted (e.g., `"colr": "blue"` becomes part of spec without warning).

## Goals / Non-Goals

**Goals:**
- Symmetric API: create, read, update all use the same `spec` structure
- Explicit request shape: `spec` is a first-class field, not implicit remainder
- Strict validation: reject unknown top-level fields, reject empty spec
- Proper error codes: client input errors return 400, not 500

**Non-Goals:**
- Changing Schema object creation (remains flat — it's a control-plane operation)
- Changing update (PUT) request shape (already uses `spec`)
- Changing internal `StoredObject` structure
- Adding PATCH / partial update support

## Decisions

### Decision 1: Require `spec` field on create

**Choice**: POST body must contain `spec` field with domain data.

**Alternatives considered:**
1. Keep flat format (current) — rejected: creates asymmetry, hides typos
2. Accept both flat and nested — rejected: ambiguous, more validation complexity
3. Require `spec` (chosen) — symmetric with GET/PUT, explicit, matches K8s convention

**Rationale**: The update handler already expects `spec`. Making create consistent reduces cognitive load. The meta-schema already uses `unevaluatedProperties: false` for Schema objects — this extends that strictness to regular objects.

### Decision 2: Reject unknown top-level fields

**Choice**: Only `metadata` and `spec` allowed at top level. Any other field → 400 Bad Request.

**Alternatives considered:**
1. Silently ignore unknown fields — rejected: hides typos, current behavior is problematic
2. Warn but accept — rejected: no partial validation in REST APIs
3. Reject unknown fields (chosen) — matches meta-schema convention, catches errors early

**Rationale**: The meta-schema for Schema objects already uses `unevaluatedProperties: false`. This extends the same strictness to regular object creation.

### Decision 3: New error variant `InvalidRequestBody`

**Choice**: Add `AppError::InvalidRequestBody(String)` → HTTP 400.

**Alternatives considered:**
1. Reuse `InvalidLabel` — rejected: semantically wrong, not label-specific
2. Reuse `InvalidSchema` — rejected: that's for schema registration, maps to 422
3. Use `Internal` — rejected: client errors should not be 500
4. New `InvalidRequestBody` (chosen) — clear semantics, proper HTTP code

**Rationale**: Currently, missing `metadata.name` returns HTTP 500 via `AppError::Internal`. This is wrong — it's a client error. The new variant covers: missing `spec`, non-object `spec`, empty `spec`, unknown fields.

### Decision 4: Validate `spec` is an object before checking non-empty

**Choice**: Check `spec.is_object()` first, then check non-empty.

**Rationale**: Defense-in-depth with clear error messages. If `spec` is an array or string, say so. If it's `{}`, say it's empty.

## Risks / Trade-offs

**[Breaking change]** → Mitigation: This is pre-1.0, breaking changes are expected. Tests will be updated as part of this change.

**[Test churn]** ~33 test bodies need updating → Mitigation: Most (21) are fixed by updating `widget()` helper. Remaining 12 are inline in `status_subresource.rs`.

**[Schema objects exempt]** Could cause confusion → Mitigation: Schema creation is a rare admin operation with its own meta-schema validation. The flat format is already established and documented.
