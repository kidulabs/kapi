## MODIFIED Requirements

### Requirement: ObjectStore trait defines the storage contract
The system SHALL define an `ObjectStore` async trait with methods `create`, `get`, `list`, `update`, `delete`, and `exists` that operate on `StoredObject` instances. The trait SHALL require `Send + Sync`. The `create` method SHALL accept `ObjectMeta` for the metadata parameter (which includes `name` and `labels`) and `serde_json::Value` for the data parameter. The `update` method SHALL accept a full `StoredObject` and perform optimistic concurrency control by comparing the embedded `object.system.resource_version` against the stored version. The `delete` method SHALL accept only `key` and `name` parameters and perform unconditional removal. The `exists` method SHALL accept a `ResourceKey` and return `Result<bool, AppError>` indicating whether any objects exist for that key.

#### Scenario: Trait is object-safe and thread-safe
- **WHEN** a type implements `ObjectStore`
- **THEN** it can be used as `dyn ObjectStore` inside `Arc` and sent across threads

#### Scenario: create accepts ObjectMeta and raw JSON value
- **WHEN** a caller invokes `create(key, meta, data)` with an `ObjectMeta` containing `name` and `labels` and a `serde_json::Value`
- **THEN** the implementation wraps the value into `UserData` internally and uses `meta.name` for the object name and `meta.labels` for labels, without the caller needing to know about `UserData`

#### Scenario: update accepts full StoredObject
- **WHEN** a caller invokes `update(object)` with a `StoredObject`
- **THEN** the implementation uses `object.system.resource_version` for optimistic concurrency control and preserves `object.metadata.labels`

#### Scenario: delete takes only key and name
- **WHEN** a caller invokes `delete(key, name)`
- **THEN** the implementation removes the object unconditionally without any version check

#### Scenario: exists checks for object presence
- **WHEN** a caller invokes `exists(key)` with a `ResourceKey`
- **THEN** the implementation returns `Ok(true)` if any objects exist for that key, `Ok(false)` otherwise
