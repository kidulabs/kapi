## MODIFIED Requirements

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

### Requirement: InMemoryStore uses DashMap for concurrent access
The `ObjectStore` trait SHALL have at least two implementations: `InMemoryStore` using `DashMap<(ResourceKey, String), StoredObject>` as its backing store, and `SQLiteStore` using a SQLite database file with `rusqlite` as its backing store. Both SHALL implement the `ObjectStore` trait and produce identical behavior for all trait methods. Neither implementation SHALL maintain global state for metadata generation.

#### Scenario: Concurrent creates from multiple threads succeed
- **WHEN** multiple threads call `create` with different names simultaneously
- **THEN** all creates succeed without deadlock or data corruption

#### Scenario: Concurrent reads do not block each other
- **WHEN** multiple threads call `get` simultaneously
- **THEN** all reads complete without blocking each other

#### Scenario: Both implementations satisfy the same trait
- **WHEN** either `InMemoryStore` or `SQLiteStore` is used as `Arc<dyn ObjectStore>`
- **THEN** all trait methods behave identically for the same inputs

## REMOVED Requirements

### Requirement: ObjectStore trait documents generation contract
**Reason**: Generation management is now the responsibility of the service layer, not the store. The store no longer initializes or bumps generation.
**Migration**: Generation logic is handled by the service layer's centralized metadata wrapper.

### Requirement: generation field in SystemMetadata
**Reason**: This requirement described store-level generation initialization, which is now handled by the service layer.
**Migration**: The service layer sets `generation = 1` when creating objects via `SystemMetadata::initial()`.

### Requirement: InMemoryStore implements update_status
**Reason**: `update_status` has been replaced by `transaction()` in a previous change. This requirement is obsolete.
**Migration**: Use `transaction()` with a callback that modifies the status field.

### Requirement: SQLiteStore implements update_status
**Reason**: `update_status` has been replaced by `transaction()` in a previous change. This requirement is obsolete.
**Migration**: Use `transaction()` with a callback that modifies the status field.

### Requirement: SQLite objects table has nullable status column
**Reason**: This requirement is already implemented and no longer needs to be specified separately. The status column exists in the schema.
**Migration**: No migration needed. The column already exists.
