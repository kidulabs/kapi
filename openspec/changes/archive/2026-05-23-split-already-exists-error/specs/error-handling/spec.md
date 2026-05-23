## MODIFIED Requirements

### Requirement: Application errors are represented by a single enum
The system SHALL define an `AppError` enum that is the sole error type used across all services, stores, and handlers.

#### Scenario: Error variants cover all application failure modes
- **WHEN** an operation fails
- **THEN** the error SHALL be representable as one of: `NotFound`, `Conflict`, `AlreadyExists`, `SchemaValidation`, `SchemaHasObjects`, `InvalidSchema`, `StoredSchemaCompilationFailed`, or `Internal`

### Requirement: Conflict errors carry version information
The system SHALL produce `Conflict` errors with `expected` and `actual` version fields **exclusively** for optimistic concurrency control failures. The `Conflict` variant SHALL NOT be used for duplicate resource creation.

#### Scenario: Optimistic concurrency mismatch
- **WHEN** an update specifies `resourceVersion=5` but the stored version is `7`
- **THEN** the error SHALL be `Conflict { expected: 5, actual: 7 }`

### Requirement: Duplicate resource creation returns AlreadyExists
The system SHALL produce `AlreadyExists { kind, name }` errors when a `create` operation targets a resource that already exists. This replaces the previous behavior of returning `Conflict { expected: 0, actual: 0 }` for duplicates.

#### Scenario: Duplicate object create returns AlreadyExists
- **WHEN** creating an object with a name that already exists
- **THEN** the error SHALL be `AlreadyExists` with the resource `kind` and `name` populated

#### Scenario: Duplicate schema create returns AlreadyExists
- **WHEN** creating a Schema with a name that already exists
- **THEN** the error SHALL be `AlreadyExists` with `kind: "Schema"` and the schema name
