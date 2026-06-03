# kapi — Storage & Events

## ObjectStore Trait

The `ObjectStore` trait defines a pluggable storage backend for all objects, including Schema objects.

```rust
#[async_trait]
pub trait ObjectStore: Send + Sync {
    async fn create(&self, key: &ResourceKey, meta: ObjectMeta, spec: Value)
        -> Result<StoredObject, AppError>;
    async fn get(&self, key: &ResourceKey, name: &str)
        -> Result<StoredObject, AppError>;
    async fn list(&self, key: &ResourceKey, opts: ListOptions)
        -> Result<ListResponse, AppError>;
    async fn update(&self, object: StoredObject)
        -> Result<StoredObject, AppError>;
    async fn delete(&self, key: &ResourceKey, name: &str)
        -> Result<StoredObject, AppError>;
    async fn update_status(&self, key: &ResourceKey, name: &str, status: Value)
        -> Result<StoredObject, AppError>;
}
```

### Design Notes

- **Schema is also an object** — there's one store for everything. Schema objects use kind `"Schema"` in group `kapi.io`.
- **`create`/`get`/`list`** take `(key, name)` — the object doesn't exist yet (create) or the caller may not have the full object (get, list).
- **`update`** takes the full `StoredObject`. The implementation peeks at `object.system.resource_version` for optimistic concurrency control. On match, applies data, bumps version, updates `updated_at`. On mismatch, returns `Conflict`.
- **`delete`** is unconditional — no version check. Returns the deleted object.
- **`update_status`** updates only the `status` field without optimistic concurrency control. Reads the object, replaces `status`, bumps `resource_version`, sets `updated_at`, returns updated object.
- **`key` and `name`** from the incoming object during `update` are trusted from the URL, not the client payload. The handler validates the match before calling the store.

## InMemoryStore

The in-memory implementation uses `DashMap<(ResourceKey, String), StoredObject>`.

```rust
pub struct InMemoryStore {
    objects: DashMap<(ResourceKey, String), StoredObject>,
    next_version: AtomicU64,
}
```

Key behaviors:

- **Versioning:** Global monotonic `AtomicU64` counter, starts at 1, incremented on every create/update
- **Pagination:** Results sorted alphabetically by name. Cursor-based pagination with base64-encoded continue tokens. The token encodes the last item name in the current page.
- **Conflict detection:** Create checks for duplicate `(key, name)` pairs. Update compares stored `resource_version` against the supplied version.
- **Thread safety:** All operations use `DashMap` for concurrent access without external synchronization.
- **Status handling:** The `status` field is `None` on create. `update_status` replaces the status value, bumps `resource_version`, and sets `updated_at`.

## SQLiteStore

The persistent implementation uses SQLite via `rusqlite`, wrapped in `Arc<Mutex<Connection>>` with `tokio::task::spawn_blocking` for async compatibility.

```rust
pub struct SQLiteStore {
    conn: Arc<Mutex<Connection>>,
    next_version: Arc<AtomicU64>,
}
```

Key behaviors:

- **Construction:** `SQLiteStore::new(path)` creates parent directories, opens (or creates) the SQLite database, and runs schema initialization automatically
- **Schema:** Two tables:
  - `objects` — composite primary key `(group, version, kind, name)`, JSON spec column, nullable `status` TEXT column, RFC 3339 timestamps
  - `labels` — separate table for label storage (see below)
- **Versioning:** Global monotonic `AtomicU64` counter, initialized from `MAX(resource_version)` on startup
- **Pagination:** SQL-level `ORDER BY name ASC` with `LIMIT` and `name > ?` skip condition for efficient cursor-based pagination
- **Conflict detection:** `INSERT` relies on SQLite's primary key constraint for duplicate detection; `UPDATE` uses `resource_version` in WHERE clause for optimistic concurrency
- **Thread safety:** Single connection behind `Arc<std::sync::Mutex>`, all blocking calls wrapped in `spawn_blocking`
- **Configuration:** DB path read from `KAPI_DB_PATH` env var with fallback to `./kapi.db`

### Labels Table

Labels are stored in a separate `labels` table to support efficient querying without embedding JSON blobs.

```sql
CREATE TABLE IF NOT EXISTS labels (
    resource_group  TEXT NOT NULL,
    api_version     TEXT NOT NULL,
    resource_kind   TEXT NOT NULL,
    name            TEXT NOT NULL,
    label_key       TEXT NOT NULL,
    label_value     TEXT NOT NULL,
    PRIMARY KEY (resource_group, api_version, resource_kind, name, label_key),
    FOREIGN KEY (resource_group, api_version, resource_kind, name)
        REFERENCES objects(resource_group, api_version, resource_kind, name)
        ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_labels_gvkn
    ON labels(resource_group, api_version, resource_kind, name);
```

Key design decisions:

- **Composite primary key** `(resource_group, api_version, resource_kind, name, label_key)` — each label row is uniquely identified by its parent object and label key. This prevents duplicate label keys on the same object at the database level.
- **`ON DELETE CASCADE`** — when an object is deleted from the `objects` table, all its labels are automatically removed by SQLite. No manual cleanup is needed.
- **Index on `(group, version, kind, name)`** — accelerates label lookup by parent object, used by both single-object queries and batch list queries.

