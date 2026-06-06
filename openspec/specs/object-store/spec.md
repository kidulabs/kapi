## Purpose

Define the `ObjectStore` trait and its `InMemoryStore` implementation for persisting, retrieving, listing, updating, and deleting `StoredObject` instances identified by `ResourceKey` and name.
## Requirements
### Requirement: ObjectStore trait defines the storage contract
The system SHALL define an `ObjectStore` async trait with methods `create`, `get`, `list`, `update`, `delete`, `exists`, and `update_status` that operate on `StoredObject` instances. The trait SHALL require `Send + Sync`. The `create` method SHALL accept `ObjectMeta` for the metadata parameter (which includes `name` and `labels`) and `serde_json::Value` for the `spec` parameter. The `update` method SHALL accept a full `StoredObject` and perform optimistic concurrency control by comparing the embedded `object.system.resource_version` against the stored version. The `delete` method SHALL accept only `key` and `name` parameters and perform unconditional removal. The `exists` method SHALL accept a `ResourceKey` and return `Result<bool, AppError>` indicating whether any objects exist for that key. The `update_status` method SHALL accept `key: &ResourceKey`, `name: &str`, and `status: serde_json::Value`, and return `Result<StoredObject, AppError>`. It SHALL perform a server-side read-modify-write without optimistic concurrency checking (no CAS on `resource_version`).

#### Scenario: Trait is object-safe and thread-safe
- **WHEN** a type implements `ObjectStore`
- **THEN** it can be used as `dyn ObjectStore` inside `Arc` and sent across threads

### Requirement: ObjectStore trait documents generation contract

The `ObjectStore` trait definition SHALL include documentation specifying that:
- `create()` initializes `generation` to 1
- `update()` bumps `generation` iff `spec.value` differs from the stored value
- `update_status()` does NOT bump `generation`

#### Scenario: Trait documentation is present
- **WHEN** reading the `ObjectStore` trait definition
- **THEN** the generation behavior is documented in the trait's doc comment

#### Scenario: create accepts ObjectMeta and raw JSON value
- **WHEN** a caller invokes `create(key, meta, data)` with an `ObjectMeta` containing `name` and `labels` and a `serde_json::Value`
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

#### Scenario: Create object with labels
- **WHEN** `create()` is called with an object that has labels `{"app": "nginx", "env": "prod"}`
- **THEN** the object SHALL be stored with those labels in `metadata.labels`

#### Scenario: Create object without labels
- **WHEN** `create()` is called with an object that has empty labels
- **THEN** the object SHALL be stored with an empty `HashMap` in `metadata.labels`

### Requirement: generation field in SystemMetadata

`SystemMetadata` SHALL include a `generation: u64` field. This field is server-maintained and represents the number of times the object's spec has been changed. It SHALL be initialized to 1 on CREATE.

#### Scenario: New object has generation 1
- **WHEN** an object is created via `store.create()`
- **THEN** the returned `StoredObject.system.generation` equals 1

### Requirement: get retrieves an existing object
The `get` method SHALL return the `StoredObject` for the given `ResourceKey` and name, including any labels stored with the object. If no such object exists, it SHALL return `AppError::NotFound`.

#### Scenario: Successful get returns the stored object
- **WHEN** `get` is called for a key/name pair that exists
- **THEN** the returned `StoredObject` matches the stored data and includes labels in `metadata.labels`

#### Scenario: Get for missing object returns NotFound
- **WHEN** `get` is called for a key/name pair that does not exist
- **THEN** the error is `AppError::NotFound` with `what` and `identifier` fields populated

#### Scenario: Get object with labels
- **WHEN** `get()` is called for an object that has labels
- **THEN** the returned `StoredObject` SHALL have those labels in `metadata.labels`

#### Scenario: Get object without labels
- **WHEN** `get()` is called for an object with no labels
- **THEN** the returned `StoredObject` SHALL have an empty `HashMap` in `metadata.labels`

