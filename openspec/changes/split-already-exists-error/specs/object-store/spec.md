## MODIFIED Requirements

### Requirement: create stores a new object and assigns a version
The `create` method SHALL store a new object with the given `ResourceKey`, name, and data. It SHALL assign a globally monotonic `resource_version` starting from 1, set `created_at` and `updated_at` to the current UTC time, and return the resulting `StoredObject`. If an object with the same key and name already exists, it SHALL return `AppError::AlreadyExists`.

#### Scenario: Successful create returns stored object with version 1
- **WHEN** `create` is called for a key/name pair that does not exist
- **THEN** the returned `StoredObject` has `resource_version` >= 1, `created_at` set, and `updated_at` equal to `created_at`

#### Scenario: Duplicate create returns AlreadyExists
- **WHEN** `create` is called for a key/name pair that already exists
- **THEN** the error is `AppError::AlreadyExists` with the resource kind and name populated
