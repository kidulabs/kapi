## Context

Cargo clippy reports 3 warnings that indicate non-idiomatic Rust code:

1. `should_implement_trait` in `src/event/bus.rs:48` - `EventBus` has a `default()` method that should implement `std::default::Default`
2. `new_without_default` in `src/store/memory.rs:21` - `InMemoryStore::new()` exists but struct lacks `Default` impl
3. `redundant_closure` in `src/store/memory.rs:93` - closure `|t| decode_continue_token(t)` can be replaced with function reference `decode_continue_token`

## Goals / Non-Goals

**Goals:**
- Fix all 3 clippy warnings to achieve a clean `cargo clippy` run

**Non-Goals:**
- No behavior changes
- No new capabilities
- No API changes

## Decisions

### Decision 1: Implement `Default` for `EventBus`

**Choice**: Implement `std::default::Default` for `EventBus`, removing the custom `default()` method.

**Rationale**: The custom `default()` method already uses the standard pattern (`Self::new(DEFAULT_CAPACITY)`). Implementing the standard trait makes `EventBus` work with Rust's `..Default::default()` syntax and signals idiomatic Rust to clippy/linters.

### Decision 2: Implement `Default` for `InMemoryStore`

**Choice**: Add `impl Default for InMemoryStore` that forwards to `Self::new()`.

**Rationale**: The struct has a `new()` constructor with no parameters. Adding `Default` is mechanically simple and satisfies clippy's `new_without_default` warning.

### Decision 3: Replace redundant closure with function reference

**Choice**: Change `.map(|t| decode_continue_token(t))` to `.map(decode_continue_token)`.

**Rationale**: `decode_continue_token` is a function that takes one argument of type `&ContinueToken`. The closure is redundant - directly passing the function reference is shorter and more idiomatic.

## Risks / Trade-offs

(None - purely mechanical changes with no behavioral impact)