## Purpose

Define the `ObjectStore` trait and its `InMemoryStore` implementation for persisting, retrieving, listing, updating, and deleting `StoredObject` instances identified by `ResourceKey` and name.

## Requirements

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

### Requirement: create stores a new object and assigns a version
The `create` method SHALL store a new object with the given `ResourceKey`, name, and data. It SHALL assign a globally monotonic `resource_version` starting from 1, set `created_at` and `updated_at` to the current UTC time, and return the resulting `StoredObject`. If an object with the same key and name already exists, it SHALL return `AppError::AlreadyExists`.

#### Scenario: Successful create returns stored object with version 1
- **WHEN** `create` is called for a key/name pair that does not exist
- **THEN** the returned `StoredObject` has `resource_version` >= 1, `created_at` set, and `updated_at` equal to `created_at`

#### Scenario: Duplicate create returns AlreadyExists
- **WHEN** `create` is called for a key/name pair that already exists
- **THEN** the error is `AppError::AlreadyExists` with the resource kind and name populated

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

### Requirement: InMemoryStore uses DashMap for concurrent access
The `ObjectStore` trait SHALL have at least two implementations: `InMemoryStore` using `DashMap<(ResourceKey, String), StoredObject>` as its backing store with `std::sync::atomic::AtomicU64` as its version counter, and `SQLiteStore` using a SQLite database file with `rusqlite` as its backing store. Both SHALL implement the `ObjectStore` trait and produce identical behavior for all trait methods.

#### Scenario: Concurrent creates from multiple threads succeed
- **WHEN** multiple threads call `create` with different names simultaneously
- **THEN** all creates succeed without deadlock or data corruption

#### Scenario: Concurrent reads do not block each other
- **WHEN** multiple threads call `get` simultaneously
- **THEN** all reads complete without blocking each other

#### Scenario: Both implementations satisfy the same trait
- **WHEN** either `InMemoryStore` or `SQLiteStore` is used as `Arc<dyn ObjectStore>`
- **THEN** all trait methods behave identically for the same inputs

### Requirement: InMemoryStore visibility restricted to crate
The `InMemoryStore` module SHALL be declared `pub(crate)` in `src/store/mod.rs` so it is visible only within the `kapi` crate, not to external consumers.

#### Scenario: InMemoryStore accessible within crate
- **WHEN** code within the kapi crate (main.rs, tests) imports `crate::store::memory::InMemoryStore`
- **THEN** the import succeeds and `InMemoryStore` can be constructed

#### Scenario: InMemoryStore not accessible outside crate
- **WHEN** an external crate depends on `kapi` and attempts to import `kapi::store::memory::InMemoryStore`
- **THEN** the compiler rejects the import

### Requirement: InMemoryStore test accessibility preserved
All existing tests that construct `InMemoryStore` directly SHALL continue to compile and pass. This includes tests in `src/store/memory.rs`, `src/object/service.rs`, and `src/openapi.rs`.

#### Scenario: Service tests construct InMemoryStore
- **WHEN** `make_service()` in `src/object/service.rs` tests creates `Arc::new(InMemoryStore::new())`
- **THEN** compilation succeeds and tests pass

#### Scenario: OpenAPI tests construct InMemoryStore
- **WHEN** `make_test_service()` in `src/openapi.rs` tests creates `std::sync::Arc::new(crate::store::memory::InMemoryStore::new())`
- **THEN** compilation succeeds and tests pass
