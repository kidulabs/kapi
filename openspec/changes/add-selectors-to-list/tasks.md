## 1. ListOptions Filter Parameters

- [ ] 1.1 Add `field_selector: Option<FieldSelector>` field to `ListOptions` in `src/object/types.rs`
- [ ] 1.2 Add `label_selector: Option<LabelSelector>` field to `ListOptions`
- [ ] 1.3 Implement `Default` for `ListOptions` if not already present (for backward compatibility)
- [ ] 1.4 Run `cargo check` to verify type changes compile

## 2. WatchFilter And Combinator

- [ ] 2.1 Add `And(Box<WatchFilter>, Box<WatchFilter>)` variant to `WatchFilter` enum in `src/object/types.rs`
- [ ] 2.2 Update `WatchFilter::matches()` to handle `And` variant: evaluate `a.matches(event) && b.matches(event)` with short-circuit
- [ ] 2.3 Write unit tests for `WatchFilter::And` covering both-match, first-fails, second-fails cases
- [ ] 2.4 Run `cargo test` to verify And combinator logic

## 3. ObjectStore Trait Update

- [ ] 3.1 Verify `ObjectStore::list()` signature accepts `ListOptions` (should already, just verify)
- [ ] 3.2 Update any documentation or comments on the trait method to mention filter parameters
- [ ] 3.3 Run `cargo check` to verify trait signature is correct

## 4. InMemoryStore Filtering

- [ ] 4.1 Update `InMemoryStore::list()` to apply `field_selector` filter after collecting objects
- [ ] 4.2 Update `InMemoryStore::list()` to apply `label_selector` filter after field filter
- [ ] 4.3 Ensure filtering happens before sorting and pagination (correct order: collect → filter → sort → skip → truncate)
- [ ] 4.4 Write unit tests for InMemoryStore list with field selector
- [ ] 4.5 Write unit tests for InMemoryStore list with label selector
- [ ] 4.6 Write unit tests for InMemoryStore list with both selectors
- [ ] 4.7 Write unit test verifying filter reduces result set correctly (50 objects, filter to 3, limit 10 → returns 3)
- [ ] 4.8 Run `cargo test` to verify InMemoryStore filtering

## 5. SQLiteStore Filtering

- [ ] 5.1 Update `SQLiteStore::list()` to build SQL with field filter: add `AND name = ?` when `field_selector` is present
- [ ] 5.2 Implement SQL generation for label equality: `EXISTS (SELECT 1 FROM labels WHERE ... AND label_key = ? AND label_value = ?)`
- [ ] 5.3 Implement SQL generation for label inequality: `NOT EXISTS (...) OR EXISTS (... AND label_value != ?)`
- [ ] 5.4 Implement SQL generation for label existence: `EXISTS (SELECT 1 FROM labels WHERE ... AND label_key = ?)`
- [ ] 5.5 Implement SQL generation for label non-existence: `NOT EXISTS (SELECT 1 FROM labels WHERE ... AND label_key = ?)`
- [ ] 5.6 Combine multiple label requirements as multiple `EXISTS`/`NOT EXISTS` clauses (AND semantics)
- [ ] 5.7 Bind parameters correctly for dynamic SQL (track parameter indices)
- [ ] 5.8 Ensure filtering happens in SQL (before pagination): WHERE clauses come before ORDER BY and LIMIT
- [ ] 5.9 Write unit tests for SQLiteStore list with field selector
- [ ] 5.10 Write unit tests for SQLiteStore list with label selector (all four requirement types)
- [ ] 5.11 Write unit tests for SQLiteStore list with both selectors
- [ ] 5.12 Write unit test verifying SQL query structure (inspect generated SQL if possible)
- [ ] 5.13 Run `cargo test` to verify SQLiteStore filtering

## 6. Handler Updates for List

- [ ] 6.1 Update list handler to parse `fieldSelector` on non-watch requests (remove 400 error)
- [ ] 6.2 Update list handler to parse `labelSelector` on non-watch requests
- [ ] 6.3 Pass parsed selectors to `ListOptions` when calling `object_service.list()`
- [ ] 6.4 Handle case where both selectors are present on list request
- [ ] 6.5 Handle invalid selectors on list request (return appropriate 400 error)
- [ ] 6.6 Run `cargo check` to verify handler changes compile

