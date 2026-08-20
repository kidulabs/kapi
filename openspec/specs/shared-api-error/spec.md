## Purpose

Define the shared `ApiError` enum that represents the wire contract between server and client, providing a single source of truth for API error types.

## Requirements

### Requirement: ApiError enum in kapi-core
The system SHALL define an `ApiError` enum in `kapi-core` that represents the structured error contract between server and client. The enum SHALL be serializable via serde and SHALL derive `thiserror::Error` for human-readable display messages.

#### Scenario: ApiError is defined in kapi-core
- **WHEN** the `kapi-core` crate is compiled
- **THEN** the `ApiError` enum SHALL be available as `kapi_core::error::ApiError`
- **THEN** the enum SHALL derive `Debug`, `Clone`, `Serialize`, `Deserialize`, `thiserror::Error`, `PartialEq`, `Eq`

#### Scenario: ApiError is re-exported by kapi-server and kapi-client
- **WHEN** `kapi-server` or `kapi-client` is compiled
- **THEN** both crates SHALL re-export `ApiError` from their public API

### Requirement: ApiError uses tagged serde serialization
The `ApiError` enum SHALL use `#[serde(tag = "code", content = "details", rename_all = "PascalCase")]` to serialize variant names as the `code` field and variant fields as the `details` object.

#### Scenario: NotFound serialization
- **WHEN** `ApiError::NotFound { what: "Namespace".into(), identifier: "missing".into() }` is serialized
- **THEN** the JSON SHALL be `{"code": "NotFound", "details": {"what": "Namespace", "identifier": "missing"}}`

#### Scenario: Conflict serialization
- **WHEN** `ApiError::Conflict { expected: 5, actual: 7 }` is serialized
- **THEN** the JSON SHALL be `{"code": "Conflict", "details": {"expected": 5, "actual": 7}}`

#### Scenario: InvalidRequest serialization
- **WHEN** `ApiError::InvalidRequest { what: "label".into(), message: "'foo' is not valid".into() }` is serialized
- **THEN** the JSON SHALL be `{"code": "InvalidRequest", "details": {"what": "label", "message": "'foo' is not valid"}}`

### Requirement: ApiError has http_status method
The `ApiError` enum SHALL provide a `pub fn http_status(&self) -> u16` method that maps each variant to its corresponding HTTP status code.

#### Scenario: NotFound maps to 404
- **WHEN** `ApiError::NotFound { .. }.http_status()` is called
- **THEN** it SHALL return `404`

#### Scenario: Conflict maps to 409
- **WHEN** `ApiError::Conflict { .. }.http_status()` is called
- **THEN** it SHALL return `409`

#### Scenario: SchemaValidation maps to 422
- **WHEN** `ApiError::SchemaValidation { .. }.http_status()` is called
- **THEN** it SHALL return `422`

#### Scenario: Internal maps to 500
- **WHEN** `ApiError::Internal { .. }.http_status()` is called
- **THEN** it SHALL return `500`

### Requirement: ApiError has 13 named variants plus Unknown
The `ApiError` enum SHALL have the following variants:

**Resource lifecycle (6):** `NotFound`, `AlreadyExists`, `Conflict`, `ObjectBeingDeleted`, `ProtectedNamespace`, `NamespaceNotEmpty`

**Schema/validation (4):** `SchemaValidation`, `InvalidSchema`, `SchemaHasObjects`, `StatusSubresourceNotEnabled`

**Generic invalid input (1):** `InvalidRequest { what: String, message: String }`

**Server internal (2):** `StoredSchemaCompilationFailed`, `Internal { message: String }`

**Forward compatibility (1):** `Unknown { code: String, message: String, details: serde_json::Value }`

#### Scenario: All variants are present
- **WHEN** the `ApiError` enum is defined
- **THEN** it SHALL contain exactly 13 named variants plus the `Unknown` catch-all

#### Scenario: Unknown variant captures unrecognized errors
- **WHEN** the client deserializes an error with `code: "NewError"` that is not in the enum
- **THEN** it SHALL deserialize as `ApiError::Unknown { code: "NewError", message: "...", details: {...} }`

### Requirement: ApiError is non-exhaustive
The `ApiError` enum SHALL be marked `#[non_exhaustive]` to allow future variants to be added without breaking existing match statements.

#### Scenario: External match requires wildcard
- **WHEN** external code matches on `ApiError`
- **THEN** the compiler SHALL require a wildcard arm (`_ => ...`) to handle future variants

### Requirement: InvalidRequest collapses 7 Invalid* variants
The `InvalidRequest { what: String, message: String }` variant SHALL replace the following server-side variants: `InvalidFieldSelector`, `InvalidLabel`, `InvalidAnnotation`, `InvalidFinalizer`, `InvalidLabelSelector`, `InvalidRequestBody`, `InvalidRequest`.

#### Scenario: Invalid label error
- **WHEN** the server encounters an invalid label
- **THEN** it SHALL produce `ApiError::InvalidRequest { what: "label".into(), message: "...".into() }`

#### Scenario: Invalid field selector error
- **WHEN** the server encounters an invalid field selector
- **THEN** it SHALL produce `ApiError::InvalidRequest { what: "field selector".into(), message: "...".into() }`

#### Scenario: Invalid request body error
- **WHEN** the server encounters an invalid request body
- **THEN** it SHALL produce `ApiError::InvalidRequest { what: "request body".into(), message: "...".into() }`

### Requirement: ApiError Display impl provides human-readable messages
Each `ApiError` variant SHALL have a `#[error("...")]` attribute that provides a human-readable message via `thiserror::Error`.

#### Scenario: NotFound display
- **WHEN** `ApiError::NotFound { what: "Namespace", identifier: "missing" }` is formatted with `{}`
- **THEN** the output SHALL be `"Namespace 'missing' not found"`

#### Scenario: InvalidRequest display
- **WHEN** `ApiError::InvalidRequest { what: "label", message: "'foo' is not valid" }` is formatted with `{}`
- **THEN** the output SHALL be `"invalid label: 'foo' is not valid"`
