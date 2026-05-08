## Purpose

Define the `ObjectStore` trait and its `InMemoryStore` implementation for persisting, retrieving, listing, updating, and deleting `StoredObject` instances identified by `ResourceKey` and name.

## Requirements

### Requirement: ObjectStore trait defines the storage contract
The system SHALL define an `ObjectStore` async trait with methods `create`, `get`, `list`, `update`, and `delete` that operate on `StoredObject` instances identified by `ResourceKey` and name. The trait SHALL require `Send + Sync`. The trait methods SHALL accept `serde_json::Value` for data parameters, not `UserData`.

#### Scenario: Trait is object-safe and thread-safe
- **WHEN** a type implements `ObjectStore`
- **THEN** it can be used as `dyn ObjectStore` inside `Arc` and sent across threads

#### Scenario: create accepts raw JSON value
- **WHEN** a caller invokes `create(key, name, data)` with a `serde_json::Value`
- **THEN** the implementation wraps the value into `UserData` internally without the caller needing to know about `UserData`

### Requirement: create stores a new object and assigns a version
The `create` method SHALL store a new object with the given `ResourceKey`, name, and data. It SHALL assign a globally monotonic `resource_version` starting from 1, set `created_at` and `updated_at` to the current UTC time, and return the resulting `StoredObject`. If an object with the same key and name already exists, it SHALL return `AppError::Conflict`.

#### Scenario: Successful create returns stored object with version 1
- **WHEN** `create` is called for a key/name pair that does not exist
- **THEN** the returned `StoredObject` has `resource_version` >= 1, `created_at` set, and `updated_at` equal to `created_at`

#### Scenario: Duplicate create returns conflict
- **WHEN** `create` is called for a key/name pair that already exists
- **THEN** the error is `AppError::Conflict`

### Requirement: get retrieves an existing object
The `get` method SHALL return the `StoredObject` for the given `ResourceKey` and name. If no such object exists, it SHALL return `AppError::NotFound`.

#### Scenario: Successful get returns the stored object
- **WHEN** `get` is called for a key/name pair that exists
- **THEN** the returned `StoredObject` matches the stored data

#### Scenario: Get for missing object returns NotFound
- **WHEN** `get` is called for a key/name pair that does not exist
- **THEN** the error is `AppError::NotFound` with `what` and `identifier` fields populated

### Requirement: list returns paginated objects for a resource kind
The `list` method SHALL return all `StoredObject` instances matching the given `ResourceKey`, sorted by name in ascending order. When `ListOptions.limit` is `Some(n)`, it SHALL return at most `n` items. When `ListOptions.continue_token` is `Some(token)`, it SHALL skip entries up to and including the name encoded in the token. The returned `ListResponse` SHALL include a `continue_token` if more items remain beyond the returned batch.

#### Scenario: List returns all objects sorted by name
- **WHEN** `list` is called with no limit or continue token
- **THEN** all objects for the key are returned in ascending name order

#### Scenario: List with limit returns partial results with continue token
- **WHEN** `list` is called with `limit = Some(2)` and 5 objects exist
- **THEN** exactly 2 items are returned and `continue_token` is `Some`

#### Scenario: List with continue token resumes from correct position
- **WHEN** `list` is called with a continue token encoding name "b"
- **THEN** objects with names <= "b" are skipped and results start from the next name

#### Scenario: List with no matching objects returns empty list
- **WHEN** `list` is called for a key with no stored objects
- **THEN** the response has an empty `items` vector and `continue_token` is `None`

### Requirement: update modifies an existing object with optimistic concurrency
The `update` method SHALL replace the data of an existing object, increment its `resource_version` via the global counter, set `updated_at` to the current UTC time, and return the updated `StoredObject`. It SHALL compare the stored object's `resource_version` against `expected_version` and return `AppError::Conflict` if they do not match. If the object does not exist, it SHALL return `AppError::NotFound`.

#### Scenario: Successful update increments version
- **WHEN** `update` is called with the correct `expected_version`
- **THEN** the returned `StoredObject` has a higher `resource_version` and updated `updated_at`

#### Scenario: Update with wrong version returns conflict
- **WHEN** `update` is called with an `expected_version` that does not match the stored version
- **THEN** the error is `AppError::Conflict` with `expected` and `actual` fields

#### Scenario: Update for missing object returns NotFound
- **WHEN** `update` is called for a key/name pair that does not exist
- **THEN** the error is `AppError::NotFound`

### Requirement: delete removes an object with optional version check
The `delete` method SHALL remove the object for the given `ResourceKey` and name and return the deleted `StoredObject`. If `expected_version` is `Some(n)`, it SHALL verify the stored version matches `n` before deleting, returning `AppError::Conflict` on mismatch. If the object does not exist, it SHALL return `AppError::NotFound`.

#### Scenario: Successful delete returns the deleted object
- **WHEN** `delete` is called for an existing object
- **THEN** the object is removed and the returned `StoredObject` matches the previously stored data

#### Scenario: Delete with matching version succeeds
- **WHEN** `delete` is called with `expected_version = Some(n)` matching the stored version
- **THEN** the object is deleted successfully

#### Scenario: Delete with mismatched version returns conflict
- **WHEN** `delete` is called with `expected_version = Some(n)` that does not match the stored version
- **THEN** the error is `AppError::Conflict` and the object is not deleted

#### Scenario: Delete with None version succeeds unconditionally
- **WHEN** `delete` is called with `expected_version = None`
- **THEN** the object is deleted regardless of its current version

#### Scenario: Delete for missing object returns NotFound
- **WHEN** `delete` is called for a key/name pair that does not exist
- **THEN** the error is `AppError::NotFound`

### Requirement: InMemoryStore uses DashMap for concurrent access
The `InMemoryStore` implementation SHALL use `DashMap<(ResourceKey, String), StoredObject>` as its backing store and `std::sync::atomic::AtomicU64` as its version counter. It SHALL implement the `ObjectStore` trait.

#### Scenario: Concurrent creates from multiple threads succeed
- **WHEN** multiple threads call `create` with different names simultaneously
- **THEN** all creates succeed without deadlock or data corruption

#### Scenario: Concurrent reads do not block each other
- **WHEN** multiple threads call `get` simultaneously
- **THEN** all reads complete without blocking each other
