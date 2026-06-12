## Purpose

Define the `ObjectStore` trait and its `InMemoryStore` implementation for persisting, retrieving, listing, and deleting `StoredObject` instances identified by `ResourceKey` and name.
## Requirements
### Requirement: ObjectStore trait defines the storage contract
The system SHALL define an `ObjectStore` async trait with methods `create`, `get`, `list`, `transaction`, and `exists` that operate on `StoredObject` instances. The trait SHALL require `Send + Sync`. The `create` method SHALL accept a complete `StoredObject` and persist it as-is. The `transaction` method SHALL accept a callback that returns a `TransactionOp`. The store SHALL NOT modify system metadata (resource_version, generation, timestamps) — it SHALL persist objects exactly as provided. The `exists` method SHALL accept a `ResourceKey` and return `Result<bool, AppError>` indicating whether any objects exist for that key.

#### Scenario: Trait is object-safe and thread-safe
- **WHEN** a type implements `ObjectStore`
- **THEN** it can be used as `dyn ObjectStore` inside `Arc` and sent across threads

#### Scenario: create accepts a complete StoredObject
- **WHEN** a caller invokes `create(object)` with a `StoredObject`
- **THEN** the implementation persists the object as-is, without modifying any system metadata fields

#### Scenario: exists checks for object presence
- **WHEN** a caller invokes `exists(key)` with a `ResourceKey`
- **THEN** the implementation returns `Ok(true)` if any objects exist for that key, `Ok(false)` otherwise

### Requirement: create stores a new object without modifying metadata
The `create` method SHALL store the provided `StoredObject` as-is. It SHALL NOT modify `system.resource_version`, `system.generation`, `system.created_at`, or `system.updated_at`. The caller (service layer) is responsible for setting all system metadata before calling `create`. If an object with the same key and name already exists, it SHALL return `AppError::AlreadyExists`.

#### Scenario: Successful create persists object with caller-provided metadata
- **WHEN** `create` is called with a `StoredObject` that has `system.resource_version = 1`, `system.generation = 1`, and timestamps set
- **THEN** the stored object SHALL have exactly those metadata values
- **AND** the returned `StoredObject` SHALL match the input

#### Scenario: Duplicate create returns AlreadyExists
- **WHEN** `create` is called for a key/name pair that already exists
- **THEN** the error is `AppError::AlreadyExists` with the resource kind and name populated

#### Scenario: Create object with labels
- **WHEN** `create()` is called with an object that has labels `{"app": "nginx", "env": "prod"}`
- **THEN** the object SHALL be stored with those labels in `metadata.labels`

#### Scenario: Create object without labels
- **WHEN** `create()` is called with an object that has empty labels
- **THEN** the object SHALL be stored with an empty `HashMap` in `metadata.labels`

### Requirement: Store implementations do not maintain global state
Store implementations SHALL NOT maintain a global version counter or any other global mutable state for metadata generation. `InMemoryStore` SHALL NOT have an `AtomicU64` field. `SQLiteStore` SHALL NOT have an `init_version_counter()` method or restore version state on startup.

#### Scenario: InMemoryStore has no global counter
- **WHEN** `InMemoryStore` is constructed
- **THEN** it SHALL NOT contain an `AtomicU64` or similar global counter

#### Scenario: SQLiteStore does not restore version state
- **WHEN** `SQLiteStore` is opened on an existing database
- **THEN** it SHALL NOT query `MAX(resource_version)` or restore any global counter

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

### Requirement: InMemoryStore uses DashMap for concurrent access
The `ObjectStore` trait SHALL have at least two implementations: `InMemoryStore` using `DashMap<(ResourceKey, String), StoredObject>` as its backing store, and `SQLiteStore` using a SQLite database file with `rusqlite` as its backing store. Both SHALL implement the `ObjectStore` trait and produce identical behavior for all trait methods. Neither implementation SHALL maintain global state for metadata generation.

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

