## Context

`ObjectService` at 1885 lines manages two distinct entity lifecycles through `if kind == SCHEMA_KIND` dispatch in `create()`, `update()`, and `delete()`. Schema operations (meta-schema validation, JSON Schema compilation, cache management, dependent-object guards) and object operations (schema validation, OCC, finalizer state machine, deletion guards) are fundamentally different concerns sharing a struct. The handler layer contains ~100 lines of pure selector parsing with zero HTTP dependencies. The SQLite `list()` method is 170 lines mixing dynamic SQL construction with query execution.

The existing architecture is sound — trait boundaries are clean, the `transaction()` pattern works well, and tests are comprehensive. This is a refactoring change with zero behavioral impact.

## Goals / Non-Goals

**Goals:**
- Reduce `ObjectService` to a single responsibility: regular object CRUD orchestration
- Extract Schema lifecycle into a focused `SchemaService` with clear boundaries
- Move selector parsing to `types.rs` where it logically belongs
- Extract SQLite query builder for testability
- Remove handler/service validation duplication
- Extract finalizer state machine for independent testability
- Maintain 100% backward compatibility — no API or behavior changes

**Non-Goals:**
- No changes to `ObjectStore` trait or store public API
- No changes to `EventBus` / `EventPublisher` trait
- No new HTTP endpoints or API behavior
- No new crate dependencies
- No generalization for future "special kinds" (YAGNI — do it when needed)

## Decisions

### Decision 1: SchemaService as a separate struct (not a trait)

**Choice:** Create `SchemaService` as a concrete struct holding `Arc<dyn ObjectStore>`, `Arc<dyn EventPublisher>`, and `SchemaRegistry`. The handler dispatches to `SchemaService` or `ObjectService` based on `kind`.

**Alternatives considered:**
- **Trait-based kind router** (`trait KindHandler`): Over-engineering for exactly two kinds. Adds indirection without benefit until a third kind exists.
- **Strategy pattern within ObjectService**: Keeps one struct but adds internal dispatch complexity. Doesn't reduce cognitive load.
- **Move Schema logic to SchemaRegistry**: SchemaRegistry is a cache/compilation collaborator, not an orchestrator. Adding store writes and event publishing would violate its single responsibility.

**Rationale:** A concrete struct is the simplest extraction. It mirrors ObjectService's pattern, is easy to test, and can be generalized later if needed. The handler-level `if kind == SCHEMA_KIND` dispatch is a single `if` in 3 methods — acceptable complexity.

### Decision 2: Handler dispatches to SchemaService or ObjectService

**Choice:** Handlers receive both `SchemaService` and `ObjectService` via `AppState`. The `create`, `update`, and `delete` handlers check `kind == SCHEMA_KIND` and route to the appropriate service.

**Alternatives considered:**
- **Service-level dispatch (keep `if` in ObjectService)**: Defeats the purpose of extraction.
- **Middleware routing**: Adds a tower layer for what is a simple `if` check. Over-engineering.

**Rationale:** Handler-level dispatch is transparent, testable, and keeps both services focused. The `if` is already in the handler today (for Schema create body parsing) — we're just moving the service call, not adding new logic.

### Decision 3: Shared helpers become free functions

**Choice:** `publish_event`, `apply_with_metadata`, and `map_validation_errors` — currently private methods on ObjectService but also needed by SchemaService — become free functions in a shared module (`src/object/helpers.rs` or inline in `service.rs` as `pub(crate)` functions).

**Alternatives considered:**
- **Duplicate in both services**: Violates DRY, creates drift risk.
- **Trait for shared behavior**: Over-engineering for 3 functions.
- **Keep as methods on ObjectService, call from SchemaService**: Creates coupling between the two services.

**Rationale:** Free functions with clear signatures are the simplest sharing mechanism. They have no `self` dependency — they operate on passed-in values. A `pub(crate)` module keeps them internal.

### Decision 4: Selector parsing moves to types.rs as impl blocks

**Choice:** `parse_field_selector` and `parse_label_selector` become `FieldSelector::parse()` and `LabelSelector::parse()` as `impl` blocks in `types.rs`. `parse_label_requirement` and `validate_label_key` become private helpers within the same `impl` blocks.

**Alternatives considered:**
- **Separate `src/object/selectors.rs` module**: Creates a new file for ~100 lines. The types already live in `types.rs` — co-locate parsing with the type.
- **Keep in handler.rs**: The current location. Pure utility code with zero HTTP dependencies doesn't belong in a handler file.

