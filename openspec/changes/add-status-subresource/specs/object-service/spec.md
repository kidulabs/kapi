## ADDED Requirements

### Requirement: ObjectService update_status method
The `ObjectService` SHALL provide an `update_status(key: ResourceKey, name: String, status: Value)` method that:
1. Looks up the Schema for the given kind to check if `statusSchema` is defined
2. If no `statusSchema` exists, returns `AppError::StatusSubresourceNotEnabled { kind }`
3. Fetches the current object from the store
4. Validates the status value against the `statusSchema`
5. If validation fails, returns `AppError::SchemaValidation`
6. Calls `store.update_status(key, name, status)` to perform the server-side read-modify-write
7. Publishes a `WatchEvent` with `event_type: StatusModified` containing the updated `StoredObject`
8. Returns the updated `StoredObject`

#### Scenario: Update status for kind with statusSchema
- **WHEN** `update_status` is called for a kind with `statusSchema` defined
- **THEN** the status is validated against `statusSchema`, stored, and a `StatusModified` event is published

#### Scenario: Update status for kind without statusSchema
- **WHEN** `update_status` is called for a kind without `statusSchema`
- **THEN** the error is `AppError::StatusSubresourceNotEnabled { kind }`

#### Scenario: Update status with invalid status
- **WHEN** `update_status` is called with a status value that fails `statusSchema` validation
- **THEN** the error is `AppError::SchemaValidation` with the list of validation errors

#### Scenario: Update status for non-existent object
- **WHEN** `update_status` is called for an object that does not exist
- **THEN** the error is `AppError::NotFound`

### Requirement: ObjectService create ignores status field
The `create` method SHALL ignore any `status` field present in the request body. Objects are always created with `status: None`.

#### Scenario: Create with status in body
- **WHEN** `create` is called with a body containing a `status` field
- **THEN** the `status` field SHALL be removed from the body before storage
- **AND** the created object SHALL have `status: None`

### Requirement: ObjectService get_status method
The `ObjectService` SHALL provide a `get_status(key: ResourceKey, name: String)` method that:
1. Fetches the object from the store
2. Looks up the Schema for the given kind to check if `statusSchema` is defined
3. If no `statusSchema` exists, returns `AppError::StatusSubresourceNotEnabled { kind }`
4. Returns the `status` field of the object (may be `None`/`null`)

#### Scenario: Get status for kind with statusSchema
- **WHEN** `get_status` is called for a kind with `statusSchema`
- **THEN** the `status` field of the object is returned

#### Scenario: Get status for kind without statusSchema
- **WHEN** `get_status` is called for a kind without `statusSchema`
- **THEN** the error is `AppError::StatusSubresourceNotEnabled { kind }`

## MODIFIED Requirements

### Requirement: Schema registration compiles status validator
When a Schema is created or updated with a `statusSchema` field, the `ObjectService` SHALL compile the `statusSchema` into a validator and cache it alongside the spec validator in the `SchemaRegistry`. The cache key for the status validator SHALL be `{kind}.{group}.status`.

#### Scenario: Schema with statusSchema is registered
- **WHEN** a Schema is created with `statusSchema` defined
- **THEN** both the spec validator and status validator are compiled and cached

#### Scenario: Schema without statusSchema is registered
- **WHEN** a Schema is created without `statusSchema`
- **THEN** only the spec validator is compiled and cached

#### Scenario: Schema with invalid statusSchema fails registration
- **WHEN** a Schema is created with a `statusSchema` that fails JSON Schema compilation
- **THEN** the error is `AppError::InvalidSchema` and nothing is stored or cached