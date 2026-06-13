## ADDED Requirements

### Requirement: InvalidRequestBody error variant
`AppError` SHALL include an `InvalidRequestBody(String)` variant for request body validation failures. This variant SHALL map to HTTP 400 Bad Request with reason `"InvalidRequestBody"` and a descriptive error message.

#### Scenario: InvalidRequestBody error response
- **WHEN** an `AppError::InvalidRequestBody("'spec' field is required")` is returned
- **THEN** the HTTP response SHALL have status 400, reason `"InvalidRequestBody"`, and a JSON body with the error message

#### Scenario: InvalidRequestBody for missing spec
- **WHEN** a create request is missing the `spec` field
- **THEN** the error SHALL be `InvalidRequestBody("'spec' field is required")`

#### Scenario: InvalidRequestBody for empty spec
- **WHEN** a create request contains `spec: {}`
- **THEN** the error SHALL be `InvalidRequestBody("'spec' must not be empty")`

#### Scenario: InvalidRequestBody for non-object spec
- **WHEN** a create request contains `spec` as a non-object type
- **THEN** the error SHALL be `InvalidRequestBody("'spec' must be a JSON object")`

#### Scenario: InvalidRequestBody for unknown fields
- **WHEN** a create request contains top-level fields other than `metadata` and `spec`
- **THEN** the error SHALL be `InvalidRequestBody` with a message indicating the unknown field(s)
