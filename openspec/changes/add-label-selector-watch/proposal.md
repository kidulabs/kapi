## Why

Phase 1 added labels to objects, but there is no way to filter watch streams by labels. Clients watching a resource kind receive all events, even when they only care about objects with specific labels (e.g., `app=nginx`, `env=prod`). Label selectors enable filtered watch streams, reducing client-side processing and network traffic.

## What Changes

- Add `LabelSelector` type with moderate Kubernetes syntax: equality (`key=value`), inequality (`key!=value`), existence (`key`), non-existence (`!key`), and AND combinator (comma-separated)
- Add `WatchFilter::LabelSelector(LabelSelector)` variant for predicate routing in EventBus
- Add `labelSelector` query parameter to list/watch handler
- Implement `parse_label_selector()` function to parse query string into `LabelSelector`
- Update `WatchFilter::matches()` to evaluate label selectors against object labels
- Add `InvalidLabelSelector(String)` error variant to `AppError` (maps to HTTP 400)
- Update OpenAPI spec to document `labelSelector` query parameter
- Update Swagger UI to reflect the new parameter
- Review and update documentation in `docs/` for deviations
- Add future work items to `roadmap.md`

## Non-goals

- Set-based operators (`in`, `notin`) — future work for full K8s parity
- Label selectors on list requests — Phase 3
- Watch filter combinators (`And` for field + label) — Phase 3
- Field selectors on list requests — Phase 3

## Capabilities

### New Capabilities
- `label-selector`: Label selector type, parser, and matching logic for filtering objects by label key-value pairs

### Modified Capabilities
- `watch-filter`: WatchFilter gains a `LabelSelector` variant for label-based predicate routing
- `object-handlers`: List/watch handler accepts `labelSelector` query parameter
- `error-handling`: New `InvalidLabelSelector` error variant for selector parse failures
- `openapi-spec`: Generated spec documents `labelSelector` query parameter

## Impact

- **API contract**: New `labelSelector` query parameter on list/watch endpoint. Existing clients unaffected (parameter is optional).
- **WatchFilter enum**: New variant added. Existing `All` and `FieldSelector` variants unchanged.
- **EventBus**: No changes needed — `WatchFilter::matches()` is the only integration point, and it gains a new match arm.
- **Code**: Touches `object/types.rs`, `object/handler.rs`, `error.rs`, OpenAPI generation, Swagger UI, docs.

## Future Work

- Full Kubernetes label selector syntax parity (set-based operators: `in`, `notin`)
- Label selector on list requests (Phase 3)
- Watch filter combinators for combining field and label selectors (Phase 3)
