## MODIFIED Requirements

### Requirement: InvalidLabel error variant
`AppError` SHALL include an `InvalidLabel(String)` variant for label validation failures. This variant SHALL map to HTTP 400 Bad Request with reason `"InvalidLabel"` and a descriptive error message.

#### Scenario: InvalidLabel error response
- **WHEN** an `AppError::InvalidLabel("label key 'invalid key!' contains invalid characters")` is returned
- **THEN** the HTTP response SHALL have status 400, reason `"InvalidLabel"`, and a JSON body with the error message

#### Scenario: InvalidLabel error display
- **WHEN** an `InvalidLabel` error is formatted
- **THEN** the error message SHALL be prefixed with `"invalid label: "` for consistency with `InvalidFieldSelector`
