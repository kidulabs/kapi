## Context

The kapi project has a layered architecture: `Handler → ObjectService → ObjectStore`. Currently, both store implementations (`InMemoryStore` and `SQLiteStore`) maintain a global `AtomicU64` counter for `resource_version`, bump `resource_version` and `updated_at` in `transaction()`, and hardcode `generation = 1` in `create()`. The service layer performs a code smell — it preserves `resource_version` and `created_at` from `existing` inside transaction callbacks, knowing the store will overwrite them anyway.

This violates separation of concerns: the store should be pure persistence, not business logic. The service layer should own all system metadata manipulation.

## Goals / Non-Goals

**Goals:**
- Make store implementations pure persistence layers (read bytes, write bytes, delete bytes)
- Eliminate duplication of metadata logic across store implementations
- Centralize system metadata manipulation (rv, generation, timestamps) in the service layer
- Switch from global `resource_version` to per-object `resource_version`
- Move OCC (optimistic concurrency control) check from store to service layer
- Eliminate the "preserve metadata" code smell in service callbacks

**Non-Goals:**
- Implementing watch resume with event-level sequence numbers (separate future work)
- Adding a `Clock` trait for deterministic testing (future improvement)
- Changing the `transaction()` callback signature (callbacks still return `TransactionOp`)
- Implementing multi-object transactions

## Decisions

### Decision 1: Per-object resource_version instead of global

**Choice**: Each object's `resource_version` starts at 1 and increments independently per object.

**Rationale**: 
- Eliminates the need for a global `AtomicU64` counter in stores
- Makes stores truly stateless (no global state to maintain)
- Per-object rv is more useful for optimistic concurrency control
- Watch resume (future work) should use event-level sequence numbers, not object rv

**Alternatives considered**:
- **Keep global rv**: Would require stores to maintain state, violating the "dumb store" goal
- **Hybrid approach (global + per-object)**: Adds complexity without clear benefit

### Decision 2: Change `create()` signature to accept `StoredObject`

**Choice**: `ObjectStore::create()` changes from `create(key, meta, spec)` to `create(object: StoredObject)`.

**Rationale**:
- Store no longer needs to know how to construct a `StoredObject`
- Service constructs the complete object with all metadata before calling store
- Consistent with the "store is pure persistence" philosophy
- Eliminates the need for store to set `generation = 1` or timestamps

**Alternatives considered**:
- **Keep current signature, have service set metadata on returned object**: Creates awkward flow where store creates object, then service mutates it. Less clean than service creating the complete object upfront.

### Decision 3: Centralized metadata wrapper in service

**Choice**: Service introduces a helper function that wraps transaction callbacks and automatically handles rv increment, generation bump (if spec changed), and timestamp updates.

**Rationale**:
- Eliminates the "update_status landmine" — generation is automatically preserved when spec doesn't change
- Single place to get metadata logic right, not scattered across callbacks
- Callbacks focus purely on domain changes (change spec, change status)
- Future callback authors only need to think about domain logic

**Implementation**:
```rust
fn apply_with_metadata<F>(existing: &StoredObject, mutator: F) -> TransactionOp
where
    F: FnOnce(&StoredObject) -> StoredObject,
{
    let mut new_obj = mutator(existing);
    new_obj.system.resource_version = existing.system.resource_version + 1;
    new_obj.system.updated_at = Utc::now();
    new_obj.system.created_at = existing.system.created_at;
    if new_obj.spec.value != existing.spec.value {
        new_obj.system.generation = existing.system.generation + 1;
    } else {
        new_obj.system.generation = existing.system.generation;
    }
    TransactionOp::Apply(new_obj)
}
```

**Alternatives considered**:
- **Each callback handles its own metadata**: Scatters logic, easy to forget, update_status landmine remains
- **Store handles metadata**: Violates "dumb store" principle, duplicates logic across implementations

### Decision 4: OCC check in service layer (transaction callback)

**Choice**: Service performs OCC check inside the transaction callback, returning `TransactionOp::Abort(Conflict)` if version doesn't match.

**Rationale**:
- Store remains dumb — just executes callback
- Service owns the policy (reject if stale)
- Callback runs while store holds lock → atomic
- Not all transactions need OCC (opt-in per operation)
- Consistent with "dumb store" philosophy

**Alternatives considered**:
- **Store does OCC check**: Store would need to know about resource_version, violating "dumb store" principle
- **OCC outside transaction**: Not atomic — race condition between check and update

### Decision 5: Remove `update()` and `update_status()` from trait (already done)

**Choice**: The trait already uses `transaction()` for all mutations. No separate `update()` or `update_status()` methods.

**Rationale**: Already implemented in a previous change. The `transaction()` method provides a single, composable API for all mutations.

## Risks / Trade-offs

**[Risk] Breaking API change to `ObjectStore::create()`** → Mitigation: Update all callers (service, tests) in the same change. Add `StoredObject::new()` or `SystemMetadata::initial()` helper to reduce boilerplate.

**[Risk] Test boilerplate explosion** → Mitigation: Add `fn test_stored_object(key, name, spec) -> StoredObject` helper immediately. All tests use this helper.

**[Risk] Integration tests bypass service** → Mitigation: Update integration tests to construct `SystemMetadata` when calling store directly. Document that tests should prefer service API when possible.

**[Risk] Doc comments become lies** → Mitigation: Update `TransactionOp::Apply` and `SystemMetadata.resource_version` doc comments to reflect new behavior.

**[Risk] External tooling assumes global rv** → Mitigation: Document that `resource_version` is per-object. Watch resume (future work) will use event-level sequence numbers.

**[Trade-off] Per-object rv loses cross-object causal ordering** → Acceptable because watch resume will use event-level sequence numbers. Per-object rv is more useful for OCC.

**[Trade-off] Service owns the clock** → Acceptable because `Utc::now()` isn't I/O. Consider adding a `Clock` trait later for test determinism.

## Migration Plan

1. Add `SystemMetadata::initial()` or `StoredObject::new()` helper
2. Change `ObjectStore::create()` signature to accept `StoredObject`
3. Update both store implementations to just persist what they're given
4. Remove `AtomicU64`, `next_version()`, `init_version_counter()` from stores
5. Remove rv bumping and timestamp generation from `transaction()` Apply arm
6. Add centralized metadata wrapper in service
7. Update all service callbacks to use the wrapper
8. Move OCC check to service layer (in transaction callback)
9. Update tests (store tests, service tests, integration tests)
10. Update doc comments

**Rollback**: Revert the change. No data migration needed (per-object rv is stored in the object, existing databases work fine).

## Open Questions

None. All design decisions have been resolved through exploration and council review.
