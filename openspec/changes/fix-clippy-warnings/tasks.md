## 1. Implement Default for EventBus

- [ ] 1.1 Remove `pub fn default()` from `src/event/bus.rs`
- [ ] 1.2 Add `impl Default for EventBus` with `fn default()` returning `Self::new(DEFAULT_CAPACITY)`

## 2. Implement Default for InMemoryStore

- [ ] 2.1 Add `impl Default for InMemoryStore` in `src/store/memory.rs` after struct definition

## 3. Replace redundant closure

- [ ] 3.1 Change `.map(|t| decode_continue_token(t))` to `.map(decode_continue_token)` at `src/store/memory.rs:93`

## 4. Verify

- [ ] 4.1 Run `cargo clippy` to confirm zero warnings