#### Diff-based label updates

On update, the store does not blindly delete and re-insert all labels. Instead it performs a diff:

1. Read existing labels from the `labels` table for the object
2. Compute keys to delete (in existing but not in new)
3. Compute keys to upsert (in new but value differs, or not in existing)
4. Apply deletes and upserts in the same transaction as the object update

This minimizes write load and avoids unnecessary row churn for unchanged labels.

#### Batch label queries

When listing objects, the store fetches labels for all returned objects in a single query using an `IN` clause:

```
SELECT name, label_key, label_value
FROM labels
WHERE resource_group = ? AND api_version = ? AND resource_kind = ? AND name IN (...)
```

The results are grouped by `name` into a `HashMap<String, HashMap<String, String>>` and merged into the returned objects. This avoids N+1 queries for paginated lists.

## EventPublisher Trait

Abstracts event distribution for SSE watch endpoints.

```rust
pub trait EventPublisher: Send + Sync {
    fn publish(&self, key: &ResourceKey, event: WatchEvent);
    fn subscribe(&self, key: &ResourceKey) -> WatchStream;
}
```

This trait isolates `ObjectService` from the concrete event bus implementation, enabling mock-based testing and future backends without touching the service layer.

## EventBus

The production implementation uses per-kind `tokio::broadcast` channels.

```rust
pub struct EventBus {
    channels: DashMap<ResourceKey, broadcast::Sender<WatchEvent>>,
    capacity: usize,  // default: 1024
}
```

Key behaviors:

- **Lazy channel creation:** Channels are created on first `subscribe`, not on `publish`. No allocation for kinds nobody is watching.
- **Dead channel cleanup:** On `publish`, if a channel has zero receivers, it is removed. A single surviving subscriber keeps the channel alive.
- **Fire-and-forget publishing:** `publish` never blocks. If there are no receivers, the event is silently dropped.
- **WatchStream:** Wraps `BroadcastStream` to hide tokio internals. On lag (`RecvError::Lagged(n)`), the stream terminates with `None` — honest signaling matching watch semantics.

## WatchStream

```rust
pub struct WatchStream {
    inner: BroadcastStream<WatchEvent>,
}

impl Stream for WatchStream {
    type Item = WatchEvent;
}
```

- Normal delivery: forwards the event
- Lagged subscriber: terminates stream (client must re-sync)
- Channel closed: terminates stream
- `Send + Sync`: safe for Axum SSE handlers

## SchemaValidator Trait

Abstracts JSON Schema validation behind a trait, isolating the service layer from the `jsonschema` crate.

```rust
pub trait SchemaValidator: Send + Sync {
    fn is_valid(&self, instance: &Value) -> bool;
    fn validate(&self, instance: &Value) -> Vec<SchemaValidationError>;
}
```

## JsonSchemaValidator

Production implementation wrapping `jsonschema::Validator` (Draft 2020-12).

```rust
pub struct JsonSchemaValidator {
    inner: jsonschema::Validator,
}

impl JsonSchemaValidator {
    pub fn compile(schema_json: &Value) -> Result<Self, anyhow::Error>;
}
```

## Meta-Schema

A hardcoded JSON Schema (Draft 2020-12, `unevaluatedProperties: false`) that defines the shape of valid Schema registration payloads:

```json
{
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "type": "object",
    "required": ["targetGroup", "targetVersion", "targetKind", "jsonSchema"],
    "properties": {
        "targetGroup": { "type": "string", "minLength": 1 },
        "targetVersion": { "type": "string", "minLength": 1 },
        "targetKind": { "type": "string", "minLength": 1 },
        "jsonSchema": { "type": "object" }
    },
    "unevaluatedProperties": false
}
```

Compiled once at server startup via `compile_meta_schema()` and injected into `SchemaRegistry`.

## Schema Cache

`SchemaRegistry` manages compiled user schemas in a `DashMap<String, Arc<dyn SchemaValidator>>` keyed by schema name (`{targetKind}.{targetGroup}`). The cache is:

- **Populated** on Schema creation and update (the jsonSchema is compiled and stored via `insert()`)
- **Invalidated** on Schema deletion (the entry is removed via `evict()`)
- **Lazily populated** on cache miss via `get_validator()` for objects created after a restart

This avoids re-compiling the JSON Schema on every object write.

## Pluggability

The architecture supports swapping implementations at every layer:

| Layer | Trait | Implementations | Future Options |
|-------|-------|-----------------|----------------|
| Storage | `ObjectStore` | `InMemoryStore`, `SQLiteStore` | Postgres, etcd |
| Events | `EventPublisher` | `EventBus` (broadcast) | Redis pub/sub, NATS |
| Validation | `SchemaValidator` | `JsonSchemaValidator` | Custom validation rules |

Swap implementations by constructing `AppConfig` with different `Arc<dyn Trait>` values:

```rust
let config = AppConfig {
    port: 8080,
    store: Arc::new(MyCustomStore::new()),
    event_bus: Arc::new(MyEventBus::new()),
};
kapi::run(config).await?;
```
