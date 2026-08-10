## MODIFIED Requirements

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

### Requirement: InvalidSchema maps to HTTP 422
The system SHALL map `AppError::InvalidSchema` to HTTP 422 Unprocessable Entity with JSON body `{"code": "InvalidSchema", "details": {"message": "..."}}`.

#### Scenario: InvalidSchema response body
- **WHEN** a handler returns `AppError::InvalidSchema("missing field: targetGroup")`
- **THEN** the response is HTTP 422 with JSON body containing `"code": "InvalidSchema"` and `"details": {"message": "missing field: targetGroup"}`

### Requirement: SchemaHasObjects maps to HTTP 409
The system SHALL map `ApiError::SchemaHasObjects` to HTTP 409 Conflict with JSON body `{"code": "SchemaHasObjects", "details": {"kind": "..."}}`.

#### Scenario: SchemaHasObjects response body
- **WHEN** a handler returns `ApiError::SchemaHasObjects { kind: "Widget" }`
- **THEN** the response is HTTP 409 with JSON body containing `"code": "SchemaHasObjects"` and `"details": {"kind": "Widget"}`

## REMOVED Requirements

### Requirement: Client preserves structured details from error responses
**Reason**: The client now deserializes `ApiError` directly via serde, eliminating the need for manual `details` preservation. The structured fields are part of the `ApiError` variant itself.
**Migration**: Use `ClientError::Api(ApiError)` which contains the full structured error. No manual extraction needed.
