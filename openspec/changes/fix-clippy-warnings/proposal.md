## Why

Cargo clippy reports 3 warnings in the codebase that should be addressed to maintain code quality and follow Rust idioms.

## What Changes

- Implement `std::default::Default` for `EventBus` instead of defining a custom `default()` method
- Implement `std::default::Default` for `InMemoryStore` to satisfy the `new_without_default` lint
- Replace redundant closure `|t| decode_continue_token(t)` with the function reference `decode_continue_token`

## Capabilities

### New Capabilities

(None - this is pure lint cleanup with no new capabilities)

### Modified Capabilities

(None - no spec-level behavior changes)

## Impact

**Affected code:**
- `src/event/bus.rs`: `EventBus::default()` → `impl Default for EventBus`
- `src/store/memory.rs`: Add `impl Default for InMemoryStore`, fix closure at line 93