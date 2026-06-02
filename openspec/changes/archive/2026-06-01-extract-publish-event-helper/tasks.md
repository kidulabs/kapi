## 1. Extract publish_event helper

- [x] 1.1 Add private `publish_event(&self, key: &ResourceKey, event_type: WatchEventType, object: &StoredObject)` method to `ObjectService` impl block in `src/object/service.rs`
- [x] 1.2 Replace inline `event_bus.publish(...)` in `validate_and_create_schema` with `self.publish_event(...)`
- [x] 1.3 Replace inline `event_bus.publish(...)` in `validate_and_create_object` with `self.publish_event(...)`
- [x] 1.4 Replace inline `event_bus.publish(...)` in `validate_and_update_schema` with `self.publish_event(...)`
- [x] 1.5 Replace inline `event_bus.publish(...)` in `validate_and_update_object` with `self.publish_event(...)`
- [x] 1.6 Replace inline `event_bus.publish(...)` in `delete` (regular object path) with `self.publish_event(...)`
- [x] 1.7 Replace inline `event_bus.publish(...)` in `delete_schema` with `self.publish_event(...)`

## 2. Verify

- [x] 2.1 Run `cargo check` — no compilation errors
- [x] 2.2 Run `cargo clippy` — no new warnings
- [x] 2.3 Run `cargo test` — all existing tests pass (T19–T33 + label tests)
