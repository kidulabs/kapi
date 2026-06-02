## ADDED Requirements

### Requirement: GET /status handler
The system SHALL provide a `get_status` handler for `GET /apis/{group}/{version}/{kind}/{name}/status` that extracts path parameters, calls `ObjectService::get_status`, and returns the status value as JSON. If the kind does not have a `statusSchema`, the handler SHALL return `404 Not Found` with `StatusSubresourceNotEnabled`.

#### Scenario: Get status for kind with statusSchema
- **WHEN** a GET request is made to `/status` for a kind with `statusSchema`
- **THEN** the handler returns `200 OK` with the status value as JSON

#### Scenario: Get status for kind without statusSchema
- **WHEN** a GET request is made to `/status` for a kind without `statusSchema`
- **THEN** the handler returns `404 Not Found` with `StatusSubresourceNotEnabled` error

### Requirement: PUT /status handler
The system SHALL provide an `update_status` handler for `PUT /apis/{group}/{version}/{kind}/{name}/status` that extracts path parameters, deserializes the request body as JSON, extracts the `status` field, and calls `ObjectService::update_status`. The handler SHALL return `200 OK` with the full `StoredObject` on success. If the kind does not have a `statusSchema`, the handler SHALL return `404 Not Found` with `StatusSubresourceNotEnabled`.

#### Scenario: Update status for kind with statusSchema
- **WHEN** a PUT request is made to `/status` with a valid status body for a kind with `statusSchema`
- **THEN** the handler returns `200 OK` with the full `StoredObject`

#### Scenario: Update status for kind without statusSchema
- **WHEN** a PUT request is made to `/status` for a kind without `statusSchema`
- **THEN** the handler returns `404 Not Found` with `StatusSubresourceNotEnabled` error

## MODIFIED Requirements

### Requirement: Status route registration
The router SHALL include routes for the status subresource:
- `GET /apis/{group}/{version}/{kind}/{name}/status` → `handler::get_status`
- `PUT /apis/{group}/{version}/{kind}/{name}/status` → `handler::update_status`

#### Scenario: Status routes are registered
- **WHEN** the router is built
- **THEN** routes for `/status` are available alongside existing CRUD routes