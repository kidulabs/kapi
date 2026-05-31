## Context

`ObjectService` currently owns four concerns: storage orchestration, event publishing, label validation, and schema management. The schema concern includes meta-schema validation, JSON Schema compilation, a `DashMap`-based cache with lazy lookup, cache insertion, and cache eviction. This accounts for ~120 lines of cohesive logic (`validate_meta_schema`, `compile_jsonschema`, `lookup_object_validator`, `map_validation_errors`) that is tightly coupled to itself but tangentially related to the service's core job of orchestrating validate → store → publish.

The service is ~500 lines of production code — not large, but the schema concern is the most self-contained extractable unit.

## Goals / Non-Goals

**Goals:**
- Extract schema compilation, caching, lookup, and eviction into a `SchemaRegistry` collaborator
- `SchemaRegistry` owns the store reference for cache-miss lookups (self-contained)
- `ObjectService` retains control of the atomic operation sequence (validate → store → publish)
- No new trait boundaries — `SchemaRegistry` is a concrete struct
- Existing tests continue to pass with minimal changes

**Non-Goals:**
- Middleware/decorator layer between `ObjectService` and `ObjectStore`
- Moving event publishing out of `ObjectService`
- Moving label validation out of `ObjectService`
- Trait abstraction over `SchemaRegistry`
- Changing the public API of `ObjectService`

## Decisions

### 1. `SchemaRegistry` as a concrete struct, not a trait

**Decision**: `SchemaRegistry` is a concrete struct, not behind `Arc<dyn SchemaRegistry>`.

**Rationale**: There is only one implementation. A trait adds a vtable dispatch and mock complexity with no benefit. If testing requires isolation, the struct's dependencies (`store`, `meta_validator`) are already behind traits and can be mocked.

**Alternatives considered**:
- **`Arc<dyn SchemaRegistry>` trait object**: Rejected. Single implementation, unnecessary indirection, harder to test (need to mock the registry itself rather than its dependencies).
- **Generic `SchemaRegistry<S: ObjectStore>`**: Rejected. The codebase already uses `Arc<dyn ObjectStore>` throughout; mixing generics with trait objects adds complexity.

### 2. `SchemaRegistry` owns the store reference

**Decision**: `SchemaRegistry` holds `Arc<dyn ObjectStore>` for cache-miss lookups. `ObjectService` does not fetch schemas from the store on behalf of the registry.

**Rationale**: Cache-miss logic (fetch → parse → compile → cache) is cohesive with the cache itself. If `ObjectService` owned this, it would need to know about cache internals (miss vs hit) to decide whether to fetch — leaking registry concerns into the orchestrator.

```
┌─────────────────────────────────────────────────────────────┐
│  Option A (chosen): Registry owns store                     │
│                                                             │
│  ObjectService.create()                                     │
│    → registry.get_validator(key)                            │
│        → cache hit? return                                  │
│        → cache miss? store.get() → compile → cache → return │
│    → validate data                                          │
│    → store.create()                                         │
│    → event_bus.publish()                                    │
│                                                             │
│  Option B (rejected): Service fetches, passes to registry   │
│                                                             │
│  ObjectService.create()                                     │
│    → store.get(schema_key)  ← service knows about cache    │
│    → registry.compile_or_lookup(schema_data)   misses       │
│    → validate data                                          │
│    → store.create()                                         │
│    → event_bus.publish()                                    │
└─────────────────────────────────────────────────────────────┘
```

**Alternatives considered**:
- **Service fetches schema, passes to registry**: Rejected. Leaks cache-miss awareness into `ObjectService`. The service would need to check the cache first, then conditionally fetch — coupling it to registry internals.

### 3. `SchemaRegistry` API surface

**Decision**: Four public methods:

| Method | Purpose | Called by |
|--------|---------|-----------|
| `validate_and_compile(data: &Value) -> Result<(SchemaData, Arc<dyn SchemaValidator>), AppError>` | Meta-schema validate + compile for Schema create/update | `ObjectService` Schema path |
| `get_validator(key: &ResourceKey) -> Result<Arc<dyn SchemaValidator>, AppError>` | Cache lookup with lazy compilation on miss | `ObjectService` object path |
| `insert(name: &str, validator: Arc<dyn SchemaValidator>)` | Cache insertion after successful store | `ObjectService` after store.create/update |
| `evict(name: &str)` | Cache eviction on Schema delete | `ObjectService` delete_schema |

**Rationale**: `validate_and_compile` returns both the parsed `SchemaData` (needed by `delete_schema` for the target kind) and the compiled validator. This avoids double-parsing. `insert` and `evict` are separate from `validate_and_compile` because insertion must happen *after* store success — the registry cannot know when the store operation completes.

**Alternatives considered**:
- **`validate_and_compile` also inserts into cache**: Rejected. If `store.create` fails after compilation, the cache would have a stale entry for a schema that was never persisted. Insertion must be controlled by `ObjectService`.
- **Single `compile` method that handles both Schema and object paths**: Rejected. The Schema path validates against meta-schema first; the object path does not. Different error semantics (`InvalidSchema` vs `StoredSchemaCompilationFailed`).

### 4. Error semantics preserved

**Decision**: Error variants remain unchanged:
- Schema registration path: `AppError::InvalidSchema` (user-supplied schema is bad → 422)
- Cache-miss compilation path: `AppError::StoredSchemaCompilationFailed` (persisted schema is broken → 500)

**Rationale**: The semantic distinction is by context, not by location. `validate_and_compile` is called during registration (user error), `get_validator` is called during object operations (system integrity issue). The registry methods return the appropriate error variant based on which method is called.

### 5. Label validation stays in `ObjectService`

**Decision**: `validate_labels`, `validate_label_key`, `validate_label_value` remain as free functions in `service.rs` (or move to a `validation.rs` submodule within `object/`).

**Rationale**: Label validation is cross-cutting — it applies to all kinds (Schema and object). It has no dependency on schema compilation or caching. It is not cohesive with the schema concern.

## Risks / Trade-offs

| Risk | Mitigation |
|------|------------|
| `SchemaRegistry` holds `Arc<dyn ObjectStore>` — same store as `ObjectService` | Accepted. Both hold `Arc` clones to the same store. No ownership conflict. The store is already shared (e.g., between `ObjectService` and integration tests). |
| `SchemaRegistry::get_validator` is async (store fetch on cache miss) | Accepted. The method is already async in the current code. No change in async surface. |
| `validate_and_compile` returns `SchemaData` — couples registry to the `SchemaData` type | Accepted. `SchemaData` is defined in `object::types` and is already a shared type. The registry needs it to extract `json_schema` for compilation. |
| Tests that assert on `schema_cache` internals need updating | Mitigated. Tests access `service.schema_cache` directly. After extraction, they access `service.schema_registry.cache` or use a test helper. The cache is still a `DashMap` — same assertion patterns. |

## Migration Plan

This is a pure refactoring — no data migration, no API changes.

1. Create `src/schema/registry.rs` with `SchemaRegistry` struct
2. Move schema-related methods from `ObjectService` to `SchemaRegistry`
3. Update `ObjectService` to hold `SchemaRegistry` instead of `meta_validator` + `schema_cache`
4. Update `ObjectService::new()` signature
5. Update call sites (`src/routes.rs`)
6. Update tests to access cache through registry
7. Run full test suite — no behavioral changes expected

## Open Questions

- None at this time.
