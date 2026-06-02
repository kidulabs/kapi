## ADDED Requirements

### Requirement: ObjectStore update_status method
The `ObjectStore` trait SHALL define an `update_status` method that accepts `key: &ResourceKey`, `name: &str`, and `status: serde_json::Value`, and returns `Result<StoredObject, AppError>`. The method SHALL perform a server-side read-modify-write: read the current object, replace only the `status` field, bump `resource_version`, set `updated_at` to the current time, and write back. It SHALL NOT perform optimistic concurrency checking (no CAS on `resource_version`). If the object does not exist, it SHALL return `AppError::NotFound`.

#### Scenario: Update status succeeds
- **WHEN** `update_status(key, name, status_value)` is called for an existing object
- **THEN** the object's `status` field is replaced with `Some(SpecData { value: status_value })`
- **AND** `system.resource_version` is incremented
- **AND** `system.updated_at` is set to the current time
- **AND** the full `StoredObject` is returned

#### Scenario: Update status for non-existent object
- **WHEN** `update_status(key, name, status_value)` is called for an object that does not exist
- **THEN** the error is `AppError::NotFound`

#### Scenario: Update status does not modify spec
- **WHEN** `update_status(key, name, status_value)` is called
- **THEN** the object's `spec` field SHALL remain unchanged
- **AND** the object's `metadata.labels` SHALL remain unchanged

#### Scenario: Update status bumps resource_version
- **WHEN** `update_status(key, name, status_value)` is called on an object with `resource_version: 5`
- **THEN** the returned `StoredObject` SHALL have `resource_version: 6`

### Requirement: InMemoryStore implements update_status
`InMemoryStore` SHALL implement `update_status` by acquiring a write lock on the DashMap entry, replacing the `status` field, incrementing `resource_version`, and setting `updated_at`.

#### Scenario: InMemoryStore update_status replaces status
- **WHEN** `update_status` is called on an InMemoryStore
- **THEN** the stored object's `status` is replaced and `resource_version` is incremented

### Requirement: SQLiteStore implements update_status
`SQLiteStore` SHALL implement `update_status` by executing an UPDATE statement that sets `status = ?1`, `resource_version = ?2`, `updated_at = ?3` WHERE `resource_group = ?4 AND api_version = ?5 AND resource_kind = ?6 AND name = ?7`. No `AND resource_version = ?8` clause is used (no CAS).

#### Scenario: SQLiteStore update_status replaces status
- **WHEN** `update_status` is called on a SQLiteStore
- **THEN** the `status` column is updated, `resource_version` is incremented, and `updated_at` is set

### Requirement: SQLite objects table has nullable status column
The `objects` table in SQLite SHALL include a `status TEXT` column that is nullable. Existing rows SHALL have `status = NULL`. The `init_schema` method SHALL create this column.

#### Scenario: New SQLite database includes status column
- **WHEN** a new SQLiteStore is created
- **THEN** the `objects` table SHALL have a `status TEXT` column

#### Scenario: Existing rows have null status
- **WHEN** an object is created without status
- **THEN** the `status` column SHALL be `NULL`