## Why

The current `InMemoryStore` loses all data on restart. Adding a SQLite-backed `ObjectStore` implementation provides persistence with zero changes to the storage contract — the trait abstraction already supports it.

## What Changes

- Add `rusqlite` as a dependency
- Create `src/store/sqlite.rs` with `SQLiteStore` implementing `ObjectStore`
- `SQLiteStore` accepts a file path in its constructor, auto-creates the DB file, and runs schema initialization
- Wire `SQLiteStore` into `main.rs`, reading the DB path from `KAPI_DB_PATH` env var (fallback: `./kapi.db`)
- `InMemoryStore` remains unchanged and available for tests

## Capabilities

### New Capabilities
- `sqlite-store`: SQLite-backed persistent implementation of the `ObjectStore` trait, including schema design, CRUD operations, pagination, and optimistic concurrency via SQLite

### Modified Capabilities
- `object-store`: Add requirement for a second implementation (`SQLiteStore`) alongside `InMemoryStore`
- `app-config`: `main.rs` construction changes to use `SQLiteStore` instead of `InMemoryStore`

## Impact

- **New dependency**: `rusqlite` crate
- **New file**: `src/store/sqlite.rs` (~300 lines)
- **Modified file**: `src/main.rs` — store construction and env var parsing
- **No breaking changes**: The `ObjectStore` trait and all public APIs remain unchanged

## Non-goals

- Connection pooling (single connection is sufficient for SQLite's write serialization model)
- Migration system beyond `CREATE TABLE IF NOT EXISTS`
- Read replicas or WAL mode
- Changing or removing `InMemoryStore`
