## Why

After a server restart, the `schema_cache` in `ObjectService` starts empty. All user schemas are lost from memory even though they persist in SQLite. Any request to create or update objects for registered kinds fails with `NotFound("compiled schema")` until schemas are re-registered via the HTTP API. This makes restarts destructive to the runtime state and requires manual re-registration of all schemas.

## What Changes

- `ObjectService::new()` will load all existing Schema objects from the store and compile them into the `schema_cache` at startup
- `lookup_object_validator()` will fall back to compiling a schema on cache miss (lazy compilation) and cache the result
- If lazy compilation fails, the operation panics — a schema that exists in the store but cannot be compiled is a fatal configuration error
- No changes to the HTTP API or external behavior — this is purely an internal reliability fix

## Capabilities

### New Capabilities

- `schema-cache-warmup`: Schemas are pre-loaded from the persistent store into the in-memory cache during service initialization

### Modified Capabilities

- `object-service`: The `lookup_object_validator` method gains lazy compilation fallback on cache miss; `ObjectService::new` gains schema cache warmup logic

## Non-goals

- Schema hot-reloading without restart
- Schema versioning or migration
- Persisting the compiled cache to disk
- Graceful degradation when a stored schema fails to compile (this is a panic condition)

## Impact

- `src/object/service.rs`: `ObjectService::new`, `lookup_object_validator`
- `src/store/mod.rs`: No changes (existing `list` method is sufficient)
- Startup time increases proportionally to the number of stored schemas (compilation cost)
- No breaking changes to the public API
