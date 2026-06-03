## Context

kapi's `StoredObject` currently has a `data: UserData` field holding arbitrary JSON. With the upcoming status subresource feature, the object will have two user-defined fields: `spec` (desired state) and `status` (observed state). Renaming `data` → `spec` now establishes correct terminology before the status feature adds complexity. This is a pure rename — no logic changes, no new behavior.

Current state:
- `StoredObject { key, metadata, system, data: UserData }` where `UserData { value: serde_json::Value }`
- `ObjectStore::create(key, meta, data: Value)` parameter
- SQLite column named `data`
- OpenAPI component `UserData` and JSON key `"data"`
- ~80 touch points across src/ and tests/

## Goals / Non-Goals

**Goals:**
- Rename `data` → `spec` across all layers (types, store, service, handlers, OpenAPI, tests)
- Rename `UserData` → `SpecData` type
- Rename SQLite `data` column → `spec`
- Maintain all existing behavior — this is a rename only

**Non-Goals:**
- Adding the status subresource (separate change)
- Adding `generation`, `status_version`, or any new fields
- Changing the ObjectStore trait semantics
- Modifying the meta-schema or SchemaData
- Supporting database migration (dev phase, recreate DB)

## Decisions

### Decision 1: Rename `UserData` → `SpecData`

**Choice**: Rename the type to `SpecData`.

**Alternatives considered**:
- Keep `UserData` — inconsistent with the field name `spec`; "UserData" doesn't convey that it's the desired state
- Rename to `Spec` — too short, could conflict with other "spec" concepts

**Rationale**: `SpecData` mirrors the field name and clearly indicates it holds the spec (desired state) payload.

### Decision 2: SQLite column rename

**Choice**: Rename the `data` column to `spec` in the `objects` table.

**Alternatives considered**:
- Keep column as `data` but rename Rust field — creates a confusing mismatch between DB schema and Rust types
- Add a new `spec` column and migrate — unnecessary complexity for dev phase

**Rationale**: Since kapi is in dev phase with no production databases, a clean column rename is simplest. The `init_schema()` method creates the table, so the column name change is straightforward.

### Decision 3: OpenAPI component naming

**Choice**: Rename `UserData` component → `SpecData`, `build_kind_data_component` → `build_kind_spec_component`, and JSON key `"data"` → `"spec"` in StoredObject schema.

**Rationale**: Consistency — the API surface should reflect the internal naming.

### Decision 4: Variable names in service/handler code

**Choice**: Rename local variables named `data` to `spec` where they refer to the object's spec payload. Keep `schema_data` as-is (it refers to `SchemaData`, not the spec field).

**Rationale**: `schema_data` is a different concept (the parsed Schema registration payload), so it should keep its name.

## Risks / Trade-offs

- **[Breaking API change]** → Acceptable in dev phase. All clients must update JSON field from `"data"` to `"spec"`.
- **[SQLite DB recreation required]** → Existing databases must be recreated. Acceptable in dev phase.
- **[Missed rename causing compile error]** → The Rust compiler catches all `.data` → `.spec` field access errors. Test failures catch JSON key changes. Low risk.

## Migration Plan

1. Apply all renames in a single commit
2. Delete any existing SQLite database files (dev phase, no migration)
3. Run `cargo test` to verify all renames are correct
4. Run integration tests to verify API behavior

No rollback strategy needed — this is a single atomic rename commit. If needed, `git revert` restores the previous state.