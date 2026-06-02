## Context

`ObjectService` in `src/object/service.rs` is the single orchestrator for all CRUD operations. Each of the 6 CRUD call sites (create-schema, create-object, update-schema, update-object, delete-schema, delete-object) contains an identical 5-line block to construct a `WatchEvent` and publish it via `event_bus.publish()`.

Current pattern repeated 6 times:
```rust
self.event_bus.publish(
    &key,
    WatchEvent {
        event_type: WatchEventType::Added,  // or Modified/Deleted
        object: stored.clone(),
    },
);
```

## Goals / Non-Goals

**Goals:**
- Eliminate duplicated `WatchEvent` construction across all 6 call sites
- Single point of definition for event publishing logic
- Zero behavioral change — identical semantics, same tests pass

**Non-Goals:**
- No broader CRUD orchestration abstraction (closure/trait approaches rejected)
- No changes to `EventPublisher` trait or `EventBus` implementation
- No new public API surface

## Decisions

### Decision: Private method on `ObjectService`, not a free function

**Rationale:** The helper needs access to `self.event_bus`, which is a private field. A private method keeps the coupling explicit and avoids passing `event_bus` as a parameter.

**Alternatives considered:**
1. **Free function** — would require `&dyn EventPublisher` parameter, adds indirection for no benefit
2. **Inherent method on `EventPublisher` trait** — would change the trait's public surface for an internal convenience
3. **Macro** — overkill for 5 lines, hurts readability

### Decision: Signature takes `&StoredObject`, not `StoredObject`

**Rationale:** The caller already has a `StoredObject` and calls `.clone()` for the `WatchEvent`. Moving the `.clone()` into the helper keeps call sites clean and makes the ownership transfer explicit at the single definition site.

## Risks / Trade-offs

| Risk | Mitigation |
|------|------------|
| Minimal — this is a mechanical extraction with no logic change | Existing tests (T19–T33) validate identical behavior |
| Future readers might expect more abstraction | Proposal and design document why broader abstraction was rejected |
