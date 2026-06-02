## MODIFIED Requirements

### Requirement: SchemaRegistry caches status validators
The `SchemaRegistry` SHALL cache status validators alongside spec validators. The cache key for status validators SHALL be `{kind}.{group}.status`. When a Schema with `statusSchema` is created or updated, the status validator SHALL be compiled and cached. When a Schema is deleted, both spec and status cache entries SHALL be evicted.

#### Scenario: Schema with statusSchema caches both validators
- **WHEN** a Schema with `statusSchema` is registered
- **THEN** the spec validator is cached under `{kind}.{group}` and the status validator is cached under `{kind}.{group}.status`

#### Scenario: Schema without statusSchema caches only spec validator
- **WHEN** a Schema without `statusSchema` is registered
- **THEN** only the spec validator is cached under `{kind}.{group}`

#### Scenario: Schema deletion evicts both validators
- **WHEN** a Schema with `statusSchema` is deleted
- **THEN** both `{kind}.{group}` and `{kind}.{group}.status` cache entries are evicted

### Requirement: SchemaRegistry get_status_validator method
The `SchemaRegistry` SHALL provide a `get_status_validator(&self, key: &ResourceKey)` method that returns the cached status validator for the given kind. On cache miss, it SHALL fetch the Schema from the store, parse `status_schema`, compile it, cache it, and return it. If the Schema has no `status_schema`, it SHALL return `AppError::StatusSubresourceNotEnabled`.

#### Scenario: Get status validator for kind with statusSchema
- **WHEN** `get_status_validator` is called for a kind with `statusSchema`
- **THEN** the compiled status validator is returned (from cache or on-demand compilation)

#### Scenario: Get status validator for kind without statusSchema
- **WHEN** `get_status_validator` is called for a kind without `statusSchema`
- **THEN** the error is `AppError::StatusSubresourceNotEnabled { kind }`

#### Scenario: Get status validator cache miss
- **WHEN** `get_status_validator` is called and the cache does not contain the status validator
- **THEN** the Schema is fetched from the store, `status_schema` is compiled, cached, and returned