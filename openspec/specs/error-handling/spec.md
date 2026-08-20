## Purpose

Define the application-wide error handling strategy including a unified error enum, structured error context, and HTTP response mapping.
## Requirements
### Requirement: Application errors are represented by a single enum
The system SHALL define an `AppError` enum in `kapi-server` that wraps `ApiError` for all structured domain errors. The `AppError` enum SHALL be the sole error type used across all services, stores, and handlers in the server.

#### Scenario: Error variants cover all application failure modes
- **WHEN** an operation fails
- **THEN** the error SHALL be representable as one of: `AppError::Api(ApiError)` for structured domain errors, `AppError::Internal(anyhow::Error)` for unexpected failures, `AppError::InvalidSchema(String)` for schema registration errors, or `AppError::StoredSchemaCompilationFailed { schema_name, reason }` for schema compilation errors

#### Scenario: AppError wraps ApiError
- **WHEN** a handler returns a domain error like `NotFound`
- **THEN** it SHALL be constructed as `AppError::Api(ApiError::NotFound { what, identifier })`

#### Scenario: Internal error wraps anyhow
- **WHEN** an underlying operation returns `anyhow::Error`
- **THEN** propagating it with `?` SHALL produce `AppError::Internal`

### Requirement: InvalidLabel error response
The server SHALL produce `ApiError::InvalidRequest { what: "label", message }` for invalid label errors.

#### Scenario: InvalidLabel error response
- **WHEN** an `ApiError::InvalidRequest { what: "label", message: "label key 'invalid key!' contains invalid characters" }` is returned
- **THEN** the HTTP response SHALL have status 400, code `"InvalidRequest"`, and details `{"what": "label", "message": "..."}`

### Requirement: InvalidLabelSelector error variant
The server SHALL produce `ApiError::InvalidRequest { what: "label selector", message }` for label selector parse failures.

#### Scenario: InvalidLabelSelector error response
- **WHEN** an `ApiError::InvalidRequest { what: "label selector", message: "malformed selector: 'invalid selector'" }` is returned
- **THEN** the HTTP response SHALL have status 400, code `"InvalidRequest"`, and details `{"what": "label selector", "message": "..."}`

### Requirement: InvalidRequestBody error variant
The server SHALL produce `ApiError::InvalidRequest { what: "request body", message }` for request body validation failures.

#### Scenario: InvalidRequestBody error response
- **WHEN** an `ApiError::InvalidRequest { what: "request body", message: "'spec' field is required" }` is returned
- **THEN** the HTTP response SHALL have status 400, code `"InvalidRequest"`, and details `{"what": "request body", "message": "..."}`

#### Scenario: InvalidRequestBody for missing spec
- **WHEN** a create request is missing the `spec` field
- **THEN** the error SHALL be `ApiError::InvalidRequest { what: "request body", message: "'spec' field is required" }`

#### Scenario: InvalidRequestBody for empty spec
- **WHEN** a create request contains `spec: {}`
- **THEN** the error SHALL be `ApiError::InvalidRequest { what: "request body", message: "'spec' must not be empty" }`

#### Scenario: InvalidRequestBody for non-object spec
- **WHEN** a create request contains `spec` as a non-object type
- **THEN** the error SHALL be `ApiError::InvalidRequest { what: "request body", message: "'spec' must be a JSON object" }`

#### Scenario: InvalidRequestBody for unknown fields
- **WHEN** a create request contains top-level fields other than `metadata` and `spec`
- **THEN** the error SHALL be `ApiError::InvalidRequest { what: "request body", message: "..." }` with a message indicating the unknown field(s)

### Requirement: NotFound errors carry structured context
The system SHALL produce `NotFound` errors with `what` and `identifier` fields so that error messages are unambiguous.

#### Scenario: Missing schema
- **WHEN** a request references a schema that does not exist
- **THEN** the error SHALL be `NotFound { what: "schema", identifier: "example.io/v1/Widget" }`

#### Scenario: Missing object
- **WHEN** a request references an object that does not exist
- **THEN** the error SHALL be `NotFound { what: "object", identifier: "my-widget" }`

### Requirement: Conflict errors carry version information
The system SHALL produce `Conflict` errors with `expected` and `actual` version fields **exclusively** for optimistic concurrency control failures. The `Conflict` variant SHALL NOT be used for duplicate resource creation.

#### Scenario: Optimistic concurrency mismatch
- **WHEN** an update specifies `resourceVersion=5` but the stored version is `7`
- **THEN** the error SHALL be `Conflict { expected: 5, actual: 7 }`

### Requirement: AlreadyExists error represents duplicate resource creation
The system SHALL produce `AlreadyExists { kind: String, name: String }` errors when a `create` operation targets a resource name that already exists within the same scope. The `kind` field SHALL contain the resource kind (e.g., "Widget", "Schema"), and the `name` field SHALL contain the resource name.

#### Scenario: Duplicate object creation
- **WHEN** creating an object with a name that already exists
- **THEN** the error SHALL be `AlreadyExists { kind: "Widget", name: "my-widget" }`

#### Scenario: Duplicate schema creation
- **WHEN** creating a Schema with a name that already exists
- **THEN** the error SHALL be `AlreadyExists { kind: "Schema", name: "Widget.example.io" }`

