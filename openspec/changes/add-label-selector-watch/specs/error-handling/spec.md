## MODIFIED Requirements

### Requirement: InvalidLabelSelector error variant
`AppError` SHALL include an `InvalidLabelSelector(String)` variant for label selector parse failures. This variant SHALL map to HTTP 400 Bad Request with reason `"InvalidLabelSelector"` and a descriptive error message.

#### Scenario: InvalidLabelSelector error response
- **WHEN** an `AppError::InvalidLabelSelector("malformed selector: 'invalid selector'")` is returned
- **THEN** the HTTP response SHALL have status 400, reason `"InvalidLabelSelector"`, and a JSON body with the error message

#### Scenario: InvalidLabelSelector error display
- **WHEN** an `InvalidLabelSelector` error is formatted
- **THEN** the error message SHALL be prefixed with `"invalid label selector: "` for consistency
