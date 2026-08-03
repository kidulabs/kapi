## MODIFIED Requirements

### Requirement: Client provides low-level HTTP methods for all endpoints
The kapi-client library SHALL provide explicit methods for each kapi-server endpoint: `list`, `get`, `create`, `update`, `delete`, `get_status`, `update_status`, and `watch`. Each method SHALL map directly to one HTTP endpoint.

#### Scenario: List objects
- **WHEN** user calls `client.list(key, namespace, opts)`
- **THEN** client sends GET request to `/apis/{group}/{version}/{kind}` (or namespace-scoped route) and returns `ListResponse`

#### Scenario: Get single object
- **WHEN** user calls `client.get(key, namespace, name)`
- **THEN** client sends GET request to `/apis/{group}/{version}/{kind}/{name}` (or namespace-scoped route) and returns `StoredObject`

#### Scenario: Create object
- **WHEN** user calls `client.create(key, namespace, meta, spec)`
- **THEN** client sends POST request with JSON body and returns created `StoredObject`

#### Scenario: Update object
- **WHEN** user calls `client.update(namespace, obj)`
- **THEN** client sends PUT request with full `StoredObject` JSON and returns updated `StoredObject`

#### Scenario: Delete object
- **WHEN** user calls `client.delete(key, namespace, name)`
- **THEN** client sends DELETE request and returns deleted `StoredObject`

#### Scenario: Get status subresource
- **WHEN** user calls `client.get_status(key, namespace, name)`
- **THEN** client sends GET request to `/status` endpoint and returns `Option<Value>`

#### Scenario: Update status subresource
- **WHEN** user calls `client.update_status(key, namespace, name, status)`
- **THEN** client sends PUT request to `/status` endpoint and returns updated `StoredObject`

#### Scenario: Watch objects
- **WHEN** user calls `client.watch(key, filter)`
- **THEN** client sends GET request with `?watch=true` and returns async stream of `WatchEvent`

#### Scenario: ApiError includes structured details
- **WHEN** the server returns a non-success status with a structured error body containing a `details` field
- **THEN** the `ClientError::ApiError` variant SHALL include the `details` field as a `serde_json::Value`, preserving the server's structured error context for downstream consumers
