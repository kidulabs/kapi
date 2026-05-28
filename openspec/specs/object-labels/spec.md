# object-labels Specification

## Purpose

Define the label system for objects: the `labels` field on `ObjectMeta`, label validation rules (key format, value format, length limits), label persistence in both InMemoryStore and SQLiteStore, and diff-based label updates. Labels follow Kubernetes-style semantics with optional prefix support.
## Requirements
### Requirement: Labels field on ObjectMeta
Objects SHALL carry a `labels` field of type `HashMap<String, String>` in their metadata. The field SHALL always be present in serialized form, with an empty map `{}` when no labels are attached.

#### Scenario: Object created without labels
- **WHEN** a client creates an object without providing `metadata.labels`
- **THEN** the stored object SHALL have `metadata.labels` as an empty map `{}`

#### Scenario: Object created with labels
- **WHEN** a client creates an object with `metadata.labels: {"app": "nginx", "env": "prod"}`
- **THEN** the stored object SHALL have `metadata.labels` containing exactly those key-value pairs

#### Scenario: Labels serialized in API response
- **WHEN** an object is returned in any API response (create, get, list, update)
- **THEN** the `metadata.labels` field SHALL always be present, even if empty

### Requirement: Label key validation
Label keys SHALL be validated according to structured label semantics. Keys MUST be non-empty, at most 256 characters, and match the pattern `[a-zA-Z0-9][-_.a-zA-Z0-9]*` with an optional `/` separator for prefix (`prefix/name` format). Prefixes MUST be at most 253 characters and follow DNS subdomain format.

#### Scenario: Valid simple key
- **WHEN** a label key is `app` or `my-label` or `label_name.v2`
- **THEN** validation SHALL pass

#### Scenario: Valid prefixed key
- **WHEN** a label key is `app.example.io/name` or `example.com/tier`
- **THEN** validation SHALL pass

#### Scenario: Empty key
- **WHEN** a label key is an empty string
- **THEN** validation SHALL fail with an `InvalidLabel` error

#### Scenario: Key exceeds length limit
- **WHEN** a label key exceeds 256 characters
- **THEN** validation SHALL fail with an `InvalidLabel` error

#### Scenario: Key with invalid characters
- **WHEN** a label key contains characters outside `[a-zA-Z0-9-_.]` (or `/` for prefix separator)
- **THEN** validation SHALL fail with an `InvalidLabel` error

### Requirement: Label value validation
Label values SHALL be validated according to structured label semantics. Values MUST be at most 256 characters and match the pattern `[a-zA-Z0-9][-_.a-zA-Z0-9]*` or be an empty string.

#### Scenario: Valid value
- **WHEN** a label value is `nginx`, `v1.2.3`, `prod-env`, or empty string
- **THEN** validation SHALL pass

#### Scenario: Value exceeds length limit
- **WHEN** a label value exceeds 256 characters
- **THEN** validation SHALL fail with an `InvalidLabel` error

#### Scenario: Value with invalid characters
- **WHEN** a label value contains characters outside `[a-zA-Z0-9-_.]` and is not empty
- **THEN** validation SHALL fail with an `InvalidLabel` error

### Requirement: Labels on Schema objects
Schema objects SHALL support labels in the same way as regular objects. Label extraction and validation SHALL apply to Schema registrations.

#### Scenario: Schema created with labels
- **WHEN** a client creates a Schema with `metadata.labels: {"team": "platform"}`
- **THEN** the stored Schema object SHALL have those labels in its metadata

#### Scenario: Schema created without labels
- **WHEN** a client creates a Schema without providing `metadata.labels`
- **THEN** the stored Schema object SHALL have `metadata.labels` as an empty map `{}`

### Requirement: Label persistence in SQLite
Labels SHALL be stored in a separate `labels` table in SQLite with composite primary key `(resource_group, api_version, resource_kind, name, label_key)` and foreign key to `objects` with `ON DELETE CASCADE`.

#### Scenario: Labels persisted on create
- **WHEN** an object with labels is created in SQLiteStore
- **THEN** each label key-value pair SHALL be inserted as a row in the `labels` table

#### Scenario: Labels deleted with object
- **WHEN** an object is deleted from SQLiteStore
- **THEN** all associated label rows SHALL be automatically deleted via `ON DELETE CASCADE`

#### Scenario: Labels reconstructed on read
- **WHEN** an object is read from SQLiteStore
- **THEN** its labels SHALL be reconstructed from the `labels` table into the `ObjectMeta.labels` field

### Requirement: Diff-based label updates
When an object is updated, label changes SHALL be computed as a diff (existing vs new) and applied as targeted deletes and upserts within the same transaction as the object update.

#### Scenario: Label added on update
- **WHEN** an object is updated with a new label key not present in the existing labels
- **THEN** only the new label SHALL be inserted into the `labels` table

#### Scenario: Label value changed on update
- **WHEN** an object is updated with a different value for an existing label key
- **THEN** only that label's value SHALL be updated in the `labels` table

#### Scenario: Label removed on update
- **WHEN** an object is updated without a label key that was present in the existing labels
- **THEN** only that label row SHALL be deleted from the `labels` table

#### Scenario: Unchanged labels not rewritten
- **WHEN** an object is updated with the same labels as before
- **THEN** no label table writes SHALL occur

#### Scenario: Label update atomicity
- **WHEN** an object update with label changes is performed
- **THEN** the object update and all label changes SHALL be applied in a single database transaction

### Requirement: Label persistence in InMemoryStore
Labels SHALL be stored as part of `ObjectMeta` within the `StoredObject` in InMemoryStore. No separate storage mechanism is needed.

#### Scenario: Labels persisted on create
- **WHEN** an object with labels is created in InMemoryStore
- **THEN** the labels SHALL be stored as part of the `ObjectMeta` in the stored `StoredObject`

#### Scenario: Labels updated on update
- **WHEN** an object is updated with new labels in InMemoryStore
- **THEN** the `ObjectMeta.labels` field SHALL be replaced with the new labels