### Requirement: list returns paginated objects for a resource kind
The `list` method SHALL return all `StoredObject` instances matching the given `ResourceKey`, sorted by name in ascending order. Each returned object SHALL include its labels in `metadata.labels`. When `ListOptions.limit` is `Some(n)`, it SHALL return at most `n` items. When `ListOptions.continue_token` is `Some(token)`, it SHALL skip entries up to and including the name encoded in the token. The returned `ListResponse` SHALL include a `continue_token` if more items remain beyond the returned batch. When `ListOptions.field_selector` and/or `ListOptions.label_selector` are set, the store SHALL apply those filters before pagination.

#### Scenario: List returns all objects sorted by name
- **WHEN** `list` is called with no limit or continue token
- **THEN** all objects for the key are returned in ascending name order, each with their labels

#### Scenario: List with limit returns partial results with continue token
- **WHEN** `list` is called with `limit = Some(2)` and 5 objects exist
- **THEN** exactly 2 items are returned and `continue_token` is `Some`

#### Scenario: List with continue token resumes from correct position
- **WHEN** `list` is called with a continue token encoding name "b"
- **THEN** objects with names <= "b" are skipped and results start from the next name

#### Scenario: List with no matching objects returns empty list
- **WHEN** `list` is called for a key with no stored objects
- **THEN** the response has an empty `items` vector and `continue_token` is `None`

#### Scenario: List objects with mixed labels
- **WHEN** `list()` is called and some objects have labels while others do not
- **THEN** each returned `StoredObject` SHALL have its correct labels (or empty map)

#### Scenario: Filter applied before pagination
- **WHEN** `list()` is called with a filter and `limit=10`
- **THEN** the filter SHALL be applied first, then the result truncated to 10 items

#### Scenario: Filter with continue token
- **WHEN** `list()` is called with a filter and a continue token
- **THEN** the filter SHALL be applied, then the cursor skip, then truncation

### Requirement: update modifies an existing object with optimistic concurrency
The `update` method SHALL accept a `StoredObject` and replace the spec and `metadata` (including `labels`) of the existing object identified by `object.metadata.name` and the object's key. It SHALL compare the stored object's `system.resource_version` against `object.system.resource_version` and return `AppError::Conflict` if they do not match. On a successful update, it SHALL increment `resource_version` via the global counter, set `updated_at` to the current UTC time, and return the updated `StoredObject`. If the object does not exist, it SHALL return `AppError::NotFound`.

It SHALL also compare the incoming object's `spec.value` with the stored object's `spec.value`. If they differ (using `serde_json::Value` structural equality), it SHALL increment `generation` by 1. If they are equal, `generation` SHALL remain unchanged.

#### Scenario: Spec change bumps generation
- **WHEN** `update()` is called with a different `spec.value` than the stored object
- **THEN** the returned `StoredObject.system.generation` is exactly 1 greater than the stored generation

#### Scenario: Same spec does not bump generation
- **WHEN** `update()` is called with the same `spec.value` but different `metadata.labels`
- **THEN** the returned `StoredObject.system.generation` equals the stored generation (unchanged)

#### Scenario: Successful update increments version
- **WHEN** `update` is called with a `StoredObject` whose `system.resource_version` matches the stored version
- **THEN** the returned `StoredObject` has a higher `system.resource_version` and updated `system.updated_at`
- **AND** `metadata.labels` reflects the updated labels

#### Scenario: Update with wrong version returns conflict
- **WHEN** `update` is called with a `StoredObject` whose `system.resource_version` does not match the stored version
- **THEN** the error is `AppError::Conflict` with `expected` and `actual` fields

#### Scenario: Update for missing object returns NotFound
- **WHEN** `update` is called for a key/name pair that does not exist
- **THEN** the error is `AppError::NotFound`

#### Scenario: Update with label changes
- **WHEN** `update()` is called with changed labels
- **THEN** the object update and label changes SHALL be applied atomically

#### Scenario: Update with no label changes
- **WHEN** `update()` is called with the same labels as the existing object
- **THEN** no label table writes SHALL occur, only the object update

### Requirement: delete removes an object unconditionally
The `delete` method SHALL remove the object for the given `ResourceKey` and name and return the deleted `StoredObject`. It SHALL NOT perform any version check. If the object does not exist, it SHALL return `AppError::NotFound`. Any associated label data SHALL be deleted along with the object.

#### Scenario: Successful delete returns the deleted object
- **WHEN** `delete` is called for an existing object
- **THEN** the object is removed and the returned `StoredObject` matches the previously stored data

