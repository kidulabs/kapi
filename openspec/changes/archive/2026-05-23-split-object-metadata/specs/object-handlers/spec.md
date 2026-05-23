## MODIFIED Requirements

### Requirement: Create handler accepts POST to /apis/{group}/{version}/{kind}
The create handler SHALL extract `group`, `version`, and `kind` from the path and deserialize the request body as `serde_json::Value`. For Schema objects (`kind == "Schema"`), the handler SHALL extract `targetKind` and `targetGroup` from the body, generate the name as `{targetKind}.{targetGroup}`, construct an `ObjectMeta` with that name, and call `ObjectService::create(key, meta, data)`. If `targetKind` or `targetGroup` is missing or not a string, the handler SHALL return `AppError::InvalidSchema`. For non-Schema objects, the handler SHALL extract the object `name` from the body's `metadata.name` field, construct an `ObjectMeta` with that name, remove the `metadata` key from the body, and call `ObjectService::create(key, meta, data)`.

#### Scenario: Successful Schema create returns 201 with generated name
- **WHEN** a Schema is POSTed to `/apis/kapi.io/v1/Schema` with `targetKind: "Widget"` and `targetGroup: "example.io"`
- **THEN** the response is 201 Created with `metadata.name` set to `"Widget.example.io"`

#### Scenario: Schema create with missing targetKind returns InvalidSchema
- **WHEN** a Schema is POSTed without a `targetKind` field
- **THEN** the response is 422 with `InvalidSchema` error

#### Scenario: Schema create with missing targetGroup returns InvalidSchema
- **WHEN** a Schema is POSTed without a `targetGroup` field
- **THEN** the response is 422 with `InvalidSchema` error

#### Scenario: Successful object create returns 201
- **WHEN** a non-Schema object is POSTed to `/apis/example.io/v1/Widget` with `metadata.name`
- **THEN** the response is 201 Created with the `StoredObject` as JSON
- **AND** the response contains `metadata` with `name` and `system` with `resourceVersion`, `createdAt`, `updatedAt`

#### Scenario: Create with invalid data returns 422
- **WHEN** an object is POSTed that fails schema validation
- **THEN** the response is 422 with `SchemaValidation` error details

#### Scenario: Create for unregistered kind returns 404
- **WHEN** an object is POSTed for a kind with no registered Schema
- **THEN** the response is 404 with `NotFound` error

### Requirement: Update handler accepts PUT to /apis/{group}/{version}/{kind}/{name}
The update handler SHALL extract path parameters, deserialize the body as `StoredObject`, validate that the URL `key` and `name` match the object's `key` and `metadata.name`, and call `ObjectService::update(object)`. The handler SHALL NOT modify `system` fields; the `resourceVersion` in `system` is used by the store for optimistic concurrency control.

#### Scenario: Successful update returns 200
- **WHEN** an object is PUT with a matching `system.resourceVersion`
- **THEN** the response is 200 OK with the updated `StoredObject` (new `system.resourceVersion`)

#### Scenario: Update with wrong version returns 409
- **WHEN** an object is PUT with a stale `system.resourceVersion`
- **THEN** the response is 409 with `Conflict` error

#### Scenario: Update with mismatched name returns 400
- **WHEN** the URL name does not match the object's `metadata.name`
- **THEN** the response is 400 Bad Request