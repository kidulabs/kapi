## MODIFIED Requirements

### Requirement: Create handler extracts labels from metadata
The create handler SHALL extract `labels` from `metadata.labels` in the request body, alongside `name` from `metadata.name`. Labels SHALL be extracted for both regular objects and Schema objects. The entire `metadata` field SHALL be stripped from the body before passing to the service.

#### Scenario: Create regular object with labels
- **WHEN** a POST request is received with body `{"metadata": {"name": "foo", "labels": {"app": "nginx"}}, "color": "blue"}`
- **THEN** the handler SHALL construct `ObjectMeta { name: "foo", labels: {"app": "nginx"} }` and pass body `{"color": "blue"}` to the service

#### Scenario: Create regular object without labels
- **WHEN** a POST request is received with body `{"metadata": {"name": "foo"}, "color": "blue"}`
- **THEN** the handler SHALL construct `ObjectMeta { name: "foo", labels: {} }` and pass body `{"color": "blue"}` to the service

#### Scenario: Create Schema object with labels
- **WHEN** a POST request for a Schema is received with body containing `metadata.labels`
- **THEN** the handler SHALL extract labels and include them in the `ObjectMeta` for the Schema

#### Scenario: Create object with invalid labels field type
- **WHEN** a POST request is received with `metadata.labels` as a non-object type (e.g., string or array)
- **THEN** the handler SHALL return an appropriate error response

### Requirement: Update handler preserves labels
The update handler SHALL accept labels as part of the `StoredObject` body's `metadata` field. Labels SHALL be passed through to the service for validation and persistence.

#### Scenario: Update object with changed labels
- **WHEN** a PUT request is received with a `StoredObject` body containing updated `metadata.labels`
- **THEN** the handler SHALL pass the full `StoredObject` (including new labels) to the service

#### Scenario: Update object removing all labels
- **WHEN** a PUT request is received with `metadata.labels: {}`
- **THEN** the handler SHALL pass the empty labels map to the service, which SHALL remove all existing labels
