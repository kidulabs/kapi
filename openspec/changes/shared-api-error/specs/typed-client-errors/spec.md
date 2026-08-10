## MODIFIED Requirements

### Requirement: ClientError wraps ApiError directly
The client SHALL define `ClientError` with an `Api(ApiError)` variant that wraps the shared `ApiError` enum from `kapi-core`. The client SHALL deserialize HTTP error responses directly into `ApiError` via serde, eliminating manual field extraction.

#### Scenario: ClientError::Api variant
- **WHEN** the client receives an HTTP error response
- **THEN** the client SHALL deserialize the response body into `ApiError` using serde
- **THEN** the error SHALL be returned as `ClientError::Api(ApiError)`

#### Scenario: NotFound error deserialization
- **WHEN** the server returns HTTP 404 with body `{"code": "NotFound", "details": {"what": "Namespace", "identifier": "missing"}}`
- **THEN** the client SHALL return `Err(ClientError::Api(ApiError::NotFound { what: "Namespace", identifier: "missing" }))`

#### Scenario: Conflict error deserialization
- **WHEN** the server returns HTTP 409 with body `{"code": "Conflict", "details": {"expected": 5, "actual": 7}}`
- **THEN** the client SHALL return `Err(ClientError::Api(ApiError::Conflict { expected: 5, actual: 7 }))`

#### Scenario: Unknown error code deserialization
- **WHEN** the server returns an error with `code: "NewError"` that is not in the `ApiError` enum
- **THEN** the client SHALL deserialize it as `ClientError::Api(ApiError::Unknown { code: "NewError", message: "...", details: {...} })`

### Requirement: ClientError retains transport and serialization variants
The `ClientError` enum SHALL retain variants for HTTP transport errors and JSON serialization errors, separate from API errors.

#### Scenario: HTTP transport error
- **WHEN** the HTTP request fails (network error, timeout, connection refused)
- **THEN** the error SHALL be `ClientError::HttpError(reqwest::Error)`

#### Scenario: JSON serialization error
- **WHEN** the response body cannot be parsed as JSON
- **THEN** the error SHALL be `ClientError::SerializationError(serde_json::Error)`

#### Scenario: Stream error
- **WHEN** a watch stream encounters an error
- **THEN** the error SHALL be `ClientError::StreamError(String)`

## REMOVED Requirements

### Requirement: TypedError provides first-class variants for branch-worthy API errors
**Reason**: `TypedError` is redundant now that `ClientError::Api(ApiError)` provides fully typed error variants. The shared `ApiError` enum in `kapi-core` already contains all the typed variants that `TypedError` previously provided.
**Migration**: Use `ClientError::Api(ApiError)` directly. Pattern match on `ApiError` variants instead of `TypedError` variants.

### Requirement: TypedError mapping is automatic via From trait
**Reason**: The `From<ClientError> for TypedError` implementation is no longer needed because `ClientError::Api(ApiError)` is already typed. The 32-line conversion with magic-string destructuring is eliminated.
**Migration**: Remove all `TypedError` usage. Use `ClientError` directly.

### Requirement: Field extraction uses defensive defaults
**Reason**: Field extraction via `details["what"]`, `details["identifier"]`, etc. is no longer needed. The `ApiError` enum variants contain typed fields that are deserialized directly by serde.
**Migration**: Access fields directly from `ApiError` variants: `ApiError::NotFound { what, identifier }` instead of `details["what"].as_str().unwrap_or("unknown")`.

### Requirement: TypedError retains Serialization variant for JSON errors
**Reason**: `TypedError` is removed entirely. JSON serialization errors are represented by `ClientError::SerializationError(serde_json::Error)`.
**Migration**: Match on `ClientError::SerializationError` instead of `TypedError::Serialization`.
