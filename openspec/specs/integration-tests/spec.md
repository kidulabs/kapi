## ADDED Requirements

### Requirement: Object CRUD flow via HTTP

The system SHALL allow clients to register a Schema, then perform full CRUD operations on objects of that kind via the HTTP API. The wire format SHALL use `metadata` for user-controlled fields (`name`) and `system` for server-controlled fields (`resourceVersion`, `createdAt`, `updatedAt`).

#### Scenario: Register schema and create object
- **WHEN** client POSTs a valid Schema to `/apis/kapi.io/v1/Schema`
- **THEN** response is 201 Created with `metadata.name` set to the generated schema name
- **AND** `system.resourceVersion`, `system.createdAt`, and `system.updatedAt` are populated in the response

#### Scenario: Read object
- **WHEN** client GETs `/apis/{group}/{version}/{kind}/{name}`
- **THEN** response is 200 OK with `metadata.name` and `system.resourceVersion` populated

#### Scenario: Update object
- **WHEN** client PUTs `/apis/{group}/{version}/{kind}/{name}` with a StoredObject matching the current `system.resourceVersion`
- **THEN** response is 200 OK with updated `system.resourceVersion` and `system.updatedAt`

#### Scenario: Update with correct resourceVersion uses system field
- **WHEN** client reads an object and updates it
- **THEN** the update request includes `system.resourceVersion` from the read response
- **AND** the update succeeds with a new `system.resourceVersion`

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

The system SHALL enforce optimistic concurrency on updates using `system.resourceVersion`.

#### Scenario: Update with correct resourceVersion succeeds
- **WHEN** client updates an object with the current `system.resourceVersion`
- **THEN** response is 200 OK with incremented `system.resourceVersion`

#### Scenario: Update with wrong resourceVersion returns conflict
- **WHEN** client updates an object with a stale `system.resourceVersion`
- **THEN** response is 409 Conflict with expected/actual version details

### Requirement: Cargo verification passes

The system SHALL maintain clean build and test status.

#### Scenario: All tests pass
- **WHEN** integration test binary runs
- **THEN** all test scenarios execute against each available store implementation
- **AND** the suite terminates immediately on first failure
- **AND** all tests pass with no warnings

#### Scenario: Clippy passes
- **WHEN** `cargo clippy -- -D warnings` runs
- **THEN** no warnings or errors

#### Scenario: Documentation builds
- **WHEN** `cargo doc --no-deps` runs
- **THEN** documentation generates without errors

### Requirement: Multi-store test execution

The integration test suite SHALL run all test scenarios against each registered store implementation.

#### Scenario: Tests run against InMemoryStore
- **WHEN** integration tests execute
- **THEN** all scenarios run with an InMemoryStore-backed TestApp

#### Scenario: Tests run against SQLiteStore
- **WHEN** integration tests execute
- **THEN** all scenarios run with a SQLiteStore-backed TestApp using a temporary database file
- **AND** the temporary file is deleted when the suite exits

#### Scenario: Test output groups by store
- **WHEN** integration tests execute against multiple stores
- **THEN** output is grouped by store name with a header (e.g., `=== memory ===`)
- **AND** each test within a group shows pass/fail status

### Requirement: Modular TestApp construction

TestApp SHALL support construction with an arbitrary `Arc<dyn ObjectStore>` and SHALL NOT provide a default no-argument constructor.

#### Scenario: TestApp created with explicit store
- **WHEN** test code calls `TestApp::with_store(store)`
- **THEN** the returned TestApp uses the provided store for all operations

#### Scenario: TestApp::new() does not exist
- **WHEN** code attempts to call `TestApp::new()`
- **THEN** compilation fails (method does not exist)

### Requirement: Store factory registration

The test harness SHALL provide a registry of available store implementations via a factory pattern.

#### Scenario: all_test_stores returns available stores
- **WHEN** `all_test_stores()` is called
- **THEN** it returns a Vec containing at least InMemoryStore and SQLiteStore factories
- **AND** each factory produces a fresh `Arc<dyn ObjectStore>` when invoked

#### Scenario: Future stores can be added
- **WHEN** a new store implementation is available
- **THEN** it can be added to `all_test_stores()` by appending a single `TestStore` entry