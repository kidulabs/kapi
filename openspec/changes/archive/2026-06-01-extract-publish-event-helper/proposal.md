## Why

The `ObjectService` repeats the same 5-line `WatchEvent` construction and `event_bus.publish()` call at every CRUD call site (6 locations). Extracting this into a single `publish_event` helper eliminates ~30 lines of duplicated boilerplate with zero abstraction cost and zero behavioral change.

## What Changes

- Add a private `publish_event(&self, key: &ResourceKey, event_type: WatchEventType, object: &StoredObject)` method to `ObjectService`
- Replace all 6 inline `event_bus.publish(...)` call sites with `self.publish_event(...)`
- No behavioral changes — identical logic, single point of definition

## Capabilities

### New Capabilities
<!-- None - this is a pure internal refactoring -->

### Modified Capabilities
<!-- None - no requirements or external behavior changes -->

## Impact

- **Affected code**: `src/object/service.rs` only — `ObjectService` impl block
- **APIs**: None — internal private method
- **Dependencies**: None
- **Tests**: Existing tests continue to pass unchanged (behavior is identical)

## Non-goals

- No broader abstraction of CRUD orchestration (closure-based or trait-based approaches were evaluated and rejected)
- No changes to event bus, schema registry, or store interfaces
- No new capabilities or features

## Future Work

- Revisit CRUD abstraction if a third kind type is added (e.g., `ConfigMap`, `Policy`), at which point the object path naturally generalizes
