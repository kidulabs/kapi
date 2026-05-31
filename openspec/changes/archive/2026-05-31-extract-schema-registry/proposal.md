## Why

`ObjectService` currently owns four distinct concerns: storage orchestration, event publishing, label validation, and schema management (meta-validation, compilation, caching, lookup, eviction). The schema concern alone accounts for ~120 lines of tightly coupled logic (`validate_meta_schema`, `compile_jsonschema`, `lookup_object_validator`, cache insert/evict) that is cohesive with itself but tangentially related to the service's core job of orchestrating validate → store → publish.

Extracting a `SchemaRegistry` collaborator isolates the schema concern without introducing a middleware layer between service and store. The service retains control of the atomic operation sequence; it simply delegates schema work to the registry.

## What Changes

- Create `SchemaRegistry` struct in `src/schema/registry.rs` owning:
  - `store: Arc<dyn ObjectStore>` — for cache-miss lookups
  - `meta_validator: Arc<dyn SchemaValidator>` — for meta-schema validation
  - `cache: DashMap<String, Arc<dyn SchemaValidator>>` — compiled validators
- `SchemaRegistry` exposes: `validate_and_compile(data)`, `get_validator(key)`, `insert(name, validator)`, `evict(name)`
- `ObjectService` replaces `meta_validator` and `schema_cache` fields with `schema_registry: SchemaRegistry`
- `ObjectService` methods delegate schema work to registry, retain store/event orchestration
- Update `object-service` spec to reflect new collaborator; create `schema-registry` spec

## Capabilities

### New Capabilities
- `schema-registry`: Schema compilation, caching, and lookup as a standalone collaborator

### Modified Capabilities
- `object-service`: Delegates schema concerns to `SchemaRegistry`; struct fields change; method bodies simplify

## Impact

- `src/schema/registry.rs`: New module — `SchemaRegistry` struct and impl
- `src/schema/mod.rs`: Re-export `SchemaRegistry`
- `src/object/service.rs`: Remove `meta_validator`, `schema_cache` fields; remove `validate_meta_schema`, `compile_jsonschema`, `lookup_object_validator`, `map_validation_errors`; simplify `validate_and_create_schema`, `validate_and_update_schema`, `validate_and_create_object`, `validate_and_update_object`, `delete_schema`
- `src/routes.rs`: Update `ObjectService::new()` call site
- `openspec/specs/object-service/spec.md`: Update struct requirements and scenarios
- `openspec/specs/schema-registry/spec.md`: New spec

## Non-goals

- Middleware/decorator layer between `ObjectService` and `ObjectStore`
- Moving event publishing out of `ObjectService` (for this change; remains an open exploration)
- Moving label validation out of `ObjectService` (for this change; remains an open exploration)
- Moving schema deletion guard out of `ObjectService` (cross-kind policy requiring store access)
- Trait abstraction over `SchemaRegistry` (concrete struct is sufficient)

## Future Work

- Consider extracting label validation to `src/object/validation.rs` if `service.rs` grows beyond ~800 lines of production code
- Consider `SchemaRegistry` warmup at startup if cold-start latency becomes a concern (currently lazy)