## 7. Handler Updates for Watch

- [ ] 7.1 Update watch handler to combine `fieldSelector` and `labelSelector` with `WatchFilter::And` when both are present
- [ ] 7.2 Implement pattern matching for four cases: both present, field only, label only, neither
- [ ] 7.3 Write unit tests for watch handler combination logic
- [ ] 7.4 Run `cargo check` to verify handler changes compile

## 8. Integration Tests

- [ ] 8.1 Add integration test: list with `fieldSelector=metadata.name=foo`, verify filtered results
- [ ] 8.2 Add integration test: list with `labelSelector=app=nginx`, verify filtered results
- [ ] 8.3 Add integration test: list with both selectors, verify filtered results
- [ ] 8.4 Add integration test: list with filter and pagination, verify correct page size
- [ ] 8.5 Add integration test: list with filter that matches no objects, verify empty result
- [ ] 8.6 Add integration test: watch with both selectors, create object matching both, verify event received
- [ ] 8.7 Add integration test: watch with both selectors, create object matching only one, verify event NOT received
- [ ] 8.8 Add integration test: list with invalid `fieldSelector`, verify 400 error
- [ ] 8.9 Add integration test: list with invalid `labelSelector`, verify 400 error
- [ ] 8.10 Run full integration test suite: `cargo test --package kapi-tests`

## 9. OpenAPI Spec Updates

- [ ] 9.1 Update OpenAPI spec generation to include `fieldSelector` on list endpoint (not just watch)
- [ ] 9.2 Update OpenAPI spec generation to include `labelSelector` on list endpoint
- [ ] 9.3 Update parameter descriptions to indicate both selectors are valid on list and watch
- [ ] 9.4 Verify generated OpenAPI spec includes both parameters on list endpoint
- [ ] 9.5 Test OpenAPI spec generation: `cargo run --bin kapi -- --print-openapi` (or equivalent)

## 10. Swagger UI Updates

- [ ] 10.1 Verify Swagger UI displays `fieldSelector` parameter on list endpoint
- [ ] 10.2 Verify Swagger UI displays `labelSelector` parameter on list endpoint
- [ ] 10.3 Test Swagger UI manually or via automated browser test

## 11. Documentation Review

- [ ] 11.1 Review `docs/` directory for any documentation that describes list or watch operations
- [ ] 11.2 Update documentation to mention `fieldSelector` and `labelSelector` on list requests
- [ ] 11.3 Update documentation to mention watch filter combinators (And)
- [ ] 11.4 Add examples showing how to use selectors on list requests
- [ ] 11.5 Add examples showing how to combine selectors on watch
- [ ] 11.6 Document the behavior change: `fieldSelector` on list now works (previously returned 400)
- [ ] 11.7 Check if any README files need updates

## 12. Roadmap Updates

- [ ] 12.1 Review `roadmap.md` for items impacted by this change
- [ ] 12.2 Mark "Label filtering" item as complete (labels + selector on watch + selector on list)
- [ ] 12.3 Mark "Watch filtering on list requests" item as complete (fieldSelector + labelSelector on list)
- [ ] 12.4 Mark "Watch filter combinators" item as complete (And combinator)
- [ ] 12.5 Add future work items: "OR combinators for label selectors", "Query optimization for high-cardinality labels"

## 13. Final Verification

- [ ] 13.1 Run `cargo clippy --all-targets --all-features` and fix any warnings
- [ ] 13.2 Run `cargo fmt --check` and format code if needed
- [ ] 13.3 Run full test suite: `cargo test --workspace`
- [ ] 13.4 Manual smoke test: start server, create objects with labels, list with selectors via curl, verify filtered results
- [ ] 13.5 Manual smoke test: watch with combined selectors, verify filtered events
- [ ] 13.6 Verify pagination correctness: list with filter and limit, verify page sizes are correct
- [ ] 13.7 Verify SQLite query performance: check that label filtering uses indexes (EXPLAIN QUERY PLAN if needed)
