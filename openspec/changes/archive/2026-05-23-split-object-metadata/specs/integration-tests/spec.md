## MODIFIED Requirements

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

### Requirement: Optimistic concurrency control
The system SHALL enforce optimistic concurrency on updates using `system.resourceVersion`.

#### Scenario: Update with correct resourceVersion succeeds
- **WHEN** client updates an object with the current `system.resourceVersion`
- **THEN** response is 200 OK with incremented `system.resourceVersion`

#### Scenario: Update with wrong resourceVersion returns conflict
- **WHEN** client updates an object with a stale `system.resourceVersion`
- **THEN** response is 409 Conflict with expected/actual version details