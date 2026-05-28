## Why

Phase 1 added labels and Phase 2 added label selectors for watch, but list requests still cannot filter by labels or fields. Clients must fetch all objects and filter client-side, which is inefficient for large collections. Additionally, watch streams cannot combine field and label selectors (e.g., `metadata.name=foo` AND `app=nginx`). This change enables store-level filtering for list requests and watch filter combinators, completing the selector story.

## What Changes

- Add `field_selector` and `label_selector` fields to `ListOptions` for store-level filtering
- Update `ObjectStore::list()` trait signature to accept filter parameters
- Implement filtering in `InMemoryStore::list()` (post-fetch filter in Rust, before pagination)
- Implement filtering in `SQLiteStore::list()` (SQL WHERE with EXISTS subqueries for labels, direct comparison for fields)
- Remove the 400 error for `fieldSelector` on non-watch list requests (enable field filtering on list)
- Add `WatchFilter::And(Box<WatchFilter>, Box<WatchFilter>)` combinator for composing field and label selectors on watch
- Update `WatchFilter::matches()` to evaluate `And` combinator (both filters must match)
- Update handler to combine field and label selectors into `WatchFilter::And` when both are present on watch
- Update handler to pass both selectors to `ListOptions` for list requests
- Update OpenAPI spec to document `fieldSelector` and `labelSelector` on list requests
- Update Swagger UI to reflect the new parameters
- Review and update documentation in `docs/` for deviations
- Add future work items to `roadmap.md`

## Non-goals

- OR combinators for selectors (K8s doesn't support OR for label selectors)
- Complex nested boolean logic (only AND of field + label)
- Server-side pagination with filtering optimization (beyond basic SQL WHERE)

## Capabilities

### New Capabilities
- `list-filtering`: Store-level filtering for list requests by field and label selectors

### Modified Capabilities
- `watch-filter`: WatchFilter gains an `And` combinator for composing field and label selectors
- `object-store`: ObjectStore::list() accepts filter parameters; both implementations filter before pagination
- `object-handlers`: List handler accepts `fieldSelector` and `labelSelector` on non-watch requests; watch handler combines selectors with `And`
- `openapi-spec`: Generated spec documents `fieldSelector` and `labelSelector` on list requests

## Impact

- **API contract**: `fieldSelector` and `labelSelector` now valid on list requests (previously `fieldSelector` returned 400). This is a behavior change but not breaking (no client should rely on the 400).
- **ObjectStore trait**: `list()` signature changes to accept filter parameters. Both implementations updated.
- **SQLite queries**: List queries gain WHERE clauses with EXISTS subqueries for label filtering. Performance impact depends on label count and index efficiency.
- **WatchFilter**: New `And` variant. Existing filters unchanged.
- **Code**: Touches `object/types.rs`, `object/handler.rs`, `store/mod.rs`, `store/memory.rs`, `store/sqlite.rs`, OpenAPI generation, Swagger UI, docs.

## Future Work

- OR combinators for label selectors (if needed)
- Query optimization for high-cardinality label filters
- Index hints or materialized views for complex label queries
