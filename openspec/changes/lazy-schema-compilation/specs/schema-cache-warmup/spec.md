## ADDED Requirements

### Requirement: Schema cache warmup on service initialization
The system SHALL load all existing Schema objects from the persistent store during `ObjectService::new()` and compile each schema's `jsonSchema` into the `schema_cache`. The warmup SHALL complete before the service begins accepting requests.

#### Scenario: Warmup loads all schemas from store
- **WHEN** `ObjectService::new(store, event_bus, meta_validator)` is called and the store contains Schema objects
- **THEN** all Schema objects are listed from the store, each `jsonSchema` is compiled, and the compiled validators are inserted into `schema_cache` under their respective names

#### Scenario: Warmup with empty store
- **WHEN** `ObjectService::new()` is called and the store contains no Schema objects
- **THEN** the `schema_cache` remains empty and no error occurs

#### Scenario: Warmup failure panics
- **WHEN** a Schema object in the store has a `jsonSchema` that fails compilation during warmup
- **THEN** the system panics with a message containing the schema name and compilation error

### Requirement: Lazy compilation fallback on cache miss
The `lookup_object_validator()` method SHALL compile a schema from the store and cache the result when a cache miss occurs. If compilation fails, the system SHALL panic.

#### Scenario: Cache miss triggers compilation
- **WHEN** `lookup_object_validator()` is called for a schema not in the cache but present in the store
- **THEN** the schema is read from the store, compiled, inserted into the cache, and the compiled validator is returned

#### Scenario: Cache miss with uncompilable schema panics
- **WHEN** `lookup_object_validator()` encounters a cache miss and the schema's `jsonSchema` fails compilation
- **THEN** the system panics with a message containing the schema name and compilation error
