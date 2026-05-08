## Why

The current `ObjectStore` trait exposes `expected_version` as a method parameter on `update` and `delete`, leaking an internal concurrency control concern into the storage contract. Additionally, `StoredObject` flattens identity (`key`, `name`), lifecycle metadata (`resource_version`, `created_at`, `updated_at`), and user data (`data`) into a single level, making it unclear what is server-managed vs client-managed. User schemas that validate the full object are forced to declare server metadata fields, which is error-prone and conflates domain concerns with infrastructure concerns.

## What Changes

- **StoredObject shape**: `name`, `resource_version`, `created_at`, `updated_at` move into a new `ObjectMetadata` struct. `key` stays top-level (it is identity, not metadata). **BREAKING** to existing type shape.
- **ObjectStore trait signatures**: `update` takes the full `StoredObject` (OCC check peeks at embedded `metadata.resource_version`). `delete` takes only `(key, name)` — unconditional, no version parameter. **BREAKING** to existing trait contract.
- **Wire format**: JSON uses camelCase for metadata fields (`resourceVersion`, `createdAt`, `updatedAt`). Metadata is server-managed; user schemas validate only the `data` portion. **BREAKING** to existing JSON shape.
- **Delete semantics**: Delete is always unconditional. The optional `expected_version` parameter is removed. **BREAKING** to existing behavior.

## Capabilities

### New Capabilities

(none — this is a refactoring of existing capabilities)

### Modified Capabilities

- `object-store`: `update` signature changes from `(key, name, data, expected_version)` to `(StoredObject)`. `delete` signature changes from `(key, name, expected_version)` to `(key, name)`. OCC moves from explicit parameter to embedded version check.
- `core-types`: `StoredObject` structure changes — metadata fields grouped into `ObjectMetadata`. New `ObjectMetadata` type introduced. Serialization uses camelCase for wire format.

## Impact

- `roadmap.md`: Key Types, Storage Traits, Design Decisions, API Surface, Request Flow, Open Questions, and Backlog sections all updated to reflect new design.
- `src/object/types.rs`: `StoredObject` structure changes, `ObjectMetadata` type added.
- `src/store/mod.rs`: `ObjectStore` trait signatures change.
- `src/store/memory.rs`: `InMemoryStore` implementation and all tests must be rewritten for new signatures.
- `openspec/specs/object-store/spec.md`: Trait method specs, update OCC spec, delete spec all change.
- `openspec/specs/core-types/spec.md`: `StoredObject` structure spec changes.
- All future service/handler/route code (not yet written) will use the new signatures.
