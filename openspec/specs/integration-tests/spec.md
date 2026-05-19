## ADDED Requirements

### Requirement: Object CRUD flow via HTTP

The system SHALL allow clients to register a Schema, then perform full CRUD operations on objects of that kind via the HTTP API.

#### Scenario: Register schema and create object
- **WHEN** client POSTs a valid Schema to `/apis/kapi.io/v1/Schema`
- **THEN** response is 201 Created with the stored Schema object
- **AND** client can POST an object conforming to that schema to `/apis/{group}/{version}/{kind}`
- **AND** response is 201 Created with the stored object

#### Scenario: Read object
- **WHEN** client GETs `/apis/{group}/{version}/{kind}/{name}`
- **THEN** response is 200 OK with the stored object

#### Scenario: Update object
- **WHEN** client PUTs `/apis/{group}/{version}/{kind}/{name}` with a StoredObject matching the current resourceVersion
- **THEN** response is 200 OK with updated object and new resourceVersion

#### Scenario: Delete object
- **WHEN** client DELETEs `/apis/{group}/{version}/{kind}/{name}`
- **THEN** response is 200 OK with the deleted object
- **AND** subsequent GET returns 404

### Requirement: List pagination with continue tokens

The system SHALL support paginated object lists with continue tokens.

#### Scenario: Single page when limit exceeds items
- **WHEN** client creates 2 objects and lists with limit=5
- **THEN** response contains 2 items and no continue token

#### Scenario: Multiple pages with continue token
- **WHEN** client creates 4 objects and lists with limit=2
- **THEN** first response contains 2 items and a continue token
- **AND** second request with the continue token returns remaining 2 items and no continue token

#### Scenario: Pagination resumes from correct position
- **WHEN** client creates 4 objects named ["a", "b", "c", "d"] and lists with limit=2
- **THEN** first page contains ["a", "b"] with continue token pointing past "b"
- **AND** second page contains ["c", "d"] with no continue token

### Requirement: Watch events via SSE

The system SHALL stream watch events via SSE when `?watch=true` is specified on the list endpoint.

#### Scenario: Subscribe and receive event after object creation
- **WHEN** client subscribes to `/apis/{group}/{version}/{kind}?watch=true` BEFORE an object is created
- **AND** client creates an object at that path
- **THEN** the SSE stream receives an Added event within 2 seconds

#### Scenario: Watch event contains correct event type
- **WHEN** client watches the Schema collection and creates a Schema
- **THEN** the SSE event has eventType "Added" and the object matches the created Schema

### Requirement: Schema deletion guard

The system SHALL prevent deletion of a Schema when objects of the target kind exist.

#### Scenario: Delete schema with no objects
- **WHEN** a Schema exists for kind K with no objects of kind K
- **AND** client DELETEs `/apis/kapi.io/v1/Schema/{schemaName}`
- **THEN** response is 200 OK

#### Scenario: Delete schema with existing objects
- **WHEN** a Schema exists for kind K and N>0 objects of kind K exist
- **AND** client DELETEs `/apis/kapi.io/v1/Schema/{schemaName}`
- **THEN** response is 409 Conflict with details { kind: K, count: N }

### Requirement: Schema registration validation

The system SHALL reject Schema registrations with invalid jsonSchema.

#### Scenario: Valid schema accepted
- **WHEN** client POSTs a Schema with valid jsonSchema
- **THEN** response is 201 Created

#### Scenario: Invalid jsonSchema rejected
- **WHEN** client POSTs a Schema with jsonSchema containing invalid type (e.g., `"type": "not-a-real-type"`)
- **THEN** response is 422 Unprocessable Entity

#### Scenario: Missing required fields rejected
- **WHEN** client POSTs a Schema missing targetKind
- **THEN** response is 422 Unprocessable Entity

### Requirement: Optimistic concurrency control

The system SHALL enforce optimistic concurrency on updates using resourceVersion.

#### Scenario: Update with correct resourceVersion succeeds
- **WHEN** client updates an object with its current resourceVersion
- **THEN** response is 200 OK with updated object and incremented resourceVersion

#### Scenario: Update with wrong resourceVersion returns conflict
- **WHEN** client updates an object with resourceVersion different from current
- **THEN** response is 409 Conflict with expected/actual version details

### Requirement: Cargo verification passes

The system SHALL maintain clean build and test status.

#### Scenario: All tests pass
- **WHEN** `cargo test` runs
- **THEN** all tests pass with no warnings

#### Scenario: Clippy passes
- **WHEN** `cargo clippy -- -D warnings` runs
- **THEN** no warnings or errors

#### Scenario: Documentation builds
- **WHEN** `cargo doc --no-deps` runs
- **THEN** documentation generates without errors