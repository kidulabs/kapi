## MODIFIED Requirements

### Requirement: ObjectStore trait defines the storage contract
The system SHALL define an `ObjectStore` async trait with methods `create`, `get`, `list`, `update`, and `delete` that operate on `StoredObject` instances. The trait SHALL require `Send + Sync`. The `create` method SHALL accept `serde_json::Value` for the data parameter. The `update` method SHALL accept a full `StoredObject` and perform optimistic concurrency control by comparing the embedded `object.metadata.resource_version` against the stored version. The `delete` method SHALL accept only `key` and `name` parameters and perform unconditional removal.

#### Scenario: Trait is object-safe and thread-safe
- **WHEN** a type implements `ObjectStore`
- **THEN** it can be used as `dyn ObjectStore` inside `Arc` and sent across threads

#### Scenario: create accepts raw JSON value
- **WHEN** a caller invokes `create(key, name, data)` with a `serde_json::Value`
- **THEN** the implementation wraps the value into `UserData` internally without the caller needing to know about `UserData`

#### Scenario: update accepts full StoredObject
- **WHEN** a caller invokes `update(object)` with a `StoredObject`
- **THEN** the implementation uses `object.metadata.resource_version` for optimistic concurrency control

#### Scenario: delete takes only key and name
- **WHEN** a caller invokes `delete(key, name)`
- **THEN** the implementation removes the object unconditionally without any version check

### Requirement: update modifies an existing object with optimistic concurrency
The `update` method SHALL accept a `StoredObject` and replace the data of the existing object identified by `object.metadata.name` and the object's key. It SHALL compare the stored object's `metadata.resource_version` against `object.metadata.resource_version` and return `AppError::Conflict` if they do not match. On a successful update, it SHALL increment `resource_version` via the global counter, set `updated_at` to the current UTC time, and return the updated `StoredObject`. If the object does not exist, it SHALL return `AppError::NotFound`.

#### Scenario: Successful update increments version
- **WHEN** `update` is called with a `StoredObject` whose `metadata.resource_version` matches the stored version
- **THEN** the returned `StoredObject` has a higher `metadata.resource_version` and updated `metadata.updated_at`

#### Scenario: Update with wrong version returns conflict
- **WHEN** `update` is called with a `StoredObject` whose `metadata.resource_version` does not match the stored version
- **THEN** the error is `AppError::Conflict` with `expected` and `actual` fields

#### Scenario: Update for missing object returns NotFound
- **WHEN** `update` is called for a key/name pair that does not exist
- **THEN** the error is `AppError::NotFound`

### Requirement: delete removes an object unconditionally
The `delete` method SHALL remove the object for the given `ResourceKey` and name and return the deleted `StoredObject`. It SHALL NOT perform any version check. If the object does not exist, it SHALL return `AppError::NotFound`.

#### Scenario: Successful delete returns the deleted object
- **WHEN** `delete` is called for an existing object
- **THEN** the object is removed and the returned `StoredObject` matches the previously stored data

#### Scenario: Delete for missing object returns NotFound
- **WHEN** `delete` is called for a key/name pair that does not exist
- **THEN** the error is `AppError::NotFound`

#### Scenario: Delete is unconditional regardless of version
- **WHEN** `delete` is called for an existing object
- **THEN** the object is removed regardless of its current `resource_version`
