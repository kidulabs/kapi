## Purpose

Define the schema cache lazy compilation behavior that ensures compiled user schemas survive server restarts through on-demand compilation at first use.

## Requirements

### Requirement: Schema cache uses lazy compilation
The system SHALL NOT perform eager warmup of the `schema_cache` during `ObjectService::new()`. The cache SHALL start empty. When `lookup_object_validator()` encounters a cache miss for a schema that exists in the store, the system SHALL compile the schema on-demand, insert the compiled validator into the cache, and return it.

#### Scenario: Object creation after restart with empty cache
- **WHEN** the server restarts with Schema objects persisted in the store but an empty `schema_cache`
- **AND** a request creates an object of a kind whose Schema exists in the store
- **THEN** the schema is fetched from the store, compiled on-demand, cached, and used for validation
- **AND** the object creation succeeds

#### Scenario: Concurrent cache misses compile independently
- **WHEN** multiple concurrent requests for the same kind encounter a cache miss
- **THEN** each request independently compiles the schema; the last insertion wins in the cache
- **AND** all requests proceed with a valid compiled validator

### Requirement: Stored schema compilation failure returns structured error
When lazy compilation of a stored schema fails, the system SHALL return `AppError::StoredSchemaCompilationFailed { schema_name, reason }` instead of panicking. This error SHALL map to HTTP 500 Internal Server Error.

#### Scenario: Stored schema fails compilation on cache miss
- **WHEN** `lookup_object_validator()` encounters a cache miss for a schema in the store
- **AND** the schema's `jsonSchema` fails compilation
- **THEN** the system returns `AppError::StoredSchemaCompilationFailed` with the schema name and compilation reason
- **AND** no cache entry is created for the failed schema
