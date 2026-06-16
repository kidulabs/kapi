# Finalizer Support

## Problem

kapi's current `delete()` is a hard delete — the object is immediately removed from storage. This works for stateless objects, but fails for objects with external side effects (cloud resources, database rows, sidecars that need cleanup).

We need an async hand-off protocol between "user wants this gone" and "it's actually gone." Finalizers provide this: controllers register interest in cleanup by adding a finalizer string, and the API server defers deletion until all finalizers are removed.

## Solution

Add finalizer support to kapi's object lifecycle:

1. **Data model**: Add `finalizers: Vec<String>` to `ObjectMeta` and `deletion_timestamp: Option<DateTime<Utc>>` to `SystemMetadata`
2. **DELETE semantics**: If `finalizers` is empty, hard delete. If non-empty, set `deletion_timestamp` and return the object (mark for deletion).
3. **UPDATE semantics**: When `deletion_timestamp` is set, only allow `finalizers` changes. When `finalizers` becomes empty, hard delete.
4. **Validation**: Reuse label-key validation for finalizer names. Max 20 finalizers per object.
5. **Events**: Publish `Modified` for mark-for-deletion, `Deleted` for hard delete. Suppress events on idempotent DELETE retries.

## Scope

### In Scope (v1)

- Regular objects (not Schema objects)
- PUT-only finalizer removal (no PATCH dependency)
- Label-key-shaped finalizer names (reuse `validate_label_key`)
- Max 20 finalizers per object
- Idempotent DELETE (200 with object on retry)
- New `ObjectBeingDeleted` error variant (409 Conflict)
- Suppress Modified events on no-op DELETE retries

### Out of Scope (v1)

- Schema object finalizers (different delete path with `SchemaHasObjects` guard)
- `deletion_grace_period_seconds` (K8s feature, rarely used)
- Foreground vs background deletion propagation (background-only)
- Stuck-object GC / force-delete admin path
- Owner references / garbage collection cascade
- PATCH endpoint (deferred until PATCH is implemented)

## Key Decisions

### 1. `deletion_timestamp` placement: `SystemMetadata`

**Decision**: Server-maintained field in `SystemMetadata`, not `ObjectMeta`.

**Rationale**: The update path wholesale-replaces `metadata` (`service.rs:385-389`). If `deletion_timestamp` lives in `ObjectMeta`, every update must surgically preserve the server's value — a footgun. `SystemMetadata` is naturally owned by `apply_with_metadata`, like `created_at` and `resource_version`.

### 2. Schema objects: skip for v1

**Decision**: No finalizer support for Schema objects.

**Rationale**: Schema deletion already has a `SchemaHasObjects` referential integrity guard. Threading finalizer logic through creates an awkward two-gate system. Schemas are infrastructure registrations, not workloads with external cleanup. Adding finalizers to schemas later is a non-breaking extension.

### 3. Idempotent DELETE: 200 with object

**Decision**: Return 200 with the object on DELETE retry (when `deletion_timestamp` is already set).

**Rationale**: HTTP DELETE is idempotent (RFC 9110 §9.2.2). The object still exists in the store. Controllers need to see current state (which finalizers remain). Suppress the Modified event if nothing actually changed.

### 4. PATCH vs PUT: PUT-only for v1

**Decision**: PUT-only finalizer removal. Don't wait for PATCH.

**Rationale**: PATCH is "exploration" on the roadmap. Blocking finalizers on an uncertain timeline isn't worth it. When PATCH arrives, finalizer removal becomes nicer, but the data model doesn't change.

### 5. Finalizer name format: label-key-shaped

**Decision**: Reuse `validate_label_key` logic. Max 20 finalizers per object.

**Rationale**: The formats are functionally equivalent. kapi's `validate_label_key` is structurally identical to K8s finalizer format. Writing a separate regex is code duplication. The server treats finalizer names as opaque identifiers — it never parses them.

### 6. Error variant: new `ObjectBeingDeleted`

**Decision**: Add `AppError::ObjectBeingDeleted { name: String }` mapping to 409 Conflict.

**Rationale**: The error is semantically distinct from "your JSON is malformed." It's a state-based rejection. 409 Conflict is the right HTTP status (the object is in a conflicting state for this operation).

### 7. Event suppression: yes

**Decision**: Suppress Modified events on idempotent DELETE retries (when `deletion_timestamp` is already set and finalizers unchanged).

**Rationale**: The event bus is for state changes, not operation attempts. If nothing changed, no event. Avoids unnecessary controller wake-ups.

## Success Criteria

- [ ] Objects can be created with `finalizers` in metadata
- [ ] DELETE on object with finalizers sets `deletion_timestamp` and returns 200
- [ ] DELETE on object without finalizers hard-deletes immediately
- [ ] UPDATE on object with `deletion_timestamp` rejects non-finalizer changes (409)
- [ ] UPDATE that empties finalizers on a deleting object hard-deletes it
- [ ] Idempotent DELETE returns 200 without emitting spurious Modified events
- [ ] Finalizer names validated as label-key-shaped, max 20
- [ ] Existing objects without `finalizers` field deserialize correctly (backward compatibility)
- [ ] Integration tests cover all state transitions

## Risks

1. **Stuck objects**: A buggy controller that never removes its finalizer leaves the object in "deleting" state forever. Mitigation: document this clearly; add force-delete admin path in v2.

2. **Concurrency**: DELETE racing with controller removing finalizers. Mitigation: transaction-based update orders them correctly. Both outcomes are valid.

3. **Backward compatibility**: Existing SQLite data doesn't have `finalizers` field. Mitigation: `#[serde(default)]` on the field.

## Dependencies

- None. This is a self-contained change to the object lifecycle.

## Related Work

- **Status subresource**: Similar pattern — server-managed field (`deletion_timestamp`) in `SystemMetadata`, separate from client-settable `ObjectMeta`.
- **OCC (Optimistic Concurrency Control)**: Finalizer removal uses the same `resource_version`-based OCC as regular updates.
- **Schema deletion guard**: `SchemaHasObjects` is a different kind of deletion guard (referential integrity vs async cleanup).
