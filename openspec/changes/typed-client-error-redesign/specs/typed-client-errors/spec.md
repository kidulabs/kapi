## ADDED Requirements

### Requirement: TypedError provides first-class variants for branch-worthy API errors
The typed client SHALL define `TypedError` variants that allow callers to pattern-match on common API errors without inspecting raw HTTP status codes or string codes. The first-class variants SHALL be: `NotFound`, `AlreadyExists`, `Conflict`, and `Forbidden`.

#### Scenario: NotFound variant
- **WHEN** the server returns HTTP 404 with code `"NotFound"`
- **THEN** the typed client method SHALL return `Err(TypedError::NotFound { what, identifier })` where `what` and `identifier` are extracted from the response `details`

#### Scenario: AlreadyExists variant
- **WHEN** the server returns HTTP 409 with code `"AlreadyExists"`
- **THEN** the typed client method SHALL return `Err(TypedError::AlreadyExists { kind, name })` where `kind` and `name` are extracted from the response `details`

#### Scenario: Conflict variant
- **WHEN** the server returns HTTP 409 with code `"Conflict"`
- **THEN** the typed client method SHALL return `Err(TypedError::Conflict { expected, actual })` where `expected` and `actual` are extracted from the response `details`

#### Scenario: Forbidden variant
- **WHEN** the server returns HTTP 403 with code `"ProtectedNamespace"`
- **THEN** the typed client method SHALL return `Err(TypedError::Forbidden { message })` where `message` is the human-readable error string

#### Scenario: Non-mapped error falls through to ApiError
- **WHEN** the server returns an error that does not match any first-class variant's (status, code) pair
- **THEN** the typed client method SHALL return `Err(TypedError::ApiError(client_error))` containing the full `ClientError`

### Requirement: TypedError mapping is automatic via From trait
The typed client SHALL implement `From<ClientError> for TypedError` so that the `?` operator in typed methods automatically maps `ClientError` to the appropriate `TypedError` variant. Callers SHALL NOT need to perform manual error conversion.

#### Scenario: get method returns NotFound directly
- **WHEN** `typed_client.get(namespace, name).await` encounters a 404 NotFound response
- **THEN** the result SHALL be `Err(TypedError::NotFound { .. })` without any manual error mapping by the caller

#### Scenario: create method returns AlreadyExists directly
- **WHEN** `typed_client.create(namespace, obj).await` encounters a 409 AlreadyExists response
- **THEN** the result SHALL be `Err(TypedError::AlreadyExists { .. })` without any manual error mapping by the caller

### Requirement: Field extraction uses defensive defaults
When extracting fields from the server's `details` JSON, the typed client SHALL use defensive defaults (`unwrap_or("unknown")` for strings, `unwrap_or(0)` for numbers) to ensure it never panics on unexpected server responses.

#### Scenario: Missing detail field in NotFound response
- **WHEN** the server returns 404 with code `"NotFound"` but the `details` object is missing the `"what"` field
- **THEN** `TypedError::NotFound { what }` SHALL have `what` set to `"unknown"`

#### Scenario: Non-numeric Conflict detail
- **WHEN** the server returns 409 with code `"Conflict"` but `details["expected"]` is not a valid u64
- **THEN** `TypedError::Conflict { expected, actual }` SHALL have `expected` set to `0`

### Requirement: TypedError retains Serialization variant for JSON errors
The typed client SHALL retain the `TypedError::Serialization(serde_json::Error)` variant for JSON serialization and deserialization failures that occur in the typed client layer (e.g., `stored_to_typed`, `typed_to_stored`).

#### Scenario: Deserialization failure
- **WHEN** the server returns a valid response but the object cannot be deserialized into the target typed resource
- **THEN** the result SHALL be `Err(TypedError::Serialization(serde_error))`
