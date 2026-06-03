## MODIFIED Requirements

### Requirement: ObjectStore trait defines the storage contract
The system SHALL define an `ObjectStore` async trait with methods `create`, `get`, `list`, `update`, `delete`, and `exists` that operate on `StoredObject` instances. The trait SHALL require `Send + Sync`. The `create` method SHALL accept `ObjectMeta` for the metadata parameter (which includes `name` and `labels`) and `serde_json::Value` for the `spec` parameter. The `update` method SHALL accept a full `StoredObject` and perform optimistic concurrency control by comparing the embedded `object.system.resource_version` against the stored version. The `delete` method SHALL accept only `key` and `name` parameters and perform unconditional removal. The `exists` method SHALL accept a `ResourceKey` and return `Result<bool, AppError>` indicating whether any objects exist for that key.

#### Scenario: Trait is object-safe and thread-safe
- **WHEN** a type implements `ObjectStore`
- **THEN** it can be used as `dyn ObjectStore` inside `Arc` and sent across threads

#### Scenario: create accepts ObjectMeta and raw JSON value
- **WHEN** a caller invokes `create(key, meta, spec)` with an `ObjectMeta` containing `name` and `labels` and a `serde_json::Value`
- **THEN** the implementation wraps the value into `SpecData` internally and uses `meta.name` for the object name and `meta.labels` for labels, without the caller needing to know about `SpecData`

#### Scenario: update accepts full StoredObject
- **WHEN** a caller invokes `update(object)` with a `StoredObject`
- **THEN** the implementation uses `object.system.resource_version` for optimistic concurrency control and preserves `object.metadata.labels`

#### Scenario: delete takes only key and name
- **WHEN** a caller invokes `delete(key, name)`
- **THEN** the implementation removes the object unconditionally without any version check

#### Scenario: exists checks for object presence
- **WHEN** a caller invokes `exists(key)` with a `ResourceKey`
- **THEN** the implementation returns `Ok(true)` if any objects exist for that key, `Ok(false)` otherwise

### Requirement: create stores a new object and assigns a version
The `create` method SHALL store a new object with the given `ResourceKey`, `ObjectMeta.name`, `ObjectMeta.labels`, and spec. It SHALL assign a globally monotonic `resource_version` starting from 1, set `created_at` and `updated_at` to the current UTC time, and return the resulting `StoredObject` with `metadata` populated from the `ObjectMeta` argument and `system` populated with the server-generated fields. If an object with the same key and name already exists, it SHALL return `AppError::AlreadyExists`.

#### Scenario: Successful create returns stored object with version 1
- **WHEN** `create` is called for a key/name pair that does not exist
- **THEN** the returned `StoredObject` has `system.resource_version` >= 1, `system.created_at` set, and `system.updated_at` equal to `system.created_at`
- **AND** `metadata.name` matches the `ObjectMeta.name` provided
- **AND** `metadata.labels` matches the `ObjectMeta.labels` provided

#### Scenario: Duplicate create returns AlreadyExists
- **WHEN** `create` is called for a key/name pair that already exists
- **THEN** the error is `AppError::AlreadyExists` with the resource kind and name populated

### Requirement: update modifies an existing object with optimistic concurrency
The `update` method SHALL accept a `StoredObject` and replace the spec and `metadata` (including `labels`) of the existing object identified by `object.metadata.name` and the object's key. It SHALL compare the stored object's `system.resource_version` against `object.system.resource_version` and return `AppError::Conflict` if they do not match. On a successful update, it SHALL increment `resource_version` via the global counter, set `updated_at` to the current UTC time, and return the updated `StoredObject`. If the object does not exist, it SHALL return `AppError::NotFound`.

#### Scenario: Successful update increments version
- **WHEN** `update` is called with a `StoredObject` whose `system.resource_version` matches the stored version
- **THEN** the returned `StoredObject` has a higher `system.resource_version` and updated `system.updated_at`
- **AND** `metadata.labels` reflects the updated labels

#### Scenario: Update with wrong version returns conflict
- **WHEN** `update` is called with a `StoredObject` whose `system.resource_version` does not match the stored version
- **THEN** the error is `AppError::Conflict` with `expected` and `actual` fields