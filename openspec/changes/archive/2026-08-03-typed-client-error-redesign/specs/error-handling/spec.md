## MODIFIED Requirements

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

#### Scenario: Client preserves structured details from error responses
- **WHEN** the kapi-client receives a structured error response with a `details` field
- **THEN** the client SHALL preserve the `details` value in `ClientError::ApiError` so that the typed client can extract structured fields (e.g., `what`, `identifier`, `kind`, `name`, `expected`, `actual`) for first-class error variants
