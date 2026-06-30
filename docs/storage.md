# kapi — Storage & Events

## ObjectStore Trait

The `ObjectStore` trait defines a pluggable storage backend for all objects, including Schema objects.

```rust
#[async_trait]
pub trait ObjectStore: Send + Sync {
    /// Persist a complete StoredObject as-is.
    /// Does NOT modify any system metadata fields.
    async fn create(&self, object: StoredObject) -> Result<StoredObject, AppError>;
    async fn get(&self, key: &ResourceKey, namespace: Option<&str>, name: &str) -> Result<StoredObject, AppError>;
    async fn list(&self, key: &ResourceKey, namespace: Option<&str>, opts: ListOptions) -> Result<ListResponse, AppError>;
    /// Atomic read-modify-write transaction.
    /// The callback receives the existing object and returns a TransactionOp.
    /// The store does NOT modify system metadata — the callback is responsible.
    fn transaction(&self, key: &ResourceKey, namespace: Option<&str>, name: &str, op: Box<dyn FnOnce(&StoredObject) -> TransactionOp + Send>) -> Result<StoredObject, AppError>;
    async fn exists(&self, key: &ResourceKey) -> Result<bool, AppError>;
}
```

### Design Notes

- **Dumb store**: The store is a pure persistence layer. It does NOT maintain any global version counters, bump `resource_version`, update timestamps, or manage `generation`. All system metadata is provided by the caller (service layer) and persisted as-is.
- **Schema is also an object** — there's one store for everything. Schema objects use kind `"Schema"` in group `kapi.io`.
- **Namespace-aware**: All mutating and querying methods accept `namespace: Option<&str>`. When `None`, the operation is cross-namespace (e.g., list across all namespaces). When `Some`, the operation is scoped to that namespace. The namespace is used as part of the composite key alongside group, version, kind, and name.
- **`create`** accepts a complete `StoredObject` with all system metadata pre-populated by the service. The store persists it as-is, including `metadata.namespace`.
- **`transaction`** is the single mutation path for updates, deletes, and status changes. The callback performs domain logic and returns a `TransactionOp`. The store executes the operation but does NOT modify metadata.
- **`list`** with `namespace: None` returns objects from all namespaces (cross-namespace list). With `namespace: Some(ns)`, returns only objects in that namespace.
- **The caller** (ObjectService or SchemaService) is responsible for: resource_version increment, generation bump on spec change, timestamp updates, created_at preservation, and OCC checks. See `apply_with_metadata()` in `object/helpers.rs`.

## InMemoryStore

The in-memory implementation uses `DashMap<(ResourceKey, String), StoredObject>`.

```rust
pub struct InMemoryStore {
    objects: DashMap<(ResourceKey, Option<String>, String), StoredObject>,
}
```

Key behaviors:

- **Dumb store:** No global version counter. Objects are persisted as-is with caller-provided metadata.
- **Namespace-aware:** Composite key is `(ResourceKey, Option<String>, String)` — `(group, version, kind, namespace, name)`. Cluster-scoped objects have `namespace: None`; namespaced objects have `namespace: Some(ns)`.
- **Pagination:** Results sorted alphabetically by name within namespace. For cross-namespace lists, sorted by `(namespace, name)`. Cursor-based pagination with base64-encoded continue tokens encoding `(namespace, name)`.
- **Conflict detection:** Create checks for duplicate `(key, namespace, name)` tuples. OCC check for updates is performed by the service layer inside the transaction callback.
- **Thread safety:** All operations use `DashMap` for concurrent access without external synchronization.

## SQLiteStore

The persistent implementation uses SQLite via `rusqlite`, wrapped in `Arc<Mutex<Connection>>` with `tokio::task::spawn_blocking` for async compatibility.

```rust
pub struct SQLiteStore {
    conn: Arc<Mutex<Connection>>,
}
```

Key behaviors:

- **Construction:** `SQLiteStore::new(path)` creates parent directories, opens (or creates) the SQLite database, and runs schema initialization automatically.
- **Dumb store:** No global version counter. No `init_version_counter()` on startup. Objects are persisted as-is with caller-provided metadata.
- **Namespace-aware:** Objects table includes a `namespace TEXT` column. Composite primary key is `(resource_group, api_version, resource_kind, namespace, name)`. The `namespace` column is `NULL` for cluster-scoped objects.
- **Schema:** Two tables:
  - `objects` — composite primary key `(resource_group, api_version, resource_kind, namespace, name)`, JSON spec column, nullable `status` TEXT column, RFC 3339 timestamps
  - `labels` — separate table for label storage (see below)