### Requirement: AlreadyExists maps to HTTP 409
The system SHALL map `ApiError::AlreadyExists` to HTTP 409 Conflict with JSON body `{"code": "AlreadyExists", "details": {"kind": "...", "name": "..."}}`.

#### Scenario: AlreadyExists response body
- **WHEN** a handler returns `ApiError::AlreadyExists { kind: "Widget", name: "my-widget" }`
- **THEN** the response is HTTP 409 with JSON body containing `"code": "AlreadyExists"` and `"details": {"kind": "Widget", "name": "my-widget"}`

### Requirement: Schema validation errors are structured
The system SHALL produce `ApiError::SchemaValidation` errors as a list of validation error strings.

#### Scenario: Invalid object payload
- **WHEN** a create or update request fails JSON Schema validation
- **THEN** the error SHALL contain `SchemaValidation { errors: Vec<String> }` where each string describes a validation failure

### Requirement: Internal errors wrap anyhow::Error
The system SHALL allow any `anyhow::Error` to convert automatically into `AppError::Internal` via the `?` operator.

#### Scenario: Unexpected store failure
- **WHEN** an underlying operation returns `anyhow::Error`
- **THEN** propagating it with `?` SHALL produce `AppError::Internal`

### Requirement: Errors map to structured HTTP responses
The system SHALL implement `axum::response::IntoResponse` for `AppError` so that every error variant maps to a specific HTTP status code and a structured JSON body. The implementation SHALL delegate to `ApiError::http_status()` and serde serialization for `AppError::Api` variants.

#### Scenario: NotFound maps to 404
- **WHEN** `AppError::Api(ApiError::NotFound { .. })` is returned from a handler
- **THEN** the response SHALL be HTTP 404 with JSON body `{"code": "NotFound", "details": {"what": "...", "identifier": "..."}}`

#### Scenario: Conflict maps to 409
- **WHEN** `AppError::Api(ApiError::Conflict { .. })` is returned from a handler
- **THEN** the response SHALL be HTTP 409 with JSON body `{"code": "Conflict", "details": {"expected": N, "actual": M}}`

#### Scenario: SchemaValidation maps to 422
- **WHEN** `AppError::Api(ApiError::SchemaValidation { .. })` is returned from a handler
- **THEN** the response SHALL be HTTP 422 with JSON body `{"code": "SchemaValidation", "details": {"errors": [...]}}`

#### Scenario: Internal maps to 500
- **WHEN** `AppError::Internal(anyhow_error)` is returned from a handler
- **THEN** the response SHALL be HTTP 500 with JSON body `{"code": "Internal", "details": {"message": "internal error"}}`
- **THEN** the server SHALL log the full anyhow error with tracing

#### Scenario: IntoResponse implementation is simplified
- **WHEN** the `IntoResponse` impl for `AppError` is written
- **THEN** it SHALL be approximately 30 lines (down from 142 lines)
- **THEN** it SHALL delegate to `ApiError::http_status()` and serde for `AppError::Api` variants

### Requirement: InvalidSchema error for broken schema registrations
The system SHALL produce `InvalidSchema` errors when a schema registration fails meta-schema validation or when the nested `jsonSchema` fails compilation.

#### Scenario: Meta-schema validation failure
- **WHEN** a Schema registration is missing required fields (targetGroup, targetVersion, targetKind, jsonSchema)
- **THEN** the error SHALL be `InvalidSchema` with a description of the validation failure

#### Scenario: Nested jsonSchema compilation failure
- **WHEN** a Schema registration contains a `jsonSchema` that cannot be compiled as a valid JSON Schema
- **THEN** the error SHALL be `InvalidSchema` with the compilation error message

### Requirement: InvalidSchema maps to HTTP 422
The system SHALL map `AppError::InvalidSchema` to HTTP 422 Unprocessable Entity with JSON body `{"code": "InvalidSchema", "details": {"message": "..."}}`.

#### Scenario: InvalidSchema response body
- **WHEN** a handler returns `AppError::InvalidSchema("missing field: targetGroup")`
- **THEN** the response is HTTP 422 with JSON body containing `"code": "InvalidSchema"` and `"details": {"message": "missing field: targetGroup"}`

### Requirement: SchemaHasObjects error blocks schema deletion
The system SHALL produce `SchemaHasObjects` errors when attempting to delete a Schema that has existing objects of the target kind.

#### Scenario: Delete schema with existing objects
- **WHEN** a DELETE request targets a Schema and objects of the target kind exist
  - **THEN** the error SHALL be `SchemaHasObjects { kind: "..." }`

### Requirement: SchemaHasObjects maps to HTTP 409
The system SHALL map `ApiError::SchemaHasObjects` to HTTP 409 Conflict with JSON body `{"code": "SchemaHasObjects", "details": {"kind": "..."}}`.

#### Scenario: SchemaHasObjects response body
- **WHEN** a handler returns `ApiError::SchemaHasObjects { kind: "Widget" }`
- **THEN** the response is HTTP 409 with JSON body containing `"code": "SchemaHasObjects"` and `"details": {"kind": "Widget"}`