#### Scenario: Delete for missing object returns NotFound
- **WHEN** `delete` is called for a key/name pair that does not exist
- **THEN** the error is `AppError::NotFound`

#### Scenario: Delete is unconditional regardless of version
- **WHEN** `delete` is called for an existing object
- **THEN** the object is removed regardless of its current `resource_version`

#### Scenario: Delete object with labels
- **WHEN** `delete()` is called for an object that has labels
- **THEN** the object row SHALL be deleted and all associated label data SHALL be automatically deleted

### Requirement: ObjectStore update_status method
The `ObjectStore` trait SHALL define an `update_status` method that accepts `key: &ResourceKey`, `name: &str`, and `status: serde_json::Value`, and returns `Result<StoredObject, AppError>`. The method SHALL perform a server-side read-modify-write: read the current object, replace only the `status` field, bump `resource_version`, set `updated_at` to the current time, and write back. It SHALL NOT perform optimistic concurrency checking (no CAS on `resource_version`). It SHALL NOT modify the `generation` field. If the object does not exist, it SHALL return `AppError::NotFound`.

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

#### Scenario: Update status does not bump generation
- **WHEN** `update_status()` is called on an object with `generation: N`
- **THEN** the returned `StoredObject.system.generation` equals N (unchanged)

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

### Requirement: InMemoryStore uses DashMap for concurrent access
The `ObjectStore` trait SHALL have at least two implementations: `InMemoryStore` using `DashMap<(ResourceKey, String), StoredObject>` as its backing store with `std::sync::atomic::AtomicU64` as its version counter, and `SQLiteStore` using a SQLite database file with `rusqlite` as its backing store. Both SHALL implement the `ObjectStore` trait and produce identical behavior for all trait methods. `InMemoryStore::create()` SHALL store labels as part of `ObjectMeta` within the `StoredObject`. `InMemoryStore::update()` SHALL replace the entire `ObjectMeta` (including labels) with the updated version.

`InMemoryStore::list()` SHALL apply field and label filters in Rust after collecting objects but before sorting and pagination (order: collect → filter → sort → skip → truncate).

`SQLiteStore::list()` SHALL apply field and label filters as SQL WHERE clauses before pagination. Field filters SHALL use `AND name = ?` bindings. Label filters SHALL use `EXISTS`/`NOT EXISTS` subqueries on the `labels` table for each label requirement. Multiple label requirements SHALL be combined with AND semantics (multiple subqueries). All filtering SHALL happen before ORDER BY and LIMIT in the SQL query.

#### Scenario: Concurrent creates from multiple threads succeed
- **WHEN** multiple threads call `create` with different names simultaneously
- **THEN** all creates succeed without deadlock or data corruption

#### Scenario: Concurrent reads do not block each other
- **WHEN** multiple threads call `get` simultaneously
- **THEN** all reads complete without blocking each other

#### Scenario: Both implementations satisfy the same trait
- **WHEN** either `InMemoryStore` or `SQLiteStore` is used as `Arc<dyn ObjectStore>`
- **THEN** all trait methods behave identically for the same inputs

#### Scenario: Create object with labels in InMemoryStore
- **WHEN** `create()` is called with an object that has labels
- **THEN** the stored `StoredObject` SHALL contain those labels in `metadata.labels`

#### Scenario: Update object labels in InMemoryStore
- **WHEN** `update()` is called with new labels
- **THEN** the stored `StoredObject.metadata.labels` SHALL be replaced with the new labels

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

### Requirement: Integration test verifies generation semantics

The integration test suite SHALL include a test that verifies generation behavior across all store implementations. The test SHALL:
1. Create an object and verify `generation == 1`
2. Update with same spec, different labels, verify `generation` unchanged
3. Update with different spec, verify `generation` incremented
4. Update status, verify `generation` unchanged
5. Update with same spec, different labels again, verify `generation` unchanged

#### Scenario: Generation test passes for InMemoryStore
- **WHEN** the integration test runs against InMemoryStore
- **THEN** all generation assertions pass

#### Scenario: Generation test passes for SQLiteStore
- **WHEN** the integration test runs against SQLiteStore
- **THEN** all generation assertions pass

