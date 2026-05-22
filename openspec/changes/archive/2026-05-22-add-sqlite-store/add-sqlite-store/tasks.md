## 1. Dependency and Module Setup

- [x] 1.1 Add `rusqlite` dependency to `Cargo.toml`
- [x] 1.2 Create `src/store/sqlite.rs` module file
- [x] 1.3 Add `pub mod sqlite;` to `src/store/mod.rs`

## 2. SQLiteStore Core Implementation

- [x] 2.1 Implement `SQLiteStore` struct with `Arc<Mutex<Connection>>` and `AtomicU64` version counter
- [x] 2.2 Implement `SQLiteStore::new(path: &str)` — create parent dirs, open DB, run schema init, return store
- [x] 2.3 Implement schema initialization: `CREATE TABLE IF NOT EXISTS objects` with composite PK and index
- [x] 2.4 Add helper: serialize `StoredObject` to DB row (JSON data, RFC 3339 timestamps)
- [x] 2.5 Add helper: deserialize DB row to `StoredObject` (parse JSON, parse timestamps)

## 3. ObjectStore Trait Implementation

- [x] 3.1 Implement `create` — INSERT with conflict detection via `INSERT OR FAIL` on composite PK
- [x] 3.2 Implement `get` — SELECT by composite key, deserialize to `StoredObject`
- [x] 3.3 Implement `list` — SELECT with `WHERE group/version/kind`, `ORDER BY name ASC`, `LIMIT`, and continue token skip
- [x] 3.4 Implement `update` — UPDATE with `resource_version` check in WHERE clause for optimistic concurrency
- [x] 3.5 Implement `delete` — DELETE by composite key, return deleted object
- [x] 3.6 Wrap all trait methods in `tokio::task::spawn_blocking` to avoid blocking async runtime

## 4. Wire Into main.rs

- [x] 4.1 Update `main.rs` to read `KAPI_DB_PATH` env var with fallback to `./kapi.db`
- [x] 4.2 Replace `InMemoryStore` with `SQLiteStore` in `AppConfig` construction
- [x] 4.3 Verify `InMemoryStore` remains accessible for tests (not removed or made private)

## 5. Tests

- [x] 5.1 Add unit tests for `SQLiteStore` in `src/store/sqlite.rs`: create/get round-trip, duplicate conflict, get missing
- [x] 5.2 Add unit tests for `SQLiteStore` list: sorted results, limit + continue token, resume from token, empty key
- [x] 5.3 Add unit tests for `SQLiteStore` update: correct version succeeds, wrong version conflict, missing not found
- [x] 5.4 Add unit tests for `SQLiteStore` delete: returns object, subsequent get not found, missing not found
- [x] 5.5 Add persistence test: create object, drop store, recreate store, verify object still exists

## 6. Documentation Review

- [x] 6.1 Review `docs/storage.md` — update "Pluggability" table to show `SQLiteStore` as implemented, add `SQLiteStore` section, adjust `InMemoryStore` description
- [x] 6.2 Review `docs/architecture.md` — update Store layer description (line ~71), add `sqlite.rs` to module tree
- [x] 6.3 Review `docs/data-model.md` — check for any storage-related content that needs updating
- [x] 6.4 Review `docs/api-reference.md` — verify no changes needed (API surface should be unchanged)
- [x] 6.5 Update `roadmap.md`: move "Persistent storage" from "Out of Scope" to completed, remove from Out of Scope list

## 7. Verification

- [x] 7.1 Run `cargo check` and `cargo clippy` — no errors or warnings
- [x] 7.2 Run `cargo test` — all tests pass (existing + new)
