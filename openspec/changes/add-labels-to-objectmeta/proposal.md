## Why

Objects in kapi currently have only a `name` in their metadata. Labels — key-value pairs attached to objects — are the standard mechanism for organizing, selecting, and filtering resources. Without labels, there is no way to group objects by environment, team, tier, or any user-defined dimension. This is the foundational change that enables label-based selection in subsequent phases.

## What Changes

- Add `labels: HashMap<String, String>` field to `ObjectMeta` (always serialized, empty map when absent)
- Add `labels` table to SQLite schema for SQL-level label filtering (Phase 3 dependency)
- Update `InMemoryStore` to store and reconstruct labels from `ObjectMeta`
- Update `SQLiteStore` to persist labels in a separate table, with diff-based updates (read existing → compute diff → apply changes in transaction)
- Add label validation in `ObjectService` following Kubernetes semantics: alphanumeric + `-_.` for values (max 256 chars), alphanumeric + `-_.` + `/` for keys (max 256 chars), non-empty keys and values
- Update create/update handlers to extract labels from `metadata.labels` in request body (applies to both regular objects and Schema objects)
- Add `InvalidLabel` error variant to `AppError` (maps to HTTP 400)
- Update OpenAPI spec to document `labels` field on `ObjectMeta`
- Update Swagger UI to reflect the new field
- Review and update documentation in `docs/` for deviations
- Add future work items to `roadmap.md`

## Non-goals

- Label selector query parameters (`labelSelector`) — Phase 2
- Label/field filtering on list requests — Phase 3
- Watch filter combinators (`And`) — Phase 3
- Label validation against user-defined schemas (schema validates `data`, not `metadata`)

## Capabilities

### New Capabilities
- `object-labels`: Labels on ObjectMeta — storage, validation, serialization, and handler extraction for key-value metadata attached to all objects (including Schema objects)

### Modified Capabilities
- `core-types`: ObjectMeta gains a `labels` field, changing its serialization shape
- `object-store`: SQLiteStore gains a `labels` table and diff-based label persistence; InMemoryStore stores labels as part of ObjectMeta
- `object-handlers`: Create and update handlers extract labels from `metadata.labels`
- `object-service`: Service layer validates labels before persistence
- `error-handling`: New `InvalidLabel` error variant for label validation failures
- `openapi-spec`: Generated spec documents `labels` field on ObjectMeta

## Impact

- **API contract**: `ObjectMeta` serialization changes — `labels` field always present (empty `{}` when no labels). Existing clients that don't send labels will get objects with `"labels": {}`.
- **SQLite schema**: New `labels` table added via `CREATE TABLE IF NOT EXISTS` (idempotent, no migration needed). Existing objects have no labels.
- **Update path**: Label updates use diff-based strategy (read existing → compute diff → apply in transaction), not delete-all-then-insert.
- **Code**: Touches `object/types.rs`, `object/handler.rs`, `object/service.rs`, `store/memory.rs`, `store/sqlite.rs`, `error.rs`, OpenAPI generation, Swagger UI, docs.

## Future Work

- Full Kubernetes label selector syntax parity (set-based operators: `in`, `notin`) — currently moderate syntax only (equality, inequality, existence, non-existence, AND)
- Label indexing for high-cardinality label queries at scale
- Annotations (free-form key-value metadata without selection semantics)
