## Why

The `data` field on `StoredObject` is a generic name that doesn't convey its semantic role. With the upcoming status subresource feature, the object will have two user-defined fields: `spec` (desired state, user-written) and `status` (observed state, controller-written). Renaming `data` → `spec` now establishes the correct terminology before the status feature adds complexity. Since kapi is in dev phase with no migration concerns, this is the right time.

## What Changes

- **BREAKING**: Rename `StoredObject.data` → `StoredObject.spec` (Rust struct field)
- **BREAKING**: Rename `UserData` → `SpecData` (type name)
- **BREAKING**: Rename `UserData.value` → `SpecData.value` (field unchanged, type renamed)
- **BREAKING**: Rename `ObjectStore::create(key, meta, data)` → `ObjectStore::create(key, meta, spec)` (parameter name)
- **BREAKING**: Rename SQLite column `data` → `spec` in the `objects` table
- Update all internal references: `.data` → `.spec`, `data:` → `spec:` in struct literals, variable names
- Update OpenAPI component schemas: `"data"` JSON key → `"spec"`, `UserData` component → `SpecData`
- Update OpenAPI dynamic path generation: `build_kind_data_component` → `build_kind_spec_component`
- Update integration test JSON payloads: `"data"` → `"spec"` in request/response bodies
- Update all unit test references

## Capabilities

### New Capabilities

_None_

### Modified Capabilities

- `core-types`: Rename `data` field to `spec` and `UserData` to `SpecData` in StoredObject and related types
- `object-store`: Rename `data` parameter to `spec` in ObjectStore trait and both implementations
- `object-service`: Update all references from `data` to `spec` in service methods
- `object-handlers`: Update handler body extraction from `data` to `spec`
- `openapi-spec`: Update OpenAPI component schemas and dynamic path generation to use `spec` instead of `data`

## Impact

- **API**: Breaking — JSON field `"data"` becomes `"spec"` in all StoredObject responses and request bodies
- **Storage**: Breaking — SQLite column `data` becomes `spec` (existing databases need recreation, acceptable in dev)
- **Code**: Mechanical rename across ~80 touch points in src/ and tests/
- **No logic changes**: Pure rename, no behavioral changes

## Non-goals

- Adding the status subresource (separate change)
- Adding `generation` or `status_version` fields
- Changing the ObjectStore trait semantics
- Modifying the meta-schema or SchemaData

## Future Work

- Add status subresource (`StoredObject.status: Option<SpecData>`, `/status` endpoint, `StatusModified` event type)
- Introduce Schema object status (kapi-defined shape, server-maintained: objectCount, schemaVersion, validationState)