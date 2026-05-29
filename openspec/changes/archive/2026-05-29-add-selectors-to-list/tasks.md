## 1. ListOptions Filter Parameters

- [x] 1.1 Add `field_selector: Option<FieldSelector>` field to `ListOptions` in `src/object/types.rs`
- [x] 1.2 Add `label_selector: Option<LabelSelector>` field to `ListOptions`
- [x] 1.3 Implement `Default` for `ListOptions` if not already present (for backward compatibility)
- [x] 1.4 Run `cargo check` to verify type changes compile

## 2. WatchFilter And Combinator

- [x] 2.1 Add `And(Box<WatchFilter>, Box<WatchFilter>)` variant to `WatchFilter` enum in `src/object/types.rs`
- [x] 2.2 Update `WatchFilter::matches()` to handle `And` variant: evaluate `a.matches(event) && b.matches(event)` with short-circuit
- [x] 2.3 Write unit tests for `WatchFilter::And` covering both-match, first-fails, second-fails cases
- [x] 2.4 Run `cargo test` to verify And combinator logic

## 3. ObjectStore Trait Update

- [x] 3.1 Verify `ObjectStore::list()` signature accepts `ListOptions` (should already, just verify)
- [x] 3.2 Update any documentation or comments on the trait method to mention filter parameters
- [x] 3.3 Run `cargo check` to verify trait signature is correct

## 4. InMemoryStore Filtering

- [x] 4.1 Update `InMemoryStore::list()` to apply `field_selector` filter after collecting objects
- [x] 4.2 Update `InMemoryStore::list()` to apply `label_selector` filter after field filter
- [x] 4.3 Ensure filtering happens before sorting and pagination (correct order: collect → filter → sort → skip → truncate)
- [x] 4.4 Write unit tests for InMemoryStore list with field selector
- [x] 4.5 Write unit tests for InMemoryStore list with label selector
- [x] 4.6 Write unit tests for InMemoryStore list with both selectors
- [x] 4.7 Write unit test verifying filter reduces result set correctly (50 objects, filter to 3, limit 10 → returns 3)
- [x] 4.8 Run `cargo test` to verify InMemoryStore filtering

## 5. SQLiteStore Filtering

- [x] 5.1 Update `SQLiteStore::list()` to build SQL with field filter: add `AND name = ?` when `field_selector` is present
- [x] 5.2 Implement SQL generation for label equality: `EXISTS (SELECT 1 FROM labels WHERE ... AND label_key = ? AND label_value = ?)`
- [x] 5.3 Implement SQL generation for label inequality: `NOT EXISTS (...) OR EXISTS (... AND label_value != ?)`
- [x] 5.4 Implement SQL generation for label existence: `EXISTS (SELECT 1 FROM labels WHERE ... AND label_key = ?)`
- [x] 5.5 Implement SQL generation for label non-existence: `NOT EXISTS (SELECT 1 FROM labels WHERE ... AND label_key = ?)`
- [x] 5.6 Combine multiple label requirements as multiple `EXISTS`/`NOT EXISTS` clauses (AND semantics)
- [x] 5.7 Bind parameters correctly for dynamic SQL (track parameter indices)
- [x] 5.8 Ensure filtering happens in SQL (before pagination): WHERE clauses come before ORDER BY and LIMIT
- [x] 5.9 Write unit tests for SQLiteStore list with field selector
- [x] 5.10 Write unit tests for SQLiteStore list with label selector (all four requirement types)
- [x] 5.11 Write unit tests for SQLiteStore list with both selectors
- [x] 5.12 Write unit test verifying SQL query structure (inspect generated SQL if possible)
- [x] 5.13 Run `cargo test` to verify SQLiteStore filtering

## 6. Handler Updates for List

- [x] 6.1 Update list handler to parse `fieldSelector` on non-watch requests (remove 400 error)
- [x] 6.2 Update list handler to parse `labelSelector` on non-watch requests
- [x] 6.3 Pass parsed selectors to `ListOptions` when calling `object_service.list()`
- [x] 6.4 Handle case where both selectors are present on list request
- [x] 6.5 Handle invalid selectors on list request (return appropriate 400 error)
- [x] 6.6 Run `cargo check` to verify handler changes compile

