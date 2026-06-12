## Why

Both store implementations (`InMemoryStore` and `SQLiteStore`) duplicate "smart" logic: maintaining a global `AtomicU64` counter for `resource_version`, bumping `resource_version` and `updated_at` in `transaction()`, and hardcoding `generation = 1` in `create()`. Meanwhile, the service layer performs a code smell — it preserves `resource_version` and `created_at` from `existing` inside transaction callbacks, knowing the store will overwrite them anyway. This violates separation of concerns: the store should be pure persistence, not business logic.

## What Changes

- **BREAKING**: `ObjectStore::create()` signature changes from `create(key, meta, spec)` to `create(object: StoredObject)` — the store persists what it's given
- Store implementations no longer maintain a global `AtomicU64` counter for `resource_version`
- Store implementations no longer bump `resource_version` or `updated_at` in `transaction()` Apply arm
- Store implementations no longer set `generation = 1` in `create()`
- `SQLiteStore::init_version_counter()` is removed (no longer needed)
- Service layer takes ownership of all system metadata manipulation (rv, generation, timestamps)
- Resource version becomes per-object (starts at 1, increments independently) instead of global
- Service introduces a centralized metadata wrapper that handles rv increment, generation bump (if spec changed), and timestamp updates
- OCC (optimistic concurrency control) check moves from store to service layer (in transaction callback)
- `TransactionOp::Apply` doc comment updated to reflect that store no longer auto-bumps

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `object-store`: Trait signature changes (`create` accepts `StoredObject`), store becomes pure persistence layer with no metadata logic
- `transaction`: `TransactionOp::Apply` no longer auto-bumps `resource_version` or `updated_at` — store persists object as-is
- `object-service`: Service takes ownership of system metadata (rv, generation, timestamps), introduces centralized wrapper, performs OCC check in transaction callback

## Impact

- **Breaking API change**: `ObjectStore::create()` signature changes, affecting all callers
- **Store implementations**: Both `InMemoryStore` and `SQLiteStore` lose `AtomicU64`, `next_version()`, `init_version_counter()`, and metadata bumping logic
- **Service layer**: Gains responsibility for rv, generation, timestamps, and OCC check
- **Tests**: Store tests must construct full `StoredObject` instead of relying on store to populate metadata; service tests verify metadata behavior through service API
- **Integration tests**: Tests that bypass service and call store directly must construct `SystemMetadata`
- **Documentation**: `TransactionOp::Apply` and `SystemMetadata.resource_version` doc comments must be updated
- **Future watch resume**: Must use event-level sequence numbers (not object `resource_version`) for global ordering

## Non-goals

- Implementing watch resume with event-level sequence numbers (separate future work)
- Adding optimistic concurrency control as a store-level feature (OCC remains in service)
- Changing the `transaction()` callback signature (callbacks still return `TransactionOp`)
- Implementing a `Clock` trait for test determinism (future improvement)

## Future Work

- Watch resume implementation will need an event-level sequence number in the event bus (separate from object `resource_version`)
- Consider adding a `Clock` trait for deterministic testing of timestamps