- **Pagination:** SQL-level `ORDER BY name ASC` (or `ORDER BY namespace, name ASC` for cross-namespace) with `LIMIT` and name/namespace skip condition for efficient cursor-based pagination
- **Conflict detection:** `INSERT` relies on SQLite's primary key constraint for duplicate detection; OCC check for updates is performed by the service layer inside the transaction callback.
- **Thread safety:** Single connection behind `Arc<std::sync::Mutex>`, all blocking calls wrapped in `spawn_blocking`
- **Configuration:** DB path read from `KAPI_DB_PATH` env var with fallback to `./kapi.db`

### Labels Table

Labels are stored in a separate `labels` table to support efficient querying without embedding JSON blobs.

```sql
CREATE TABLE IF NOT EXISTS labels (
    resource_group  TEXT NOT NULL,
    api_version     TEXT NOT NULL,
    resource_kind   TEXT NOT NULL,
    namespace       TEXT,
    name            TEXT NOT NULL,
    label_key       TEXT NOT NULL,
    label_value     TEXT NOT NULL,
    PRIMARY KEY (resource_group, api_version, resource_kind, namespace, name, label_key),
    FOREIGN KEY (resource_group, api_version, resource_kind, namespace, name)
        REFERENCES objects(resource_group, api_version, resource_kind, namespace, name)
        ON DELETE CASCADE
);
```

Key design decisions:

- **Composite primary key** `(resource_group, api_version, resource_kind, namespace, name, label_key)` — namespace is part of the composite key, enabling the same object name in different namespaces to have independent label sets.
- **`ON DELETE CASCADE`** — when an object is deleted from the `objects` table, all its labels are automatically removed by SQLite. No manual cleanup is needed.
- **Index on `(group, version, kind, namespace, name)`** — accelerates label lookup by parent object, includes namespace for namespace-scoped lookups.

#### Diff-based label updates

On update, the store does not blindly delete and re-insert all labels. Instead it performs a diff (namespace-aware — labels include the namespace in the query):

1. Read existing labels from the `labels` table for the object (by key + namespace + name)
2. Compute keys to delete (in existing but not in new)
3. Compute keys to upsert (in new but value differs, or not in existing)
4. Apply deletes and upserts in the same transaction as the object update

This minimizes write load and avoids unnecessary row churn for unchanged labels.

#### Batch label queries

When listing objects, the store fetches labels for all returned objects in a single query using an `IN` clause (namespace-aware — queries include the namespace column):

```
SELECT namespace, name, label_key, label_value
FROM labels
WHERE resource_group = ? AND api_version = ? AND resource_kind = ? AND namespace ? AND name IN (...)
```

The results are grouped by `(namespace, name)` into a `HashMap<(Option<String>, String), HashMap<String, String>>` and merged into the returned objects. This avoids N+1 queries for paginated lists.

## EventPublisher Trait

Abstracts event distribution for SSE watch endpoints.

```rust
pub trait EventPublisher: Send + Sync {
    fn publish(&self, key: &ResourceKey, event: WatchEvent);
    fn subscribe(&self, key: &ResourceKey) -> WatchStream;
}
```

This trait isolates both services (`ObjectService` and `SchemaService`) from the concrete event bus implementation, enabling mock-based testing and future backends without touching the service layer.

## EventBus

The production implementation uses per-kind `tokio::broadcast` channels. Events carry `StoredObject` with `metadata.namespace`, enabling natural namespace-based filtering on the subscriber side.

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
- **Namespace-aware events:** No structural changes were needed in EventBus for namespace support — the namespace lives in `StoredObject.namespace`. Watchers subscribing via namespace-scoped endpoints receive only events for that namespace (filtered by `WatchFilter` or disconnected early if namespace doesn't match).

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

`SchemaRegistry` manages compiled user schemas in a `DashMap<String, CachedSchema>` keyed by versioned schema name (`{targetKind}.{targetGroup}.{targetVersion}`). Each cached entry holds:
- `validator: Arc<dyn SchemaValidator>` — the compiled JSON Schema validator
- `scope: String` — `"Namespaced"` or `"Cluster"`, used by ObjectService for scope validation

Two versions of the same kind occupy independent cache entries. Status validators are keyed as `{kind}.{group}.{version}.status`. The cache is:

- **Populated** on Schema creation and update (the jsonSchema is compiled and stored via `insert()` alongside the scope)
- **Invalidated** on Schema deletion (the entry is removed via `evict()`)
- **Lazily populated** on cache miss via `get_validator()` for objects created after a restart

This avoids re-compiling the JSON Schema on every object write and enables efficient scope lookup without an extra store read.

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
