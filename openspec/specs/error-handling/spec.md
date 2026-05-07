## Purpose

Define the application-wide error handling strategy including a unified error enum, structured error context, and HTTP response mapping.

## Requirements

### Requirement: Application errors are represented by a single enum
The system SHALL define an `AppError` enum that is the sole error type used across all services, stores, and handlers.

#### Scenario: Error variants cover all application failure modes
- **WHEN** an operation fails
- **THEN** the error SHALL be representable as one of: `NotFound`, `Conflict`, `SchemaValidation`, or `Internal`

### Requirement: NotFound errors carry structured context
The system SHALL produce `NotFound` errors with `what` and `identifier` fields so that error messages are unambiguous.

#### Scenario: Missing schema
- **WHEN** a request references a schema that does not exist
- **THEN** the error SHALL be `NotFound { what: "schema", identifier: "example.io/v1/Widget" }`

#### Scenario: Missing object
- **WHEN** a request references an object that does not exist
- **THEN** the error SHALL be `NotFound { what: "object", identifier: "my-widget" }`

### Requirement: Conflict errors carry version information
The system SHALL produce `Conflict` errors with `expected` and `actual` version fields for optimistic concurrency failures.

#### Scenario: Optimistic concurrency mismatch
- **WHEN** an update specifies `resourceVersion=5` but the stored version is `7`
- **THEN** the error SHALL be `Conflict { expected: 5, actual: 7 }`

### Requirement: Schema validation errors are structured
The system SHALL produce `SchemaValidation` errors as a list of `ValidationError` objects, each with a `path` (JSON pointer) and `message`.

#### Scenario: Invalid object payload
- **WHEN** a create or update request fails JSON Schema validation
- **THEN** the error SHALL contain `ValidationError { path: "/spec/replicas", message: "must be >= 0" }`

### Requirement: Internal errors wrap anyhow::Error
The system SHALL allow any `anyhow::Error` to convert automatically into `AppError::Internal` via the `?` operator.

#### Scenario: Unexpected store failure
- **WHEN** an underlying operation returns `anyhow::Error`
- **THEN** propagating it with `?` SHALL produce `AppError::Internal`

### Requirement: Errors map to structured HTTP responses
The system SHALL implement `axum::response::IntoResponse` for `AppError` so that every error variant maps to a specific HTTP status code and a structured JSON body.

#### Scenario: NotFound maps to 404
- **WHEN** `AppError::NotFound` is returned from a handler
- **THEN** the response SHALL be HTTP 404 with JSON body `{ "error": "...", "code": "NotFound", "details": { "what": "...", "identifier": "..." } }`

#### Scenario: Conflict maps to 409
- **WHEN** `AppError::Conflict` is returned from a handler
- **THEN** the response SHALL be HTTP 409 with JSON body `{ "error": "...", "code": "Conflict", "details": { "expected": N, "actual": M } }`

#### Scenario: SchemaValidation maps to 422
- **WHEN** `AppError::SchemaValidation` is returned from a handler
- **THEN** the response SHALL be HTTP 422 with JSON body `{ "error": "...", "code": "SchemaValidation", "details": { "errors": [...] } }`

#### Scenario: Internal maps to 500
- **WHEN** `AppError::Internal` is returned from a handler
- **THEN** the response SHALL be HTTP 500 with JSON body `{ "error": "internal error", "code": "Internal", "details": null }`
