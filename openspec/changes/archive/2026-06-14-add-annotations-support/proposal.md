## Why

The roadmap calls for annotations as free-form key-value metadata on `ObjectMeta` without selection semantics. Labels serve selection/filtering use cases with strict validation and indexing. Annotations serve a different purpose: attaching arbitrary metadata (build info, git SHAs, controller state, descriptive notes) that clients need to read and write but never query against. Without annotations, users either abuse labels (polluting the index) or store metadata in spec (mixing concerns).

## What Changes

- Add `annotations: HashMap<String, String>` field to `ObjectMeta` with `#[serde(default)]`
- Implement minimal validation: non-empty keys (max 256 chars), any string values, 256KB total limit per object
- Add `AppError::InvalidAnnotation(String)` error variant returning HTTP 400
- Store annotations as JSON column in SQLite `objects` table (no separate table, no indexing)
- Add `extract_annotations()` handler function mirroring `extract_labels()`
- Update OpenAPI spec to include `annotations` in `ObjectMeta` component
- Add integration tests covering create, update, get, list with annotations

## Capabilities

### New Capabilities
- `object-annotations`: Annotation field on ObjectMeta, minimal validation rules, JSON column storage in SQLite, handler extraction, OpenAPI spec updates

### Modified Capabilities
- `core-types`: ObjectMeta struct gains `annotations: HashMap<String, String>` field with `#[serde(default)]`

## Impact

**Code changes:**
- `src/object/types.rs`: Add `annotations` field to `ObjectMeta`
- `src/object/service.rs`: Add `validate_annotations()` function, call in create/update paths
- `src/object/handler.rs`: Add `extract_annotations()` function, call in create/update handlers
- `src/error.rs`: Add `InvalidAnnotation` variant
- `src/store/sqlite.rs`: Add `annotations TEXT` column, serialize/deserialize as JSON
- `src/openapi/components.rs`: Add `annotations` to ObjectMeta schema
- `tests/src/object_annotations.rs`: New test module mirroring `object_labels.rs`

**API changes:**
- Request bodies accept `metadata.annotations` (optional, defaults to empty map)
- Response bodies include `metadata.annotations` (always present, even if empty)
- No breaking changes: existing clients without annotations continue to work

**Storage changes:**
- SQLite schema migration: `ALTER TABLE objects ADD COLUMN annotations TEXT`
- InMemoryStore: no changes (annotations stored in `ObjectMeta` directly)

**Dependencies:** None

## Non-goals

- `annotationSelector` query parameter (annotations have no selection semantics by design)
- Separate `annotations` table with indexes (annotations are never queried in WHERE clauses)
- Strict character validation (annotations accept arbitrary strings, unlike labels)
- Per-value size limits (256KB total cap is sufficient)
- System annotation prefix handling (`kapi.io/*`) — add later if needed
- Migration tooling for existing objects (handled by `#[serde(default)]`)

## Future Work

- System annotation prefix (`kapi.io/*`) for server-managed metadata (e.g., `kapi.io/last-applied-config`)
- Annotation size metrics and monitoring
- Consider annotation-specific watch filters if use cases emerge
