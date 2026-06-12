## Context

`StoredObject` is the canonical persisted form in kapi. It has two user-defined payload fields: `spec` (desired state, user-written) and `status` (observed state, controller-written, optional). Both are currently typed as `SpecData`, a named-struct wrapper around `serde_json::Value`:

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SpecData {
    pub value: serde_json::Value,
}
```

The wrapper was introduced in the `rename-data-to-spec` change to give the field a type name. It was carried forward to `status` when the status subresource was added. The roadmap contains an open question on whether to rename it to `UserData`.

The wrapper has accumulated cost without delivering value:

1. **Wire-format asymmetry**: Request bodies do *not* include the wrapper. Clients send `{"color": "blue"}` directly into the `spec` field. Response bodies *do* include the wrapper. Clients read `obj.spec.value.color`. The same field has two access paths depending on direction.

2. **~116 references to maintain**: 17 manual struct literals (`SpecData { value: x }`) in test code, 30+ `.value` unwraps across `src/` and `tests/`, ~50 type references. Every storage read, every validation call, every comparison, every test assertion unwraps the wrapper to get at the inner value.

3. **Misleading name**: `status: Option<SpecData>` reads as "optional spec" rather than "optional status payload". The naming debate in the roadmap is downstream of the existence problem.

4. **No realized benefit**: Zero impl methods, zero extension fields, zero validation tied to the envelope. Six months pre-1.0 with no fields added despite the placeholder being explicitly designed for future extension.

5. **K8s-shape deviation**: Kubernetes uses inline `spec` and `status` objects, not envelopes. kapi is explicitly K8s-shaped-but-not-K8s-compatible per the project README. There is no model fidelity argument for keeping the envelope.

The kapi project is in dev phase, pre-1.0, with no external clients. The integration tests in `tests/` are the only consumers of the wire format. This is the right time to delete the envelope.

## Goals / Non-Goals

**Goals:**
- Remove the `SpecData` type from the codebase
- Make `StoredObject.spec` and `StoredObject.status` use `serde_json::Value` directly
- Make the wire format consistent: same shape on read and write, no `.value` indirection
- Remove the `SpecData` component from the OpenAPI spec
- Resolve the open roadmap question permanently
- Preserve all existing behavior: validation, generation, status, watch, OCC, transactions

**Non-Goals:**
- Changing SQLite column types (the on-disk JSON content is identical)
- Adding a new wrapper type with a different name (option B / rename)
- Restricting the JSON value shape via OpenAPI (the spec remains `{}` — any value)
- Adding envelope metadata fields (content_type, version, schema_ref)
- Changing the `transaction()` API
- Changing validation rules or meta-schema
- Implementing watch resume, finalizers, or other roadmap items
- Adding a `Clock` trait for deterministic testing

## Decisions

### Decision 1: Delete the wrapper, do not rename it

**Choice**: Remove `SpecData` entirely. Replace with `serde_json::Value` directly on `StoredObject`.

**Rationale**:
- The wrapper has no realized value (no methods, no extension fields after 6+ months).
- Renaming keeps the cost ledger identical. Deleting removes the cost.
- Inlining `Value` is a K8s-shape alignment: `spec` and `status` become plain JSON objects, not envelopes.
- The wrapper is unwrapped at every interesting code site (validation, comparison, storage). Deleting it removes ~30 unwrap sites that exist only to bypass the wrapper.

**Alternatives considered**:
- **Rename to `UserData` (the original roadmap question)**: Keeps the indirection, keeps the asymmetry, keeps the cost. The naming problem is a symptom; the existence is the disease.
- **Rename to `ObjectData` or `JsonEnvelope` (council-recommended alternatives)**: Same as above. Better name for a tax we keep paying.
- **Keep status quo**: The roadmap question stays open forever. The "SpecData used for status" cognitive tax compounds.

### Decision 2: Use `serde_json::Value` directly on `StoredObject`

**Choice**: `StoredObject.spec: serde_json::Value` and `StoredObject.status: Option<serde_json::Value>`.

**Rationale**:
- We use `serde_json::Value` from the `serde_json` crate (the standard de-facto JSON value type in the Rust ecosystem) because it provides `Debug + Clone + Serialize + Deserialize` out of the box. The wrapper adds nothing on top.
- The fields are domain-payload fields, not domain-modeled fields. They are validated by the registered JSON Schema at write time, but they are not Rust-modeled.
- Using a generic JSON value type matches how the data is actually used: passed through to schema validators, compared for generation bumping, persisted as JSON.

**Alternatives considered**:
- **Define a typed `Spec` and `Status` struct per kind**: Would require generic over kind. Out of scope and explicitly deferred to a future change.
- **Define a single typed `Spec` and `Status` struct with kind-specific fields**: The whole point of kapi is dynamic, schema-registered kinds. Static typing defeats this.

### Decision 3: OpenAPI components — drop `SpecData`, use unconstrained JSON for `spec` and `status`

**Choice**: Remove the `SpecData` component entirely. The `StoredObject` component declares `spec` and `status` as unconstrained JSON values (no schema reference, no `value` wrapper).

**Rationale**:
- The wrapper had a single purpose: produce a `value` key in the JSON. With the wrapper gone, the spec/status fields are direct JSON values.
- Unconstrained JSON is the right shape — the actual shape is determined by the user's registered schema, not the system.
- Removing the component simplifies the OpenAPI document. Code-gen clients no longer need to model an extra envelope type.

**OpenAPI shape**:
```json
{
  "StoredObject": {
    "type": "object",
    "properties": {
      "key": { "$ref": "#/components/schemas/ResourceKey" },
      "metadata": { "$ref": "#/components/schemas/ObjectMeta" },
      "system": { "$ref": "#/components/schemas/SystemMetadata" },
      "spec": { "description": "User-defined spec payload, validated against the kind's registered jsonSchema" },
      "status": {
        "nullable": true,
        "description": "Status subresource, managed via /status endpoint. Null for kinds without statusSchema."
      }
    },
    "required": ["key", "metadata", "system", "spec"]
  }
}
```

**Alternatives considered**:
- **Reference kind-specific components in StoredObject**: The dynamic per-kind components (`WidgetExampleIoStoredObject`) are still generated. The static `StoredObject` component is the generic shape, used for Schema responses and any place that needs to reference "any object". Keeping it unconstrained is correct.
- **Inline the spec as `{}` (any value) in the static StoredObject**: Same as above. Use `description` for documentation.

### Decision 4: SQLite column content is identical

**Choice**: The `spec` and `status` columns continue to store JSON-stringified values. The bytes on disk are identical to the current implementation, since the wrapper's only role was to add a `value` key in the JSON, and the stringified content is always the inner `value`.

**Rationale**:
- Existing test fixtures work without DB migration.
- The `serde_json::to_string(&object.spec)` call (where `object.spec` is now `Value` directly) produces the same bytes as the previous `serde_json::to_string(&object.spec.value)`.

**Alternatives considered**:
- **Add a SQLite migration to drop and re-add the column**: Unnecessary. The on-disk format is unchanged.
- **Change the column type**: Not needed. The column is `TEXT` storing JSON, and that does not change.

### Decision 5: Request bodies and response bodies use the same shape

**Choice**: The `POST /apis/...` and `PUT /apis/.../{name}` request bodies already use the unwrapped shape (`{"color": "blue"}` inside the spec field). The response bodies change to match. After this change, request and response shapes are identical for the same field.

**Rationale**:
- Eliminates the read/write asymmetry.
- Clients now write `obj.spec.color` in all code paths.
- The handler body extraction logic is unchanged — it was already stripping `metadata` and `status` from the body, and treating the rest as the spec payload.

**Alternatives considered**:
- **Keep the wire format asymmetric and rename the wrapper**: Worse. The asymmetry is the user-facing bug; renaming does not fix it.

### Decision 6: Generation comparison uses `Value` equality

**Choice**: The generation-bump logic (in `apply_with_metadata()`) compares `new_obj.spec != existing.spec` instead of `new_obj.spec.value != existing.spec.value`.

**Rationale**:
- `serde_json::Value` implements `PartialEq`. Direct comparison works.
- One fewer `.value` access at this call site.

**Alternatives considered**:
- **Compare serialized form**: Equivalent to `PartialEq` on `Value`, but less idiomatic.

### Decision 7: Per-kind spec component is the user's specSchema directly

**Choice**: The `build_kind_spec_component` function in `src/openapi/components.rs` currently produces a component shaped as `{ "type": "object", "properties": { "value": <userSpecSchema> }, "required": ["value"] }` — i.e. it wraps the user's specSchema in a `value` envelope. After this change, the function returns the user's specSchema directly as the component value, with no `value` wrapper.

**Rationale**:
- The wrapper currently has zero effect on the wire format (the user's specSchema is what gets stored, not the wrapper) and zero effect on validation. It exists *only* in the OpenAPI generation, where it forces swagger UI to display the user's fields one level deeper than necessary.
- Today, swagger UI for `WidgetExampleIo` shows: `value [object] → color, size`. After the change, it shows: `color, size` at the top level. This is a direct UX win.
- The wrapper also creates an inconsistency between the create-request body schema (which is `build_create_request_schema` and *does not* wrap) and the response `StoredObject.spec` schema (which references `WidgetExampleIo` and *does* wrap). After this change, both the create-request shape and the response shape show the user's fields at the same level.

**OpenAPI shape (per-kind, after change)**:
```json
{
  "WidgetExampleIo": {
    "type": "object",
    "properties": {
      "color": { "type": "string" },
      "size":  { "type": "integer" }
    }
  }
}
```

(versus today, which is `{ "type": "object", "properties": { "value": { "type": "object", "properties": { "color": ..., "size": ... } } }, "required": ["value"] }`)

**Alternatives considered**:
- **Keep the wrapper for the per-kind component only**: Inconsistent — the static `StoredObject.spec` would be unwrapped but the per-kind `WidgetExampleIo` would be wrapped. Worse UX, no benefit.
- **Add a `value` wrapper to the create request body for symmetry**: Worse — would force clients to send `{ "metadata": ..., "value": { "color": ..., "size": ... } }` instead of the current `{ "metadata": ..., "color": ..., "size": ... }`. Breaks the existing wire format more invasively.

### Decision 8: Swagger UI coherence is a verification step, not a separate deliverable

**Choice**: The swagger UI itself (`src/openapi/swagger.rs`) is HTML+JS that loads from CDN and renders whatever OpenAPI spec it gets from `/openapi`. It has no hardcoded references to `SpecData`. The swagger UI "breaks" only in the sense that the OpenAPI spec it consumes is being changed.

**Rationale**:
- The HTML/JS in `src/openapi/swagger.rs` is agnostic to the spec content. No changes needed there.
- Coherence is a property of the *generated spec*, not the UI shell. The verification is: after the change, the generated spec at `/openapi` has no `SpecData` component and no `value` wrappers anywhere. Swagger UI will then display user fields at the top level.
- A coherence check is added to the tasks (Section 10a): manually inspect `/openapi` and `/swagger-ui/` after the change to confirm there is no `value` indirection visible to users.

## Risks / Trade-offs

**[Risk] Wire format break for any existing consumer** → Mitigation: There are no external consumers. The integration tests in `tests/src/` are the only consumers and they are updated in the same change. Document the break in the release notes as a single line: "Wire format: `spec` and `status` are now inline JSON values; the `{value: ...}` envelope is gone."

**[Risk] Future need for envelope metadata (version, content_type, schema_ref)** → Mitigation: When/if that need arrives, reintroduce a wrapper with the actual fields. YAGNI for now. The cost of reintroducing is mechanical and small (one struct, ~17 literals, one OpenAPI component). Document this decision in the proposal's Future Work section.

**[Risk] Test boilerplate from updating 27+ integration test assertions** → Mitigation: All updates are mechanical (`["spec"]["value"]["x"]` → `["spec"]["x"]`, `{"value": {...}}` → `{...}`). The tests assert on the same data shape they did before, just with one less level of nesting. Pure textual refactor.

**[Risk] OpenAPI breaking change for code-gen clients** → Mitigation: There are no code-gen clients in this dev-phase repo. The breaking change is a feature, not a bug — the previous `SpecData` component was a code-gen wart.

**[Risk] Forgetting a `.value` access somewhere and introducing a runtime bug** → Mitigation: `cargo build` will fail on any `SpecData` reference, which is the only type-level error possible. `cargo test` will catch any missed `["spec"]["value"]` JSON path. No semantic changes are possible from this refactor.

**[Trade-off] Loss of "future extensibility hook"** → Acceptable. The hook has not been used in 6+ months. Reintroducing when needed is mechanical.

**[Trade-off] The renaming question is answered permanently** → Acceptable. The "rename `SpecData` to `UserData`?" question is removed from the roadmap as part of this change.

## Migration Plan

This is a pure refactor with no data migration. The order of changes:

1. Update `src/object/types.rs`: remove `SpecData` struct, change `StoredObject` field types to `Value`/`Option<Value>`, update `test_stored_object` helper.
2. Update `src/object/service.rs`: remove `SpecData` references, pass `Value` directly, update generation comparison.
3. Update `src/object/handler.rs`: change `get_status` return type to `Json<Option<Value>>`.
4. Update `src/store/memory.rs`: store `Value` directly, no envelope construction.
5. Update `src/store/sqlite.rs`: store `Value` directly, no envelope construction; the SQL queries and column types are unchanged.
6. Update `src/schema/registry.rs`: `serde_json::from_value(schema_obj.spec.clone())` instead of `... schema_obj.spec.value`.
7. Update `src/openapi/components.rs`: remove the `SpecData` component entry, update the `StoredObject` component to declare `spec` and `status` as unconstrained JSON.
8. Update `src/openapi/paths.rs`: replace `spec_data_ref` with direct unconstrained JSON for spec/status in path schemas.
9. Update `src/openapi/mod.rs`: remove `SpecData` from any component listing.
10. Update `tests/src/object_crud.rs`, `tests/src/status_subresource.rs`, `tests/src/watch_events.rs`, `tests/src/optimistic_concurrency.rs`, `tests/src/generation_semantics.rs`: drop `.value` from JSON paths in request bodies and assertions.
11. Update `openspec/specs/core-types/spec.md`, `openspec/specs/openapi-spec/spec.md` via the delta specs in this change.
12. Update `docs/data-model.md` and `docs/api-reference.md`.
13. Update `roadmap.md`: remove the "Should we rename the struct `SpecData` to `UserData`?" line.
14. Run `cargo build && cargo test` until green.
15. Run `cargo clippy --all-targets --all-features -- -D warnings`.

**Rollback**: Revert the change. No data migration needed — the on-disk SQLite format is byte-identical.

## Open Questions

None. The change is mechanical; all design decisions are resolved through the council review and exploration session.
