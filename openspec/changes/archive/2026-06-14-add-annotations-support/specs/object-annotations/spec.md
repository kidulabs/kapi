## ADDED Requirements

### Requirement: Annotations field on ObjectMeta
Objects SHALL carry an `annotations` field of type `HashMap<String, String>` in their metadata. The field SHALL always be present in serialized form, with an empty map `{}` when no annotations are attached.

#### Scenario: Object created without annotations
- **WHEN** a client creates an object without providing `metadata.annotations`
- **THEN** the stored object SHALL have `metadata.annotations` as an empty map `{}`

#### Scenario: Object created with annotations
- **WHEN** a client creates an object with `metadata.annotations: {"description": "my object", "owner": "team-platform"}`
- **THEN** the stored object SHALL have `metadata.annotations` containing exactly those key-value pairs

#### Scenario: Annotations serialized in API response
- **WHEN** an object is returned in any API response (create, get, list, update)
- **THEN** the `metadata.annotations` field SHALL always be present, even if empty

### Requirement: Annotation key validation
Annotation keys SHALL be validated with minimal rules. Keys MUST be non-empty and at most 256 characters. No character restrictions SHALL apply beyond length limits.

#### Scenario: Valid simple key
- **WHEN** an annotation key is `description` or `owner` or `build-url`
- **THEN** validation SHALL pass

#### Scenario: Valid key with special characters
- **WHEN** an annotation key is `kapi.io/last-applied-config` or `example.com/path@v1`
- **THEN** validation SHALL pass

#### Scenario: Empty key
- **WHEN** an annotation key is an empty string
- **THEN** validation SHALL fail with an `InvalidAnnotation` error

#### Scenario: Key exceeds length limit
- **WHEN** an annotation key exceeds 256 characters
- **THEN** validation SHALL fail with an `InvalidAnnotation` error

### Requirement: Annotation value validation
Annotation values SHALL accept any string, including empty strings. No character restrictions or length limits SHALL apply to individual values beyond the total size limit.

#### Scenario: Valid value with special characters
- **WHEN** an annotation value is `{"key": "value", "nested": {"data": true}}`
- **THEN** validation SHALL pass

#### Scenario: Valid empty value
- **WHEN** an annotation value is an empty string
- **THEN** validation SHALL pass

#### Scenario: Valid value with URLs
- **WHEN** an annotation value is `https://example.com/path?query=value&other=123`
- **THEN** validation SHALL pass

### Requirement: Total annotation size limit
The total serialized size of all annotations for a single object SHALL not exceed 256KB.

#### Scenario: Annotations within size limit
- **WHEN** the total serialized size of annotations is less than 256KB
- **THEN** validation SHALL pass

#### Scenario: Annotations exceed size limit
- **WHEN** the total serialized size of annotations exceeds 256KB
- **THEN** validation SHALL fail with an `InvalidAnnotation` error

### Requirement: Annotations on Schema objects
Schema objects SHALL support annotations in the same way as regular objects. Annotation extraction and validation SHALL apply to Schema registrations.

#### Scenario: Schema created with annotations
- **WHEN** a client creates a Schema with `metadata.annotations: {"team": "platform", "docs": "https://..."}`
- **THEN** the stored Schema object SHALL have those annotations in its metadata

#### Scenario: Schema created without annotations
- **WHEN** a client creates a Schema without providing `metadata.annotations`
- **THEN** the stored Schema object SHALL have `metadata.annotations` as an empty map `{}`

### Requirement: Annotation persistence in SQLite
Annotations SHALL be stored as a JSON-serialized `HashMap<String, String>` in the `annotations TEXT` column of the `objects` table.

#### Scenario: Annotations persisted on create
- **WHEN** an object with annotations is created in SQLiteStore
- **THEN** the annotations SHALL be serialized as JSON and stored in the `annotations` column

#### Scenario: Annotations reconstructed on read
- **WHEN** an object is read from SQLiteStore
- **THEN** its annotations SHALL be deserialized from the `annotations` column into the `ObjectMeta.annotations` field

#### Scenario: Null annotations column
- **WHEN** an object is read from SQLiteStore with `annotations` column set to `NULL`
- **THEN** the `ObjectMeta.annotations` field SHALL be an empty `HashMap`

### Requirement: Annotation persistence in InMemoryStore
Annotations SHALL be stored as part of `ObjectMeta` within the `StoredObject` in InMemoryStore. No separate storage mechanism is needed.

#### Scenario: Annotations persisted on create
- **WHEN** an object with annotations is created in InMemoryStore
- **THEN** the annotations SHALL be stored as part of the `ObjectMeta` in the stored `StoredObject`

#### Scenario: Annotations updated on update
- **WHEN** an object is updated with new annotations in InMemoryStore
- **THEN** the `ObjectMeta.annotations` field SHALL be replaced with the new annotations

### Requirement: InvalidAnnotation error variant
The system SHALL define an `InvalidAnnotation(String)` variant in `AppError` that returns HTTP 400 Bad Request with the error message.

#### Scenario: InvalidAnnotation returns 400
- **WHEN** an `AppError::InvalidAnnotation(msg)` is returned from a handler
- **THEN** the HTTP response SHALL be 400 Bad Request with the error message

### Requirement: Annotation extraction in handlers
The handler layer SHALL extract annotations from `metadata.annotations` in the request body, returning an empty `HashMap` when absent and an error when the field is not an object with string values.

#### Scenario: Extract annotations from request
- **WHEN** a request body contains `metadata.annotations: {"key": "value"}`
- **THEN** the handler SHALL extract a `HashMap` containing `{"key": "value"}`

#### Scenario: Extract empty annotations
- **WHEN** a request body does not contain `metadata.annotations`
- **THEN** the handler SHALL extract an empty `HashMap`

#### Scenario: Invalid annotations format
- **WHEN** a request body contains `metadata.annotations: "not-an-object"`
- **THEN** the handler SHALL return an `InvalidAnnotation` error

#### Scenario: Non-string annotation value
- **WHEN** a request body contains `metadata.annotations: {"key": 123}`
- **THEN** the handler SHALL return an `InvalidAnnotation` error
