## ADDED Requirements

### Requirement: Schema registration
The system SHALL allow users to register a JSON Schema definition that defines a custom object kind.

#### Scenario: Successful schema registration
- **WHEN** user POSTs a valid JSON Schema to `/apis/kapi.io/v1/schemas`
- **THEN** the schema is stored and a 201 response is returned

#### Scenario: Invalid schema rejected
- **WHEN** user POSTs an invalid JSON Schema to `/apis/kapi.io/v1/schemas`
- **THEN** the system returns a 422 Unprocessable Entity response

### Requirement: Schema retrieval
The system SHALL allow users to retrieve a previously registered JSON Schema by its group, version, and kind.

#### Scenario: Get existing schema
- **WHEN** user GETs `/apis/kapi.io/v1/schemas/{group}/{version}/{kind}` for a registered schema
- **THEN** the system returns the stored JSON Schema as JSON

#### Scenario: Get missing schema
- **WHEN** user GETs `/apis/kapi.io/v1/schemas/{group}/{version}/{kind}` for a schema that does not exist
- **THEN** the system returns a 404 Not Found response

### Requirement: Schema listing
The system SHALL allow users to list all registered schemas.

#### Scenario: List all schemas
- **WHEN** user GETs `/apis/kapi.io/v1/schemas`
- **THEN** the system returns a JSON array of all registered schemas

### Requirement: Schema deletion
The system SHALL allow users to delete a previously registered schema.

#### Scenario: Delete existing schema
- **WHEN** user DELETEs `/apis/kapi.io/v1/schemas/{group}/{version}/{kind}` for a registered schema
- **THEN** the schema is removed and the deleted schema is returned

#### Scenario: Delete missing schema
- **WHEN** user DELETEs `/apis/kapi.io/v1/schemas/{group}/{version}/{kind}` for a schema that does not exist
- **THEN** the system returns a 404 Not Found response

### Requirement: Object creation with validation
The system SHALL allow users to create an object of a registered kind, validating the payload against the registered JSON Schema before admission.

#### Scenario: Create valid object
- **WHEN** user POSTs a valid object to `/apis/{group}/{version}/{kind}` that matches the registered schema
- **THEN** the object is stored and a 201 response is returned with the created object

#### Scenario: Create object with invalid schema
- **WHEN** user POSTs an object that fails schema validation to `/apis/{group}/{version}/{kind}`
- **THEN** the system returns a 422 Unprocessable Entity response with validation errors

#### Scenario: Create object for unregistered kind
- **WHEN** user POSTs an object to `/apis/{group}/{version}/{kind}` for a kind with no registered schema
- **THEN** the system returns a 404 Not Found response

### Requirement: Object retrieval
The system SHALL allow users to retrieve a specific object by kind and name.

#### Scenario: Get existing object
- **WHEN** user GETs `/apis/{group}/{version}/{kind}/{name}` for an existing object
- **THEN** the system returns the stored object as JSON

#### Scenario: Get missing object
- **WHEN** user GETs `/apis/{group}/{version}/{kind}/{name}` for an object that does not exist
- **THEN** the system returns a 404 Not Found response

### Requirement: Object listing
The system SHALL allow users to list all objects of a given kind.

#### Scenario: List objects
- **WHEN** user GETs `/apis/{group}/{version}/{kind}`
- **THEN** the system returns a JSON list of all objects of that kind

### Requirement: Object update with optimistic concurrency
The system SHALL allow users to update an object, requiring the client to provide the current `resourceVersion` to prevent lost updates.

#### Scenario: Successful update
- **WHEN** user PUTs an object to `/apis/{group}/{version}/{kind}/{name}` with the matching `resourceVersion`
- **THEN** the object is updated and returned with a new `resourceVersion`

#### Scenario: Conflict on update
- **WHEN** user PUTs an object with a `resourceVersion` that does not match the stored version
- **THEN** the system returns a 409 Conflict response

#### Scenario: Update with invalid data
- **WHEN** user PUTs an object that fails schema validation
- **THEN** the system returns a 422 Unprocessable Entity response

### Requirement: Object deletion
The system SHALL allow users to delete an object. Deletion MAY optionally require `resourceVersion`.

#### Scenario: Delete existing object
- **WHEN** user DELETEs `/apis/{group}/{version}/{kind}/{name}`
- **THEN** the object is removed and returned

### Requirement: Watch for changes
The system SHALL support watching for changes to objects via Server-Sent Events using the `?watch=true` query parameter on the list endpoint.

#### Scenario: Watch stream
- **WHEN** user GETs `/apis/{group}/{version}/{kind}?watch=true`
- **THEN** the system returns an SSE stream that emits events for object additions, modifications, and deletions

#### Scenario: Watch for unregistered kind
- **WHEN** user GETs `/apis/{group}/{version}/{kind}?watch=true` for an unregistered kind
- **THEN** the system returns a 404 Not Found response

### Requirement: OpenAPI schema
The system SHALL expose an OpenAPI schema endpoint and Swagger UI.

#### Scenario: OpenAPI JSON
- **WHEN** user GETs `/openapi.json`
- **THEN** the system returns the OpenAPI specification as JSON

#### Scenario: Swagger UI
- **WHEN** user GETs `/swagger-ui`
- **THEN** the system serves an interactive Swagger UI

### Requirement: Middleware stubs
The system SHALL provide Tower middleware stubs for authentication, metrics, and tracing.

#### Scenario: Request passes through middleware
- **WHEN** any request is received
- **THEN** it passes through the Tower middleware chain (auth, metrics, trace) before reaching the handler
