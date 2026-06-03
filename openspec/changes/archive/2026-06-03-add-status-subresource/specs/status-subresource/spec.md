## ADDED Requirements

### Requirement: Status subresource provides separate write path for observed state
The system SHALL provide a status subresource for kinds that have a `statusSchema` defined in their Schema registration. The status subresource allows controllers to update observed state without conflicting with spec updates. Kinds without a `statusSchema` SHALL NOT have a status subresource.

#### Scenario: Kind with statusSchema has status subresource
- **WHEN** a Schema is registered with `statusSchema` defined
- **THEN** objects of that kind SHALL have a `status` field that can be read and updated via the `/status` endpoint

#### Scenario: Kind without statusSchema has no status subresource
- **WHEN** a Schema is registered without `statusSchema`
- **THEN** objects of that kind SHALL have `status: null` and the `/status` endpoint SHALL return `StatusSubresourceNotEnabled`

#### Scenario: Status starts as null on create
- **WHEN** an object is created via `POST /apis/{g}/{v}/{kind}`
- **THEN** the `status` field SHALL be `null` regardless of whether a `status` field is present in the request body

### Requirement: GET /status returns status sub-object
The system SHALL provide a `GET /apis/{group}/{version}/{kind}/{name}/status` endpoint that returns only the `status` field of the object. For kinds without `statusSchema`, it SHALL return `StatusSubresourceNotEnabled`.

#### Scenario: Get status for kind with statusSchema
- **WHEN** a GET request is made to `/status` for an object of a kind with `statusSchema`
- **THEN** the response SHALL be the `status` field value (may be `null` if not yet set)

#### Scenario: Get status for kind without statusSchema
- **WHEN** a GET request is made to `/status` for an object of a kind without `statusSchema`
- **THEN** the response SHALL be `404 Not Found` with `StatusSubresourceNotEnabled` error

### Requirement: PUT /status updates status without optimistic concurrency
The system SHALL provide a `PUT /apis/{group}/{version}/{kind}/{name}/status` endpoint that accepts a JSON body containing only the `status` field. The server SHALL perform a read-modify-write: read the current object, replace only the `status` field, bump `resource_version`, set `updated_at`, and write back. No client-provided version is required or checked. For kinds without `statusSchema`, it SHALL return `StatusSubresourceNotEnabled`.

#### Scenario: Update status for kind with statusSchema
- **WHEN** a PUT request is made to `/status` with a valid status body for a kind with `statusSchema`
- **THEN** the server SHALL validate the status against `statusSchema`, replace only the `status` field, bump `resource_version`, set `updated_at`, and return the full `StoredObject`

#### Scenario: Update status with invalid status
- **WHEN** a PUT request is made to `/status` with a status body that fails `statusSchema` validation
- **THEN** the response SHALL be `422 Unprocessable Entity` with `SchemaValidation` error

#### Scenario: Update status for kind without statusSchema
- **WHEN** a PUT request is made to `/status` for a kind without `statusSchema`
- **THEN** the response SHALL be `404 Not Found` with `StatusSubresourceNotEnabled` error

#### Scenario: Update status for non-existent object
- **WHEN** a PUT request is made to `/status` for an object that does not exist
- **THEN** the response SHALL be `404 Not Found` with `NotFound` error

#### Scenario: Concurrent spec and status updates do not conflict
- **WHEN** a spec update and a status update happen concurrently on the same object
- **THEN** the spec update SHALL use optimistic concurrency (CAS on `resource_version`) and the status update SHALL succeed without version checking

### Requirement: StatusModified event type for watch
The system SHALL define a `StatusModified` variant in `WatchEventType`. When a status update occurs, the system SHALL publish a `WatchEvent` with `event_type: StatusModified` containing the full `StoredObject`.

#### Scenario: Status update publishes StatusModified event
- **WHEN** an object's status is updated via the `/status` endpoint
- **THEN** a `WatchEvent` with `event_type: StatusModified` SHALL be published with the full `StoredObject`

#### Scenario: Spec update publishes Modified event (unchanged)
- **WHEN** an object's spec is updated via the regular PUT endpoint
- **THEN** a `WatchEvent` with `event_type: Modified` SHALL be published (unchanged behavior)