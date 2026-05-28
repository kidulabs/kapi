## MODIFIED Requirements

### Requirement: Application errors are represented by a single enum
The system SHALL define an `AppError` enum that is the sole error type used across all services, stores, and handlers.

#### Scenario: Error variants cover all application failure modes
- **WHEN** an operation fails
- **THEN** the error SHALL be representable as one of: `NotFound`, `Conflict`, `AlreadyExists`, `InvalidLabel`, `SchemaValidation`, `SchemaHasObjects`, `InvalidSchema`, `StoredSchemaCompilationFailed`, or `Internal`

#### Scenario: InvalidLabel error response
- **WHEN** an `AppError::InvalidLabel("label key 'invalid key!' contains invalid characters")` is returned
- **THEN** the HTTP response SHALL have status 400, reason `"InvalidLabel"`, and a JSON body with the error message

#### Scenario: InvalidLabel error display
- **WHEN** an `InvalidLabel` error is formatted
- **THEN** the error message SHALL be prefixed with `"invalid label: "` for consistency with `InvalidFieldSelector`
