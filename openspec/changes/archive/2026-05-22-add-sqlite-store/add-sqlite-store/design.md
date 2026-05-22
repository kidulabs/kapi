## Context

The `ObjectStore` trait (`src/store/mod.rs`) already defines the storage contract with `create`, `get`, `list`, `update`, and `delete`. Currently only `InMemoryStore` (DashMap-backed) implements it. The trait is `Send + Sync` and used as `Arc<dyn ObjectStore>` in `AppConfig`, making it straightforward to add a second implementation.

## Goals / Non-Goals

**Goals:**
- Persistent storage via SQLite that survives process restarts
- Same `ObjectStore` contract — no API changes
- Auto-initialize schema on construction (`CREATE TABLE IF NOT EXISTS`)
- Zero-config default: env var `KAPI_DB_PATH` with fallback to `./kapi.db`

**Non-Goals:**
- Connection pooling (single `Arc<Mutex<Connection>>` is sufficient)
- WAL mode or read replicas
- Migration versioning system
- Removing or modifying `InMemoryStore`

## Decisions

### Decision 1: rusqlite over sqlx

**Choice:** `rusqlite` with `tokio::task::spawn_blocking` for async wrapping.

**Rationale:**
- rusqlite is lightweight, mature, and has minimal dependency footprint
- SQLite serializes writes anyway — connection pooling provides no real concurrency benefit for this workload
- sqlx adds compile-time SQL checking but introduces significant complexity (pool setup, async runtime integration) for simple CRUD
- `spawn_blocking` is idiomatic for wrapping sync I/O in tokio

**Alternatives considered:**
- **sqlx**: Rejected — overkill for single-table CRUD, heavier deps
- **libsql**: Rejected — less mature ecosystem

### Decision 2: Single connection behind `Arc<Mutex<Connection>>`

**Choice:** One `rusqlite::Connection` wrapped in `Arc<std::sync::Mutex<Connection>>`.

**Rationale:**
- SQLite's write serialization means concurrent writes block anyway
- Read concurrency with a single connection is acceptable for this API server's expected load
- Simpler than pool management, no connection lifecycle concerns
- If concurrency becomes a bottleneck, WAL mode + pool can be added later

**Alternatives considered:**
- **r2d2 pool**: Rejected — adds complexity without meaningful benefit for SQLite's concurrency model
- **Arc<Mutex> with WAL**: Deferred — can enable WAL later if needed

### Decision 3: Single table with JSON columns

**Choice:** One `objects` table with all fields, `data` stored as JSON text.

```sql
CREATE TABLE IF NOT EXISTS objects (
  group            TEXT    NOT NULL,
  version          TEXT    NOT NULL,
  kind             TEXT    NOT NULL,
  name             TEXT    NOT NULL,
  data             TEXT    NOT NULL,
  resource_version INTEGER NOT NULL,
  created_at       TEXT    NOT NULL,
  updated_at       TEXT    NOT NULL,
  PRIMARY KEY (group, version, kind, name)
);
```

**Rationale:**
- Maps directly to `StoredObject` struct — no joins needed
- `data` is always fetched/stored as a unit with metadata
- Composite primary key eliminates need for surrogate IDs
- Index on `(group, version, kind, name)` supports all query patterns

**Alternatives considered:**
- **Normalized tables**: Rejected — overkill since all columns are always accessed together
- **Rowid primary key + unique constraint**: Rejected — composite PK is cleaner for lookups

### Decision 4: Auto-migrate on construction

**Choice:** `SQLiteStore::new(path)` opens the DB and runs `CREATE TABLE IF NOT EXISTS` before returning.

**Rationale:**
- Idempotent — safe to run every startup
- Zero extra API surface — no `migrate()` method to remember
- Simple enough that a migration system is unnecessary until schema evolves

### Decision 5: Serialization format

**Choice:**
- `DateTime<Utc>` → RFC 3339 string via `chrono`'s `to_rfc3339()` / `DateTime::parse_from_rfc3339()`
- `serde_json::Value` → `serde_json::to_string()` / `serde_json::from_str()`

**Rationale:**
- Human-readable in the DB for debugging
- No dependency on rusqlite extension features

## Risks / Trade-offs

| Risk | Mitigation |
|------|-----------|
| `spawn_blocking` overhead per operation | Negligible for CRUD operations; can batch later if needed |
| Single connection becomes bottleneck under high read concurrency | Enable WAL mode or add read pool as future work |
| DB file path doesn't exist (parent directory missing) | Create parent directories with `std::fs::create_dir_all` before opening |
| JSON serialization errors on malformed data | Return `AppError::Internal` — should not happen with validated data |
| `rusqlite` is blocking by nature | All trait methods already `async`; `spawn_blocking` handles it transparently |
