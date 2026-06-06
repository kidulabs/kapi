## Why

The roadmap calls for a `generation` field that enables controllers to distinguish spec changes from metadata-only changes. Without it, every update (labels, annotations, status) looks like a "modification" to watchers, causing unnecessary reconciliation. This is a prerequisite for the upcoming annotations feature, which will produce frequent metadata-only updates.

## What Changes

- Add `generation: u64` to `SystemMetadata`
- `generation` starts at 1 on CREATE
- `generation` bumps ONLY when `spec` changes in `update()`
- `generation` does NOT bump on `update_status()` or metadata-only updates
- Both `InMemoryStore` and `SQLiteStore` implement this behavior
- Integration test verifies generation semantics across all store implementations

## Capabilities

### New Capabilities

<!-- None — this extends existing capabilities -->

### Modified Capabilities

- `object-store`: `SystemMetadata` gains `generation: u64`. The `update()` method SHALL bump `generation` iff the spec value differs from the stored spec. The `update_status()` method SHALL NOT bump `generation`. The `create()` method SHALL initialize `generation` to 1.
- `integration-tests`: Add test scenarios verifying generation bumps on spec changes and stays constant on metadata-only and status-only updates.

## Impact

- `src/object/types.rs` — add `generation: u64` to `SystemMetadata`
- `src/store/mod.rs` — document generation behavior in `ObjectStore` trait
- `src/store/memory.rs` — compare spec, bump generation in `update()`
- `src/store/sqlite.rs` — compare spec, bump generation in `update()`
- `tests/` — add generation integration test
- `roadmap.md` — mark generation field as complete
