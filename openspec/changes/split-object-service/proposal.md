## Why

`ObjectService` (1885 lines) manages two distinct entity lifecycles — Schema and regular objects — with `if kind == SCHEMA_KIND` dispatch in every mutating method. This violates SRP: Schema lifecycle (meta-schema validation, compilation, cache management, dependent-object guards) and object lifecycle (schema validation, OCC, finalizer state machine, deletion guards) are fundamentally different concerns bolted into one struct. Adding any new "special kind" would require touching every public method. Secondary issues include selector parsing logic misplaced in the HTTP handler and a 170-line `list()` method in SQLite that mixes query building with execution.

## What Changes

- **Extract `SchemaService`**: All Schema-specific logic (`validate_and_create_schema`, `validate_and_update_schema`, `delete_schema`, and the `if kind == SCHEMA_KIND` dispatch in `create`/`update`/`delete`) moves to a new `src/object/schema_service.rs`. The handler dispatches directly to `SchemaService` or `ObjectService` based on kind.
- **Move selector parsing to types.rs**: `parse_field_selector`, `parse_label_selector`, `parse_label_requirement`, and `validate_label_key` (~100 lines of pure utility code with zero HTTP dependencies) move from `handler.rs` to `types.rs` as `impl` blocks on `FieldSelector` and `LabelSelector`.
- **Extract SQLite query builder**: The dynamic SQL WHERE clause construction in `sqlite.rs` `list()` (~88 lines) is extracted into a testable helper that encapsulates `param_idx` tracking.
- **Remove validation duplication**: Handler-level `validate_labels`/`validate_annotations`/`validate_finalizers` calls are removed; the service already provides defense-in-depth validation for all entry points.
- **Extract finalizer state machine**: The `DeleteAction` enum, `Arc<Mutex<DeleteAction>>` pattern, deletion guard logic, and hard-delete trigger are extracted to `src/object/finalizer.rs` as a pure state machine.

## Capabilities

### New Capabilities
- `schema-service`: Schema lifecycle management — meta-schema validation, schema compilation, cache management, dependent-object guards on deletion. Owns the Schema-specific create/update/delete paths currently embedded in ObjectService.

### Modified Capabilities
- `object-service`: Removes Schema lifecycle responsibilities. ObjectService becomes focused on regular object CRUD: schema validation (delegated to SchemaRegistry), OCC, finalizer lifecycle, system metadata management, and event publishing.
- `object-handlers`: Selector parsing moves to types.rs. Handler dispatches to SchemaService or ObjectService based on kind. Removes duplicated validation calls.
- `finalizer-support`: Finalizer state machine extracted from ObjectService into a standalone, independently testable module.

## Non-Goals

- No changes to the `ObjectStore` trait or store implementations' public API
- No changes to the `EventBus` or `EventPublisher` trait
- No changes to the `SchemaRegistry` internal implementation (only ownership moves)
- No new HTTP endpoints or API behavior changes
- No changes to the `validation` module's pure functions

## Impact

- **Code**: `src/object/service.rs` shrinks from ~1885 to ~1200 lines. New `src/object/schema_service.rs` (~400 lines) and `src/object/finalizer.rs` (~150 lines). `src/object/handler.rs` shrinks by ~100 lines. `src/object/types.rs` grows by ~100 lines.
- **Routes/AppState**: `AppState` gains a `SchemaService` field alongside `ObjectService`. Router wiring changes to pass both services to handlers.
- **Tests**: Existing integration tests remain valid (behavior is unchanged). Unit tests for the new `SchemaService` and `FinalizerStateMachine` modules are added.
- **Dependencies**: No new crate dependencies.

## Future Work

- If additional "special kinds" are added (e.g., ConfigMap, Secret analogs), the dispatch pattern established by SchemaService can be generalized into a trait-based kind router.
- SQLite query builder extraction could be extended to support a Postgres backend with dialect-specific SQL generation.
