## 1. Create validation module

- [x] 1.1 Create `src/validation/mod.rs` with module documentation explaining it provides stateless format validation callable from any layer
- [x] 1.2 Move `validate_label_key` from `object/service.rs` to `validation/mod.rs`, converting `Regex::new` calls to `std::sync::LazyLock<Regex>` statics (`PREFIX_RE`, `NAME_RE`)
- [x] 1.3 Move `validate_label_value` to `validation/mod.rs`, converting `Regex::new` to a `LazyLock<Regex>` static (`VALUE_RE`)
- [x] 1.4 Move `validate_labels` to `validation/mod.rs`
- [x] 1.5 Move `validate_annotation_key` to `validation/mod.rs`
- [x] 1.6 Move `validate_annotations` to `validation/mod.rs`
- [x] 1.7 Declare `pub mod validation;` in `src/lib.rs`
- [x] 1.8 Move existing unit tests for `validate_labels` and `validate_annotations` from `object/service.rs` tests to `validation/mod.rs` tests

## 2. Update ObjectService to import from validation module

- [x] 2.1 Remove the five validation functions from `object/service.rs`
- [x] 2.2 Add `use crate::validation::{validate_labels, validate_annotations};` to `object/service.rs`
- [x] 2.3 Verify all existing service tests still pass (defense-in-depth calls remain unchanged)

## 3. Update handlers to validate eagerly

- [x] 3.1 Add `use crate::validation::{validate_labels, validate_annotations};` to `object/handler.rs`
- [x] 3.2 In `create()` handler: add `validate_labels(&labels)?;` and `validate_annotations(&annotations)?;` immediately after extraction (after line 150), before the kind branch
- [x] 3.3 In `update()` handler: add `validate_labels(&body.metadata.labels)?;` and `validate_annotations(&body.metadata.annotations)?;` after the URL/body consistency check (after line 443), before calling `state.object_service().update(body)`
- [x] 3.4 Update the module doc comment in `object/handler.rs` to: "Handlers validate input format and deserialization constraints. They never access the store, event bus, or schema registry, and never contain conditional mutation logic."

## 4. Add handler-level tests

- [x] 4.1 Add unit tests in `object/handler.rs` for create handler rejecting invalid label keys before service invocation
- [x] 4.2 Add unit tests in `object/handler.rs` for create handler rejecting annotations exceeding 256KB before service invocation
- [x] 4.3 Add unit tests in `object/handler.rs` for update handler rejecting invalid labels before service invocation

## 5. Verify

- [x] 5.1 Run `cargo check` and fix any compilation errors
- [x] 5.2 Run `cargo clippy` and fix any warnings
- [x] 5.3 Run `cargo test` (unit tests) and verify all pass
- [x] 5.4 Run integration tests against both InMemory and SQLite stores and verify all pass

## 6. Documentation and roadmap

- [x] 6.1 Check `docs/` directory for any documentation referencing validation location and update if necessary
- [x] 6.2 Check `openspec/specs/roadmap-update/` or roadmap items for any entries impacted by this change and update accordingly
