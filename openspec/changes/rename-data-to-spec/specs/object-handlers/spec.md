## MODIFIED Requirements

### Requirement: Handler extracts spec from request body
The `create` handler SHALL extract `metadata` (name and labels) from the request body, remove it from the JSON, and pass the remaining JSON as the `spec` parameter to `ObjectService::create`. The `update` handler SHALL deserialize the full `StoredObject` from the request body, which includes the `spec` field.

#### Scenario: Create handler extracts metadata and passes spec
- **WHEN** a POST request arrives with body `{ "metadata": { "name": "foo" }, "color": "blue" }`
- **THEN** the handler extracts `metadata.name = "foo"`, removes `metadata` from the body, and passes `{ "color": "blue" }` as the spec value to `ObjectService::create`

#### Scenario: Update handler deserializes full StoredObject
- **WHEN** a PUT request arrives with a full `StoredObject` JSON body
- **THEN** the handler deserializes it into a `StoredObject` struct with the `spec` field populated