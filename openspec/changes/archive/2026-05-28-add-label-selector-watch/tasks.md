## 1. LabelSelector Type and Matching

- [x] 1.1 Define `LabelRequirement` enum in `src/object/types.rs` with variants: `Equals`, `NotEquals`, `Exists`, `NotExists`
- [x] 1.2 Define `LabelSelector` struct with `requirements: Vec<LabelRequirement>` field
- [x] 1.3 Implement `LabelRequirement::matches(&self, labels: &HashMap<String, String>) -> bool` for each variant
- [x] 1.4 Implement `LabelSelector::matches(&self, labels: &HashMap<String, String>) -> bool` with AND semantics (use `Iterator::all`)
- [x] 1.5 Write unit tests for `LabelRequirement::matches()` covering all four variants with matching and non-matching cases
- [x] 1.6 Write unit tests for `LabelSelector::matches()` covering empty selector, single requirement, multiple requirements
- [x] 1.7 Run `cargo test` to verify matching logic

## 2. Label Selector Parsing

- [x] 2.1 Implement `parse_label_selector(raw: &str) -> Result<LabelSelector, AppError>` function in `src/object/handler.rs`
- [x] 2.2 Add parsing logic for equality (`key=value`): split on `=`, validate key and value
- [x] 2.3 Add parsing logic for inequality (`key!=value`): split on `!=`, validate key and value
- [x] 2.4 Add parsing logic for existence (`key`): validate key format
- [x] 2.5 Add parsing logic for non-existence (`!key`): strip `!` prefix, validate key format
- [x] 2.6 Add parsing logic for AND combinator: split on `,`, trim whitespace, parse each requirement
- [x] 2.7 Handle edge cases: empty string (return empty selector), whitespace trimming, empty segments (return error)
- [x] 2.8 Return `AppError::InvalidLabelSelector` with descriptive messages for malformed selectors
- [x] 2.9 Write unit tests for `parse_label_selector()` covering all syntax forms and error cases
- [x] 2.10 Run `cargo test` to verify parsing logic

## 3. Error Handling

- [x] 3.1 Add `InvalidLabelSelector(String)` variant to `AppError` enum in `src/error.rs`
- [x] 3.2 Add HTTP 400 mapping for `InvalidLabelSelector` in `into_response()` implementation
- [x] 3.3 Add error message formatting with `"invalid label selector: "` prefix
- [x] 3.4 Run `cargo check` to verify error variant compiles

## 4. WatchFilter Integration

- [x] 4.1 Add `LabelSelector(LabelSelector)` variant to `WatchFilter` enum in `src/object/types.rs`
- [x] 4.2 Update `WatchFilter::matches()` to handle `LabelSelector` variant: delegate to `ls.matches(&event.object.metadata.labels)`
- [x] 4.3 Write unit tests for `WatchFilter::matches()` with `LabelSelector` variant
- [x] 4.4 Run `cargo test` to verify WatchFilter integration

## 5. Handler Integration

- [x] 5.1 Add `label_selector: Option<String>` field to `ListQuery` struct in `src/object/handler.rs` with `#[serde(rename = "labelSelector")]`
- [x] 5.2 Update list handler to parse `labelSelector` when present (call `parse_label_selector()`)
- [x] 5.3 Update list handler to create `WatchFilter::LabelSelector` for watch requests with `labelSelector`
- [x] 5.4 Handle case where `labelSelector` is present on non-watch list request (return 400 for now — Phase 3 will enable it)
- [x] 5.5 Run `cargo check` to verify handler changes compile

## 6. Integration Tests

- [x] 6.1 Add integration test: watch with `labelSelector=app=nginx`, create object with matching labels, verify event received
- [x] 6.2 Add integration test: watch with `labelSelector=app=nginx`, create object with non-matching labels, verify event NOT received
- [x] 6.3 Add integration test: watch with `labelSelector=app=nginx,env=prod`, create object with both labels, verify event received
- [x] 6.4 Add integration test: watch with `labelSelector=!experimental`, create object without `experimental` label, verify event received
- [x] 6.5 Add integration test: watch with invalid `labelSelector`, verify 400 error response
- [x] 6.6 Add integration test: watch with empty `labelSelector`, verify all events received (matches all)
- [x] 6.7 Run full integration test suite: `cargo test --package kapi-tests`

## 7. OpenAPI Spec Updates

- [x] 7.1 Update OpenAPI spec generation to include `labelSelector` query parameter on list/watch endpoint
- [x] 7.2 Define `labelSelector` as `type: string`, optional, with description explaining syntax
- [x] 7.3 Verify generated OpenAPI spec includes `labelSelector` parameter
- [x] 7.4 Test OpenAPI spec generation: `cargo run --bin kapi -- --print-openapi` (or equivalent)

## 8. Swagger UI Updates

- [x] 8.1 Verify Swagger UI displays `labelSelector` parameter on list/watch endpoint
- [x] 8.2 Test Swagger UI manually or via automated browser test

## 9. Documentation Review

- [x] 9.1 Review `docs/` directory for any documentation that describes watch or filtering
- [x] 9.2 Update documentation to mention `labelSelector` parameter and supported syntax
- [x] 9.3 Add examples showing how to use label selectors with watch
- [x] 9.4 Document the moderate syntax (equality, inequality, existence, non-existence, AND)
- [x] 9.5 Check if any README files need updates

## 10. Roadmap Updates

- [x] 10.1 Review `roadmap.md` for items impacted by this change
- [x] 10.2 Update "Label filtering" item to reflect progress (label selector on watch done, list pending)
- [x] 10.3 Add future work item: "Full Kubernetes label selector syntax parity (set-based operators: in, notin)"
- [x] 10.4 Verify "Watch filter combinators" item is still pending (Phase 3)

## 11. Final Verification

- [x] 11.1 Run `cargo clippy --all-targets --all-features` and fix any warnings
- [x] 11.2 Run `cargo fmt --check` and format code if needed
- [x] 11.3 Run full test suite: `cargo test --workspace`
- [x] 11.4 Manual smoke test: start server, create objects with labels, watch with labelSelector via curl, verify filtered events
- [x] 11.5 Verify error messages: test invalid labelSelector, verify clear error response
