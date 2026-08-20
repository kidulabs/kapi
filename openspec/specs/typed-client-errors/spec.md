## Purpose

Define the client-side error types for handling API responses and transport failures.

## Requirements

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
