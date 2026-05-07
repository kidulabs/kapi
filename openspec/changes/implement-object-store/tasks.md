## 1. ObjectStore Trait Definition

- [ ] 1.1 Define `ObjectStore` async trait in `src/store/mod.rs` with `create`, `get`, `list`, `update`, `delete` methods using `serde_json::Value` for data parameters
- [ ] 1.2 Add necessary imports (`async_trait`, `Value`, `ListOptions`, `ListResponse`, `StoredObject`, `AppError`, `ResourceKey`) to `src/store/mod.rs`
- [ ] 1.3 Verify `cargo build` succeeds with trait definition

## 2. InMemoryStore Structure

- [ ] 2.1 Implement `InMemoryStore` struct in `src/store/memory.rs` with `DashMap<(ResourceKey, String), StoredObject>` and `AtomicU64` fields
- [ ] 2.2 Implement `InMemoryStore::new()` constructor
- [ ] 2.3 Implement `next_version()` helper using `AtomicU64::fetch_add(1, Ordering::Relaxed)`
- [ ] 2.4 Implement `now()` helper for current UTC timestamp

## 3. Create and Get

- [ ] 3.1 Implement `create`: check for duplicates, assign version, set timestamps, insert into DashMap, return `StoredObject`
- [ ] 3.2 Implement `get`: lookup by key+name, return `StoredObject` or `NotFound`
- [ ] 3.3 Verify `cargo build` succeeds

## 4. List with Pagination

- [ ] 4.1 Implement `list`: filter by ResourceKey, sort by name ascending
- [ ] 4.2 Implement continue token decoding: base64 decode to get the skip-past name
- [ ] 4.3 Implement limit slicing and next continue token encoding
- [ ] 4.4 Return `ListResponse` with items and optional continue token

## 5. Update with Optimistic Concurrency

- [ ] 5.1 Implement `update`: lookup object, compare `expected_version` with stored version, return `Conflict` on mismatch
- [ ] 5.2 On version match: replace data, increment version, update `updated_at`, return updated `StoredObject`
- [ ] 5.3 Return `NotFound` if object does not exist

## 6. Delete with Optional Version Check

- [ ] 6.1 Implement `delete`: lookup object, if `expected_version` is `Some` verify match, return `Conflict` on mismatch
- [ ] 6.2 Remove entry from DashMap and return the deleted `StoredObject`
- [ ] 6.3 Return `NotFound` if object does not exist

## 7. Unit Tests

- [ ] 7.1 Write test: create + get round-trip verifies data integrity
- [ ] 7.2 Write test: duplicate create returns `Conflict`
- [ ] 7.3 Write test: get missing returns `NotFound`
- [ ] 7.4 Write test: list returns all objects sorted by name
- [ ] 7.5 Write test: list with limit returns correct count and continue token
- [ ] 7.6 Write test: list with continue token resumes from correct position
- [ ] 7.7 Write test: update with correct version succeeds and increments version
- [ ] 7.8 Write test: update with wrong version returns `Conflict`
- [ ] 7.9 Write test: update missing returns `NotFound`
- [ ] 7.10 Write test: delete returns the removed object and subsequent get returns `NotFound`
- [ ] 7.11 Write test: delete with wrong version returns `Conflict` and object remains
- [ ] 7.12 Write test: delete with `None` version succeeds unconditionally
- [ ] 7.13 Write test: delete missing returns `NotFound`
- [ ] 7.14 Run `cargo test` — all tests pass

## 8. Roadmap Progress Marking

- [ ] 8.1 Mark T13 (ObjectStore async trait) as done in `roadmap.md`
- [ ] 8.2 Mark T14 (InMemoryStore with DashMap) as done in `roadmap.md`
- [ ] 8.3 Mark T15 (AtomicU64 version counter) as done in `roadmap.md`
- [ ] 8.4 Mark T16 (optimistic concurrency in update) as done in `roadmap.md`
- [ ] 8.5 Mark T17 (optional version check in delete) as done in `roadmap.md`
- [ ] 8.6 Mark T18 (unit tests) as done in `roadmap.md`

## 9. Roadmap Deviation Review

- [ ] 9.1 Compare `src/store/mod.rs` against roadmap's module tree — verify structure matches
- [ ] 9.2 Compare `src/store/memory.rs` implementation against roadmap's ObjectStore trait signature — verify method signatures match
- [ ] 9.3 Compare `src/object/types.rs` against roadmap's Key Types section — verify field names and types match (e.g., `resource_version` vs `version`)
- [ ] 9.4 Compare `src/error.rs` against roadmap's AppError enum — verify variants and fields match
- [ ] 9.5 Review roadmap Design Decisions table — verify each decision is reflected in the actual code
- [ ] 9.6 Review roadmap Non-Goals section — verify no non-goal was accidentally implemented
- [ ] 9.7 Review roadmap Open Questions section — note any that have been resolved by this implementation
- [ ] 9.8 Apply corrections to `roadmap.md` for any deviations found
