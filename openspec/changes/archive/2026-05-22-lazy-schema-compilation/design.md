## Context

`ObjectService` maintains an in-memory `schema_cache: DashMap<String, Arc<dyn SchemaValidator>>` that stores compiled JSON Schema validators. The cache is populated when Schema objects are created or updated via the HTTP API, and evicted when Schema objects are deleted. The cache starts empty on every server restart (`DashMap::new()` at `service.rs:49`).

When a regular object is created or updated, `lookup_object_validator()` reads the Schema definition from the persistent store, parses it, builds a cache key, and then checks the cache. If the cache miss occurs — which happens for all schemas after a restart — the method returns `NotFound("compiled schema")` even though the schema exists in the store.

The `ObjectStore` trait already provides a `list()` method that can enumerate all Schema objects, so pre-loading is feasible with the existing interface.

## Goals / Non-Goals

**Goals:**
- Restore compiled schema cache from persistent store on startup
- Provide lazy compilation fallback for any schema not found in cache
- Fail loudly (panic) if a stored schema cannot be compiled — this indicates data corruption or a bug
- Keep the change localized to `ObjectService` — no API changes

**Non-Goals:**
- Schema hot-reloading without restart
- Schema versioning or migration
- Persisting the compiled cache to disk
- Graceful degradation for uncompilable schemas

## Decisions

### Decision 1: Eager warmup at startup + lazy fallback

**Approach:** Both strategies are implemented. On startup, `ObjectService::new()` lists all Schema objects from the store and compiles them into the cache. Additionally, `lookup_object_validator()` compiles on cache miss as a safety net.

**Rationale:** Eager warmup ensures all schemas are ready before the first request arrives. The lazy fallback handles edge cases (e.g., a schema created concurrently during warmup). Together they provide complete coverage.

**Alternatives considered:**
- *Lazy only:* Simpler, but the first request after restart pays compilation cost for all schemas. With eager warmup, the cost is paid once at startup.
- *Eager only:* If a new schema is created while warmup is running (race condition), it would fail. The lazy fallback eliminates this race.

### Decision 2: Panic on compilation failure during lazy compile

**Approach:** If `compile_jsonschema()` fails during a cache miss in `lookup_object_validator()`, the method panics with a descriptive message.

**Rationale:** A schema that exists in the store but cannot be compiled represents a fatal inconsistency. This can only happen due to data corruption or a jsonschema crate bug. Continuing to serve requests without validation would be worse than crashing. The panic message includes the schema name and compilation error for debugging.

**Alternatives considered:**
- *Return error:* The caller would receive `AppError::InvalidSchema` and return a 500 to the client. The server stays up but operates with an unvalidated schema — silently accepting invalid objects. This is unacceptable for a schema-driven API platform.
- *Background compilation with circuit breaker:* Too complex for this use case. The failure mode is rare and should be fatal.

### Decision 3: Warmup is synchronous in `new()`

**Approach:** Schema warmup happens synchronously during `ObjectService::new()`. The server does not start accepting requests until all schemas are compiled.

**Rationale:** This is the simplest correct approach. The alternative — async background warmup — introduces complexity (what happens if a request arrives before warmup completes?) without meaningful benefit. Schema compilation is fast (sub-millisecond for typical JSON Schemas), and the number of schemas is expected to be small (tens, not thousands).

**Alternatives considered:**
- *Async background warmup:* Would require a "ready" flag, request queuing, or lazy fallback for all requests during warmup. More complexity, marginal benefit.
- *Lazy only:* See Decision 1.

### Decision 4: Use existing `store.list()` with no pagination for warmup

**Approach:** Call `store.list()` with `limit: None` to get all Schema objects in one call.

**Rationale:** The number of schemas is expected to be small. Pagination adds complexity without benefit. The existing `ListOptions` struct supports `limit: None` for unbounded listing.

## Risks / Trade-offs

| Risk | Mitigation |
|------|-----------|
| Slow startup with many schemas | Expected schema count is small (tens). If this grows, switch to lazy-only approach. |
| Warmup failure crashes server at startup | This is intentional — a server with broken schemas should not start. Panic message includes schema name for debugging. |
| Race condition: schema created during warmup | Lazy fallback in `lookup_object_validator()` handles this. |
| Memory usage grows with schema count | Compiled validators are small (typically <10KB each). DashMap overhead is minimal. |
