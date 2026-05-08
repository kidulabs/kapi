## 1. ObjectStore Trait Definition

- [x] 1.1 Define `ObjectStore` async trait in `src/store/mod.rs` with `create`, `get`, `list`, `update`, `delete` methods using `serde_json::Value` for data parameters
- [x] 1.2 Add necessary imports (`async_trait`, `Value`, `ListOptions`, `ListResponse`, `StoredObject`, `AppError`, `ResourceKey`) to `src/store/mod.rs`
- [x] 1.3 Verify `cargo build` succeeds with trait definition

## 2. InMemoryStore Structure

- [x] 2.1 Implement `InMemoryStore` struct in `src/store/memory.rs` with `DashMap<(ResourceKey, String), StoredObject>` and `AtomicU64` fields
- [x] 2.2 Implement `InMemoryStore::new()` constructor
- [x] 2.3 Implement `next_version()` helper using `AtomicU64::fetch_add(1, Ordering::Relaxed)`
- [x] 2.4 Implement `now()` helper for current UTC timestamp

## 3. Create and Get

- [x] 3.1 Implement `create`: check for duplicates, assign version, set timestamps, insert into DashMap, return `StoredObject`
- [x] 3.2 Implement `get`: lookup by key+name, return `StoredObject` or `NotFound`
- [x] 3.3 Verify `cargo build` succeeds

## 4. List with Pagination

- [x] 4.1 Implement `list`: filter by ResourceKey, sort by name ascending
- [x] 4.2 Implement continue token decoding: base64 decode to get the skip-past name
- [x] 4.3 Implement limit slicing and next continue token encoding
- [x] 4.4 Return `ListResponse` with items and optional continue token

## 5. Update with Optimistic Concurrency

- [x] 5.1 Implement `update`: lookup object, compare `expected_version` with stored version, return `Conflict` on mismatch
- [x] 5.2 On version match: replace data, increment version, update `updated_at`, return updated `StoredObject`
- [x] 5.3 Return `NotFound` if object does not exist

## 6. Delete with Optional Version Check

- [x] 6.1 Implement `delete`: lookup object, if `expected_version` is `Some` verify match, return `Conflict` on mismatch
- [x] 6.2 Remove entry from DashMap and return the deleted `StoredObject`
- [x] 6.3 Return `NotFound` if object does not exist

## 7. Unit Tests

- [x] 7.1 Write test: create + get round-trip verifies data integrity
- [x] 7.2 Write test: duplicate create returns `Conflict`
- [x] 7.3 Write test: get missing returns `NotFound`
- [x] 7.4 Write test: list returns all objects sorted by name
- [x] 7.5 Write test: list with limit returns correct count and continue token
- [x] 7.6 Write test: list with continue token resumes from correct position
- [x] 7.7 Write test: update with correct version succeeds and increments version
- [x] 7.8 Write test: update with wrong version returns `Conflict`
- [x] 7.9 Write test: update missing returns `NotFound`
- [x] 7.10 Write test: delete returns the removed object and subsequent get returns `NotFound`
- [x] 7.11 Write test: delete with wrong version returns `Conflict` and object remains
- [x] 7.12 Write test: delete with `None` version succeeds unconditionally
- [x] 7.13 Write test: delete missing returns `NotFound`
- [x] 7.14 Write test: list with no matching objects returns empty list with no continue token
- [x] 7.15 Write test: delete with matching version succeeds and removes object
- [x] 7.16 Run `cargo test` — all tests pass

## 8. Roadmap Progress Marking

- [x] 8.1 Mark T13 (ObjectStore async trait) as done in `roadmap.md`
- [x] 8.2 Mark T14 (InMemoryStore with DashMap) as done in `roadmap.md`
- [x] 8.3 Mark T15 (AtomicU64 version counter) as done in `roadmap.md`
- [x] 8.4 Mark T16 (optimistic concurrency in update) as done in `roadmap.md`
- [x] 8.5 Mark T17 (optional version check in delete) as done in `roadmap.md`
- [x] 8.6 Mark T18 (unit tests) as done in `roadmap.md`

## 9. Roadmap Deviation Review

- [x] 9.1 Compare `src/store/mod.rs` against roadmap's module tree — verify structure matches
- [x] 9.2 Compare `src/store/memory.rs` implementation against roadmap's ObjectStore trait signature — verify method signatures match
- [x] 9.3 Compare `src/object/types.rs` against roadmap's Key Types section — verify field names and types match (e.g., `resource_version` vs `version`)
- [x] 9.4 Compare `src/error.rs` against roadmap's AppError enum — verify variants and fields match
- [x] 9.5 Review roadmap Design Decisions table — verify each decision is reflected in the actual code
- [x] 9.6 Review roadmap Non-Goals section — verify no non-goal was accidentally implemented
- [x] 9.7 Review roadmap Open Questions section — note any that have been resolved by this implementation
- [x] 9.8 Apply corrections to `roadmap.md` for any deviations found
