## ADDED Requirements

### Requirement: SQLiteStore implements ObjectStore with persistent storage
The system SHALL provide a `SQLiteStore` type that implements the `ObjectStore` trait, backed by a SQLite database file. `SQLiteStore` SHALL be constructed with a file path, create the parent directories if they do not exist, open (or create) the SQLite database, and initialize the schema before returning.

#### Scenario: SQLiteStore constructs with valid path
- **WHEN** `SQLiteStore::new("/tmp/test/kapi.db")` is called
- **THEN** the directory `/tmp/test/` is created if it does not exist, the database file is opened or created, the schema is initialized, and a ready `SQLiteStore` is returned

#### Scenario: SQLiteStore implements ObjectStore trait
- **WHEN** a `SQLiteStore` is constructed
- **THEN** it can be used as `Arc<dyn ObjectStore>` and sent across threads

### Requirement: SQLiteStore schema uses a single objects table
The SQLite database SHALL contain a single `objects` table with columns `group`, `version`, `kind`, `name`, `data` (TEXT as JSON), `resource_version` (INTEGER), `created_at` (TEXT as RFC 3339), `updated_at` (TEXT as RFC 3339). The primary key SHALL be the composite `(group, version, kind, name)`. An index SHALL exist on `(group, version, kind, name)` for efficient lookups and pagination. The schema SHALL be created using `CREATE TABLE IF NOT EXISTS` so it is idempotent across restarts.

#### Scenario: Schema initialization is idempotent
- **WHEN** `SQLiteStore::new` is called on an existing database that already has the table
- **THEN** no error occurs and the store is ready to use

#### Scenario: Schema is created on first use
- **WHEN** `SQLiteStore::new` is called with a path to a non-existent file
- **THEN** the file is created and the `objects` table is created with the correct schema

### Requirement: SQLiteStore create inserts and returns StoredObject
The `create` method SHALL insert a new row into the `objects` table with a globally monotonic `resource_version` starting from 1 (tracked via a SQLite sequence or counter), set `created_at` and `updated_at` to the current UTC time as RFC 3339 strings, serialize `data` as JSON text, and return the resulting `StoredObject`. If a row with the same composite key already exists, it SHALL return `AppError::Conflict`.

#### Scenario: create inserts row and returns object with version 1
- **WHEN** `create` is called for a key/name pair that does not exist
- **THEN** a row is inserted, the returned `StoredObject` has `resource_version` >= 1, and the data is persisted

#### Scenario: create for duplicate returns conflict
- **WHEN** `create` is called for a key/name pair that already exists in the database
- **THEN** the error is `AppError::Conflict`

### Requirement: SQLiteStore get retrieves from database
The `get` method SHALL query the `objects` table by composite key, deserialize the JSON `data` column into `serde_json::Value`, parse RFC 3339 timestamps into `DateTime<Utc>`, and return the `StoredObject`. If no row matches, it SHALL return `AppError::NotFound`.

#### Scenario: get returns persisted object
- **WHEN** `get` is called for a key/name pair that exists
- **THEN** the returned `StoredObject` matches the stored data with correct metadata

#### Scenario: get for missing returns NotFound
- **WHEN** `get` is called for a key/name pair that does not exist
- **THEN** the error is `AppError::NotFound`

### Requirement: SQLiteStore list queries with pagination
The `list` method SHALL query the `objects` table filtering by `(group, version, kind)`, ordering by `name ASC`. When `ListOptions.limit` is `Some(n)`, it SHALL use `LIMIT n+1` to detect if more rows exist. When `ListOptions.continue_token` is `Some(token)`, it SHALL skip entries with `name <= decoded_token`. The returned `ListResponse` SHALL include a `continue_token` if more items remain.

#### Scenario: list returns all objects sorted by name
- **WHEN** `list` is called with no limit or continue token
- **THEN** all objects for the key are returned in ascending name order

#### Scenario: list with limit returns partial results with continue token
- **WHEN** `list` is called with `limit = Some(2)` and 5 objects exist
- **THEN** exactly 2 items are returned and `continue_token` is `Some`

#### Scenario: list with continue token resumes from correct position
- **WHEN** `list` is called with a continue token encoding name "b"
- **THEN** objects with names <= "b" are skipped and results start from the next name

### Requirement: SQLiteStore update with optimistic concurrency
The `update` method SHALL update the row identified by composite key, checking that the stored `resource_version` matches `object.metadata.resource_version`. On match, it SHALL set the new `data`, increment `resource_version`, update `updated_at`, and return the updated `StoredObject`. On version mismatch, it SHALL return `AppError::Conflict`. If the row does not exist, it SHALL return `AppError::NotFound`.

#### Scenario: update with correct version succeeds
- **WHEN** `update` is called with a `StoredObject` whose `resource_version` matches the stored version
- **THEN** the row is updated, `resource_version` is incremented, and the updated object is returned

#### Scenario: update with wrong version returns conflict
- **WHEN** `update` is called with a `StoredObject` whose `resource_version` does not match
- **THEN** the error is `AppError::Conflict`

### Requirement: SQLiteStore delete removes row unconditionally
The `delete` method SHALL remove the row identified by composite key and return the deleted `StoredObject`. It SHALL NOT check `resource_version`. If no row matches, it SHALL return `AppError::NotFound`.

#### Scenario: delete removes and returns object
- **WHEN** `delete` is called for an existing key/name pair
- **THEN** the row is removed and the returned `StoredObject` matches the previously stored data

#### Scenario: delete for missing returns NotFound
- **WHEN** `delete` is called for a key/name pair that does not exist
- **THEN** the error is `AppError::NotFound`

### Requirement: SQLiteStore uses rusqlite with spawn_blocking
All `ObjectStore` trait method implementations SHALL wrap rusqlite calls in `tokio::task::spawn_blocking` to avoid blocking the async runtime. The connection SHALL be held behind `Arc<std::sync::Mutex<rusqlite::Connection>>`.

#### Scenario: Operations do not block the async runtime
- **WHEN** multiple async operations are issued concurrently
- **THEN** they execute via the tokio thread pool without blocking the runtime
