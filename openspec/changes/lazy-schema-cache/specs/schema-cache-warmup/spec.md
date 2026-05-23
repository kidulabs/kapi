## MODIFIED Requirements

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

## REMOVED Requirements

### Requirement: Schema cache warmup on service initialization
**Reason**: Eager warmup is unnecessary with lazy compilation. The cache starts empty and populates on-demand.
**Migration**: None. Behavior is superseded by lazy compilation.

#### Scenario: Warmup loads all schemas from store
**Reason**: Removed. No eager warmup occurs.

#### Scenario: Warmup with empty store
**Reason**: Removed. No eager warmup occurs.

#### Scenario: Warmup failure panics
**Reason**: Removed. Panic on compilation failure replaced with structured error.

### Requirement: Lazy compilation fallback on cache miss
**Reason**: This requirement is now the primary behavior, not a fallback. It is subsumed by the modified "Schema cache uses lazy compilation" requirement above.
**Migration**: None. Behavior is now the default.

#### Scenario: Cache miss triggers compilation
**Reason**: Moved into the modified "Schema cache uses lazy compilation" requirement.

#### Scenario: Cache miss with uncompilable schema panics
**Reason**: Removed. Panic replaced with `StoredSchemaCompilationFailed` error.
