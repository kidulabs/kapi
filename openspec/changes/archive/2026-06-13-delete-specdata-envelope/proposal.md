## Why

`StoredObject.spec` and `.status` are typed as `SpecData`, a wrapper `{ value: serde_json::Value }`. The wrapper exists purely as a wire-format indirection — no methods, no impl block, no extension fields, no validation. After 6+ months pre-1.0 it has not gained a single field.

The wrapper is also a wire-format bug. Request bodies are unwrapped (clients send `{"color":"blue"}` directly into the spec field), but response bodies are wrapped (clients read `obj.spec.value.color`). The same field has two access paths depending on direction.

The roadmap contains an open question on renaming `SpecData` → `UserData`. The right answer is not to rename the indirection — it is to delete it. Inlining `serde_json::Value` directly eliminates the asymmetry, removes ~116 references to a struct that contributes nothing to domain logic, and aligns the wire format with K8s-shaped resources (where `spec` and `status` are inline JSON, not envelopes).

This is a wire-format-breaking change. It is acceptable now: kapi is pre-1.0, there are no external clients, and the integration tests are the only consumers. Carrying the envelope to 1.0 is much more expensive than removing it now.

## What Changes

- **BREAKING**: Remove the `SpecData` struct. `StoredObject.spec` becomes `serde_json::Value`. `StoredObject.status` becomes `Option<serde_json::Value>`.
- **BREAKING**: Wire format: response bodies drop the `.value` wrapper. `obj.spec.value.color` becomes `obj.spec.color`. Same for `status`.
- **BREAKING**: OpenAPI: remove the `SpecData` component. `StoredObject.spec` and `.status` become unconstrained JSON.
- **BREAKING**: `get_status` returns `Json<Option<serde_json::Value>>` instead of `Json<Option<SpecData>>`.
- Remove ~17 `SpecData { value: x }` literals and ~30 `.value` unwrap sites. Update integration tests, `docs/`, and `roadmap.md` (remove the rename question).

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `core-types`: `StoredObject.spec` is `serde_json::Value`; `.status` is `Option<serde_json::Value>`. The `SpecData` type does not exist.
- `object-store`: Both stores persist `serde_json::Value` directly. SQLite column contents are byte-identical to today.
- `object-service`: Service methods accept and return `serde_json::Value`. `apply_with_metadata` compares `spec` directly via `Value` equality.
- `object-handlers`: `get_status` returns `Json<Option<serde_json::Value>>`.
- `openapi-spec`: Remove the `SpecData` component. `StoredObject` references unconstrained JSON for `spec` and `status`.

## Impact

- **Wire format**: Breaking. `{"spec":{"value":{...}}}` becomes `{"spec":{...}}`. No external clients exist; integration tests are updated in the same change.
- **Code**: Pure structural refactor. ~116 references collapse. No logic changes — validation, generation, status, watch, OCC, transactions all behave identically.
- **Storage**: SQLite column contents are byte-identical. No DB migration needed.
- **OpenAPI**: One component removed. Two `$ref`s updated.

## Non-goals

- Reintroducing a wrapper with a different name (the indirection is being deleted, not renamed).
- Adding envelope metadata fields (content_type, schema_version) — YAGNI.
- Changing the SQLite schema, `transaction()` API, meta-schema, or `SchemaData`.
- Implementing watch resume, finalizers, or other roadmap items.
- Adding a `Clock` trait for deterministic timestamp testing.

## Future Work

- If a real need arises for envelope metadata, reintroduce a wrapper at that time with the actual fields. Do not pre-emptively recreate the placeholder.
- Codify a review principle: "no newtype wrappers around `serde_json::Value` without a realized use case."
