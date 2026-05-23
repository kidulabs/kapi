## Context

The `ObjectService` maintains an in-memory `schema_cache: DashMap<String, Arc<dyn SchemaValidator>>` that maps schema names (e.g., `"Widget.example.io"`) to compiled JSON Schema validators. Currently, the cache is populated only when a user registers a Schema via `create()` or `update()`. The `lookup_object_validator()` method performs a cache lookup and returns `NotFound { what: "compiled schema" }` on a cache miss.

With persistence, the store survives restarts but the in-memory cache does not. After a restart, every object creation or update for a pre-existing kind fails because the compiled validator is missing from the cache.

The existing `schema-cache-warmup` spec describes an eager warmup approach (load all schemas at startup) and mandates panicking on compilation failure. During exploration we decided against both: no eager warmup, and return a domain error instead of panicking.

## Goals / Non-Goals

**Goals:**
- Object operations must succeed after server restart even if no schemas have been re-registered.
- Schema compilation failures during lazy lookup must surface as a clear, structured error instead of a panic.
- The fix must not introduce eager initialization, cache-warming, or sentinel caching of failures.

**Non-Goals:**
- Startup performance optimization (eager cache warmup).
- Deduplication of concurrent compilation for the same schema (acceptable wasted work).
- Caching negative results (failed compilations).

## Decisions

### 1. Lazy compilation on cache miss
**Decision**: When `lookup_object_validator()` encounters a cache miss, fetch the Schema object from the store, parse it, compile the `jsonSchema`, insert the result into `schema_cache`, and return it.

**Rationale**: This is the simplest way to guarantee correctness after restart without any startup-time behavior. The first request for each kind pays the compilation cost; subsequent requests are fast.

**Alternatives considered**:
- **Eager warmup at startup**: Rejected. Slower startup, fails loudly if any persisted schema is invalid, unnecessary given lazy compilation works.
- **No caching at all, compile every time**: Rejected. Schema compilation is non-trivial; repeated compilation on every object operation is wasteful.

### 2. Return `StoredSchemaCompilationFailed` instead of panic
**Decision**: Introduce a new `AppError` variant `StoredSchemaCompilationFailed { schema_name: String, reason: String }` that maps to HTTP 500.

**Rationale**: A persisted schema that fails compilation is a system integrity issue, not a user error. HTTP 500 is the correct semantic. Returning a structured error variant (rather than `AppError::Internal`) allows monitoring and alerting to distinguish this specific failure mode from generic internal errors.

**Alternatives considered**:
- **Panic**: Rejected. One bad schema out of many should not crash the entire server and deny service for all other kinds.
- **Map to `InvalidSchema` (422)**: Rejected. The user did not supply the schema; telling them it's invalid is misleading.
- **Map to `AppError::Internal`**: Rejected. Too generic — loses the structured "schema compilation" signal for operators.

### 3. Keep `InvalidSchema` unchanged for the registration path
**Decision**: The `compile_jsonschema()` helper continues to return `AppError::InvalidSchema` when called during Schema registration (`create`/`update` for `kind == "Schema"`). The new `StoredSchemaCompilationFailed` is only used in the lazy path inside `lookup_object_validator()`.

**Rationale**: The semantics differ by context. During registration, compilation failure means "the schema you just gave us is bad" → 422. During object creation, compilation failure means "the schema we already have is somehow broken" → 500.

## Risks / Trade-offs

| Risk | Mitigation |
|------|------------|
| Thundering herd on restart (N concurrent requests for the same kind all compile the schema) | Accepted. `DashMap` insert is atomic; last write wins. Deduplication is a future optimization, not required now. |
| Repeated 500s for a poisoned schema (every request for a broken kind pays compilation cost) | Accepted. No sentinel caching of failures. Operator must fix the schema. Logging at ERROR level with schema body will aid debugging. |
| Compilation cost on first request adds latency | Accepted. Compilation is typically fast (<1ms for simple schemas). If it becomes a problem, eager warmup or background compilation can be added later. |

## Migration Plan

This is a behavioral change, not a data migration.

1. Update `AppError` with the new variant.
2. Update `lookup_object_validator()` to compile on cache miss.
3. Remove any eager warmup logic from `ObjectService::new()` (if present).
4. Update spec files to reflect new behavior.
5. Add/update tests for the new lazy compilation and error paths.
6. No rollback strategy needed — the old behavior was broken for persistence.

## Open Questions

- None at this time.
