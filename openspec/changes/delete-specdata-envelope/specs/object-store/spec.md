## MODIFIED Requirements

### Requirement: InMemoryStore uses DashMap for concurrent access
The `ObjectStore` trait SHALL have at least two implementations: `InMemoryStore` using `DashMap<(ResourceKey, String), StoredObject>` as its backing store, and `SQLiteStore` using a SQLite database file with `rusqlite` as its backing store. Both SHALL implement the `ObjectStore` trait and produce identical behavior for all trait methods. Neither implementation SHALL maintain global state for metadata generation. Both SHALL persist `StoredObject.spec` and `StoredObject.status` as `serde_json::Value` directly, with no envelope wrapper.

#### Scenario: Concurrent creates from multiple threads succeed
- **WHEN** multiple threads call `create` with different names simultaneously
- **THEN** all creates succeed without deadlock or data corruption

#### Scenario: Concurrent reads do not block each other
- **WHEN** multiple threads call `get` simultaneously
- **THEN** all reads complete without blocking each other

#### Scenario: Both implementations satisfy the same trait
- **WHEN** either `InMemoryStore` or `SQLiteStore` is used as `Arc<dyn ObjectStore>`
- **THEN** all trait methods behave identically for the same inputs

#### Scenario: Stores persist spec and status as inline JSON values
- **WHEN** `create` is called with a `StoredObject` whose `spec` and `status` are `serde_json::Value`
- **THEN** the stores SHALL persist the values as-is, with no wrapper or envelope
- **AND** `get` SHALL return `StoredObject` with `spec` and `status` as the same `serde_json::Value` instances (or semantically equivalent parsed values)