## 7. Handler Updates for Watch

- [x] 7.1 Update watch handler to combine `fieldSelector` and `labelSelector` with `WatchFilter::And` when both are present
- [x] 7.2 Implement pattern matching for four cases: both present, field only, label only, neither
- [x] 7.3 Write unit tests for watch handler combination logic
- [x] 7.4 Run `cargo check` to verify handler changes compile

## 8. Integration Tests

- [x] 8.1 Add integration test: list with `fieldSelector=metadata.name=foo`, verify filtered results
- [x] 8.2 Add integration test: list with `labelSelector=app=nginx`, verify filtered results
- [x] 8.3 Add integration test: list with both selectors, verify filtered results
- [x] 8.4 Add integration test: list with filter and pagination, verify correct page size
- [x] 8.5 Add integration test: list with filter that matches no objects, verify empty result
- [x] 8.6 Add integration test: watch with both selectors, create object matching both, verify event received
- [x] 8.7 Add integration test: watch with both selectors, create object matching only one, verify event NOT received
- [x] 8.8 Add integration test: list with invalid `fieldSelector`, verify 400 error
- [x] 8.9 Add integration test: list with invalid `labelSelector`, verify 400 error
- [x] 8.10 Run full integration test suite: `cargo test --package kapi-tests`

## 9. OpenAPI Spec Updates

- [x] 9.1 Update OpenAPI spec generation to include `fieldSelector` on list endpoint (not just watch)
- [x] 9.2 Update OpenAPI spec generation to include `labelSelector` on list endpoint
- [x] 9.3 Update parameter descriptions to indicate both selectors are valid on list and watch
- [x] 9.4 Verify generated OpenAPI spec includes both parameters on list endpoint
- [x] 9.5 Test OpenAPI spec generation: `cargo run --bin kapi -- --print-openapi` (or equivalent)

## 10. Swagger UI Updates

- [x] 10.1 Verify Swagger UI displays `fieldSelector` parameter on list endpoint
- [x] 10.2 Verify Swagger UI displays `labelSelector` parameter on list endpoint
- [x] 10.3 Test Swagger UI manually or via automated browser test

## 11. Documentation Review

- [x] 11.1 Review `docs/` directory for any documentation that describes list or watch operations
- [x] 11.2 Update documentation to mention `fieldSelector` and `labelSelector` on list requests
- [x] 11.3 Update documentation to mention watch filter combinators (And)
- [x] 11.4 Add examples showing how to use selectors on list requests
- [x] 11.5 Add examples showing how to combine selectors on watch
- [x] 11.6 Document the behavior change: `fieldSelector` on list now works (previously returned 400)
- [x] 11.7 Check if any README files need updates

## 12. Roadmap Updates

- [x] 12.1 Review `roadmap.md` for items impacted by this change
- [x] 12.2 Mark "Label filtering" item as complete (labels + selector on watch + selector on list)
- [x] 12.3 Mark "Watch filtering on list requests" item as complete (fieldSelector + labelSelector on list)
- [x] 12.4 Mark "Watch filter combinators" item as complete (And combinator)
- [x] 12.5 Add future work items: "OR combinators for label selectors", "Query optimization for high-cardinality labels"

## 13. Final Verification

- [x] 13.1 Run `cargo clippy --all-targets --all-features` and fix any warnings
- [x] 13.2 Run `cargo fmt --check` and format code if needed
- [x] 13.3 Run full test suite: `cargo test --workspace`
- [x] 13.4 Manual smoke test: start server, create objects with labels, list with selectors via curl, verify filtered results
- [x] 13.5 Manual smoke test: watch with combined selectors, verify filtered events
- [x] 13.6 Verify pagination correctness: list with filter and limit, verify page sizes are correct
- [x] 13.7 Verify SQLite query performance: check that label filtering uses indexes (EXPLAIN QUERY PLAN if needed)
