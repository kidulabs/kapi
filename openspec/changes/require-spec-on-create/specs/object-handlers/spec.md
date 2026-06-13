## MODIFIED Requirements

### Requirement: Create handler accepts POST to /apis/{group}/{version}/{kind}
The create handler SHALL extract `group`, `version`, and `kind` from the path and deserialize the request body as `serde_json::Value`. For Schema objects (`kind == "Schema"`), the handler SHALL extract `targetKind` and `targetGroup` from the body, generate the name as `{targetKind}.{targetGroup}`, construct an `ObjectMeta` with that name and any `labels` from `metadata.labels`, and call `ObjectService::create(key, meta, data)`. If `targetKind` or `targetGroup` is missing or not a string, the handler SHALL return `AppError::InvalidSchema`. For non-Schema objects, the handler SHALL extract the object `name` from the body's `metadata.name` field and `labels` from `metadata.labels` (defaulting to empty if absent), extract the `spec` field from the body, construct an `ObjectMeta` with those values, and call `ObjectService::create(key, meta, spec)`. The handler SHALL validate that the request body contains only `metadata` and `spec` as top-level fields; any other field SHALL return `AppError::InvalidRequestBody`. The handler SHALL validate that `spec` is present and is a JSON object; if missing or not an object, the handler SHALL return `AppError::InvalidRequestBody`. The handler SHALL validate that `spec` is non-empty; if `spec` is an empty object `{}`, the handler SHALL return `AppError::InvalidRequestBody`. If `metadata.labels` is present but not an object type, the handler SHALL return an appropriate error response.

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
- **WHEN** a non-Schema object is POSTed to `/apis/example.io/v1/Widget` with `metadata.name` and `spec` containing domain data
- **THEN** the response is 201 Created with the `StoredObject` as JSON
- **AND** the response contains `metadata` with `name` and `system` with `resourceVersion`, `createdAt`, `updatedAt`

#### Scenario: Create object with labels
- **WHEN** a non-Schema object is POSTed with `metadata.labels: {"app": "nginx"}` and `spec` containing domain data
- **THEN** the response is 201 Created with `metadata.labels` containing the provided labels

#### Scenario: Create object without labels
- **WHEN** a non-Schema object is POSTed without `metadata.labels` but with `spec` containing domain data
- **THEN** the response is 201 Created with `metadata.labels` as an empty object

#### Scenario: Create object with invalid labels field type
- **WHEN** a POST request is received with `metadata.labels` as a non-object type (e.g., string or array)
- **THEN** the handler SHALL return an appropriate error response

#### Scenario: Create with missing spec returns 400
- **WHEN** a non-Schema object is POSTed without a `spec` field
- **THEN** the response is 400 Bad Request with `InvalidRequestBody` error

#### Scenario: Create with empty spec returns 400
- **WHEN** a non-Schema object is POSTed with `spec: {}`
- **THEN** the response is 400 Bad Request with `InvalidRequestBody` error

#### Scenario: Create with non-object spec returns 400
- **WHEN** a non-Schema object is POSTed with `spec` as a non-object type (e.g., string, array, number)
- **THEN** the response is 400 Bad Request with `InvalidRequestBody` error

#### Scenario: Create with unknown top-level field returns 400
- **WHEN** a non-Schema object is POSTed with a top-level field other than `metadata` or `spec`
- **THEN** the response is 400 Bad Request with `InvalidRequestBody` error

#### Scenario: Create with invalid data returns 422
- **WHEN** an object is POSTed that fails schema validation
- **THEN** the response is 422 with `SchemaValidation` error details

#### Scenario: Create for unregistered kind returns 404
- **WHEN** an object is POSTed for a kind with no registered Schema
- **THEN** the response is 404 with `NotFound` error
