## Why

When the server restarts with a persistent store, Schema objects exist in storage but the in-memory `schema_cache` is empty. Object creation and updates fail with "compiled schema not found" because the cache was only populated during schema registration. The system must survive restarts without requiring all schemas to be re-registered.

## What Changes

- Change `lookup_object_validator()` from **cache-only lookup** to **lazy compilation**: when a schema is not in cache but exists in the store, compile it on-demand and insert into the cache.
- Replace the **eager schema warmup at startup** in `ObjectService::new()` with a no-op (schema cache starts empty).
- Replace **panic on compilation failure** during lazy compilation with a new domain error `StoredSchemaCompilationFailed` that maps to HTTP 500 Internal Server Error.
- Update the `schema-cache-warmup` spec to remove eager warmup and panic requirements, adding lazy compilation and error handling requirements instead.

## Capabilities

### New Capabilities
- *(none — this is a behavior change to existing capabilities)*

### Modified Capabilities
- `schema-cache-warmup`: Replace eager warmup and panic-on-failure with lazy compilation and a new domain error for stored schema compilation failures.
- `object-service`: Update `lookup_object_validator()` to compile on cache miss; update error contract for compilation failures from panic to `StoredSchemaCompilationFailed`.

## Impact

- `src/object/service.rs`: Core logic change in `lookup_object_validator()` and `ObjectService::new()`.
- `src/error.rs`: New `AppError::StoredSchemaCompilationFailed` variant.
- `openspec/specs/schema-cache-warmup/spec.md`: Requirements delta (remove eager warmup, remove panic, add lazy compilation and error handling).
- `openspec/specs/object-service/spec.md`: Update scenarios for cache miss and compilation failure.

## Non-goals

- Cache warming at startup.
- Caching compilation failures (sentinel values) to avoid repeated compilation attempts.
- Multi-instance cache invalidation or distributed caching.