**Rationale:** Co-locating parsing with the type it produces is idiomatic Rust. The `parse()` method pattern is familiar (like `Duration::parse()`, `Url::parse()`). Zero behavioral change.

### Decision 5: SQLite query builder as a private helper struct

**Choice:** Extract the dynamic SQL WHERE clause construction from `list()` into a private `ListQueryBuilder` struct within `sqlite.rs`. The builder encapsulates `where_clauses`, `params`, and `param_idx` tracking.

**Alternatives considered:**
- **Extract to `src/store/sqlite/query_builder.rs`**: Premature — only one call site. Keep it private within `sqlite.rs` until a second backend justifies a shared module.
- **Leave as-is**: The 88-line SQL construction with manual `param_idx` tracking is the most error-prone part. A builder encapsulates the index management and enables unit testing without a database.

**Rationale:** A private struct within `sqlite.rs` is the minimum viable extraction. It eliminates the `param_idx` bug class and makes query generation testable. No new files.

### Decision 6: Remove handler-level validation, keep service-level

**Choice:** Remove `validate_labels`, `validate_annotations`, `validate_finalizers` calls from the handler. The service already performs these validations as defense-in-depth. The handler's "fail-fast" benefit is marginal (saves microseconds of schema registry lookup).

**Alternatives considered:**
- **Keep both (current state)**: Duplication across 6 call sites. If validation rules change, both layers must be updated.
- **Extract shared `validate_metadata()` function**: Adds a function that's called from one place (the service). The service calls are already clear.

**Rationale:** The service is the single entry point for all mutations (HTTP, future gRPC, tests). Validation there covers all paths. Handler-level validation is redundant — the spec explicitly calls service validation "defense-in-depth," meaning the handler calls are the extra layer, not the other way around.

**Note:** This modifies the `object-handlers` spec requirements for eager validation. The handler spec's "Handler principle" is updated to remove format validation as a handler responsibility.

### Decision 7: Finalizer state machine as a standalone module

**Choice:** Extract `DeleteAction` enum, the `Arc<Mutex<DeleteAction>>` pattern, deletion guard logic, and hard-delete trigger into `src/object/finalizer.rs`. The module exposes pure functions: `evaluate_delete(existing: &StoredObject) -> DeleteAction` and `evaluate_update(existing: &StoredObject, incoming: &ObjectMeta) -> FinalizerDecision`.

**Alternatives considered:**
- **Keep in ObjectService**: The finalizer logic is transactionally coupled to the store. But the *decision* logic (what action to take) is pure — it only reads state and returns a decision. The *execution* (store transaction) stays in the service.
- **Extract as a trait**: Over-engineering for a state machine with 3 states.

**Rationale:** Separating decision from execution makes the state machine independently testable without mocks. The service calls `finalizer::evaluate_delete()` to decide, then executes the decision via `store.transaction()`. This is the same pattern as the OCC check — decision is pure, execution is effectful.

## Risks / Trade-offs

- **[SchemaService ownership of SchemaRegistry]** → SchemaRegistry moves from ObjectService to SchemaService. ObjectService still needs it for `get_validator()` calls during object validation. **Mitigation:** ObjectService receives `SchemaRegistry` as a constructor parameter (shared ownership via `Arc<SchemaRegistry>` or passed by value since it's already internally Arc-based). Alternatively, ObjectService holds its own `SchemaRegistry` reference passed at construction. The key constraint is that SchemaRegistry's `DashMap` cache is already thread-safe.
- **[Handler dispatch adds coupling]** → Handlers now depend on two services instead of one. **Mitigation:** Both services are behind `Arc`, so `AppState` just gains another field. The dispatch `if` is already present in the handler for Schema body parsing — we're not adding new complexity, just routing to a different service.
- **[Free functions lose discoverability]** → `publish_event` as a free function is harder to find than `self.publish_event()`. **Mitigation:** Group in a `pub(crate) mod helpers` with clear doc comments. The function signatures are self-documenting.
- **[Validation removal changes handler spec]** → Removing handler-level validation changes the contract described in `object-handlers` spec. **Mitigation:** The spec is updated to reflect that format validation is a service responsibility. Integration tests already test end-to-end behavior, so they remain valid.
- **[Finalizer extraction adds indirection]** → The service now calls `finalizer::evaluate_delete()` instead of inline logic. **Mitigation:** The function is pure and well-named. The service's `delete()` method becomes a clear sequence: evaluate → execute → publish. Readability improves.
