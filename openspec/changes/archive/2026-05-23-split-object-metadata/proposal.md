## Why

`ObjectMetadata` currently mixes two concerns: user-controlled fields (`name`) and server-controlled fields (`resourceVersion`, `createdAt`, `updatedAt`). This coupling makes the API semantics unclear — clients see system fields alongside user fields in a flat structure, and the `create()` method signature will grow awkwardly as we add user-provided metadata (labels, annotations). Splitting metadata into `ObjectMeta` (user-controlled) and `SystemMetadata` (server-controlled) establishes a clean boundary before labels are added, making future extensions natural rather than bolted on.

## What Changes

- **BREAKING**: Replace `ObjectMetadata` with two structs: `ObjectMeta` (user-controlled: `name`) and `SystemMetadata` (server-controlled: `resourceVersion`, `createdAt`, `updatedAt`)
- **BREAKING**: Restructure `StoredObject` from `{ key, metadata, data }` to `{ key, metadata, system, data }` where `metadata` is `ObjectMeta` and `system` is `SystemMetadata`
- **BREAKING**: Change wire format — `metadata` no longer contains `resourceVersion`/`createdAt`/`updatedAt`; these move to a new `system` field
- **BREAKING**: Change update request format — client sends `system.resourceVersion` for OCC instead of `metadata.resourceVersion`
- Update `ObjectStore` trait: `create()` takes `ObjectMeta` instead of bare `name`
- Update `InMemoryStore` and `SQLiteStore` to assemble `SystemMetadata` internally
- Update `ObjectService` to use `ObjectMeta` and `SystemMetadata` fields
- Update handlers to extract `ObjectMeta` from request body
- Update OpenAPI component schemas to reflect new wire format
- Update integration tests to use new JSON paths

## Capabilities

### New Capabilities

_(None — this is a restructuring of existing concepts, not a new capability)_

### Modified Capabilities

- `core-types`: `ObjectMetadata` split into `ObjectMeta` and `SystemMetadata`; `StoredObject` restructured
- `object-store`: `create()` signature changes from `(key, name, data)` to `(key, meta, data)`; internal assembly of `SystemMetadata` stays in stores
- `object-handlers`: Create handler extracts `ObjectMeta` from body; update handler uses `system.resourceVersion`
- `object-service`: Field access changes from `.metadata.name` / `.metadata.resource_version` to `.metadata.name` / `.system.resource_version`
- `openapi-spec`: Component schemas split `ObjectMetadata` into `ObjectMeta` + `SystemMetadata`; `StoredObject` schema gains `system` field
- `integration-tests`: JSON paths change (`metadata.resourceVersion` → `system.resourceVersion`, etc.)

## Impact

- **API**: Breaking wire format change. All endpoints returning `StoredObject` change shape.
- **Code**: `src/object/types.rs`, `src/store/memory.rs`, `src/store/sqlite.rs`, `src/object/service.rs`, `src/object/handler.rs`, `src/openapi/components.rs`, `src/openapi/paths.rs`, `src/openapi/mod.rs`, integration tests
- **Docs**: OpenAPI/Swagger UI will show the new structure automatically
- **Roadmap**: This change is a prerequisite for the "Label filtering" roadmap item — labels will be added to `ObjectMeta`