## 1. Add generation field to SystemMetadata

- [ ] 1.1 Add `generation: u64` to `SystemMetadata` in `src/object/types.rs`
- [ ] 1.2 Add `#[serde(default)]` to `generation` for backward compatibility with existing serialized objects
- [ ] 1.3 Update all test fixtures in `src/object/types.rs` to include `generation: 1`
- [ ] 1.4 Verify `cargo build` succeeds

## 2. Document generation contract in ObjectStore trait

- [ ] 2.1 Add doc comment to `ObjectStore` trait in `src/store/mod.rs` specifying generation behavior:
  - `create()` initializes `generation` to 1
  - `update()` bumps `generation` iff `spec.value` differs
  - `update_status()` does NOT bump `generation`
- [ ] 2.2 Verify `cargo doc --no-deps` succeeds

## 3. Implement generation in InMemoryStore

- [ ] 3.1 Modify `InMemoryStore::create()` to set `generation: 1`
- [ ] 3.2 Modify `InMemoryStore::update()` to compare `old.spec.value != new.spec.value` and bump `generation` if different
- [ ] 3.3 Verify `update_status()` does NOT touch `generation` (already the case, confirm)
- [ ] 3.4 Update existing unit tests in `src/store/memory.rs` to account for `generation` field
- [ ] 3.5 Add unit test: metadata-only update does not bump generation
- [ ] 3.6 Add unit test: spec change bumps generation
- [ ] 3.7 Run `cargo test` — all tests pass

## 4. Implement generation in SQLiteStore

- [ ] 4.1 Add `generation INTEGER NOT NULL DEFAULT 1` column to `objects` table in `init_schema`
- [ ] 4.2 Modify `SQLiteStore::create()` to include `generation` in INSERT
- [ ] 4.3 Modify `SQLiteStore::update()` to compare spec and bump `generation` in UPDATE
- [ ] 4.4 Modify `row_to_stored_object()` to read `generation` column
- [ ] 4.5 Verify `update_status()` UPDATE statement does NOT modify `generation`
- [ ] 4.6 Update existing unit tests in `src/store/sqlite.rs` to account for `generation` field
- [ ] 4.7 Add unit test: metadata-only update does not bump generation
- [ ] 4.8 Add unit test: spec change bumps generation
- [ ] 4.9 Run `cargo test` — all tests pass

## 5. Add integration test for generation semantics

- [ ] 5.1 Add `test_generation_semantics` to integration test suite (`tests/src/`)
- [ ] 5.2 Test creates object, verifies `generation == 1`
- [ ] 5.3 Test updates with same spec + different labels, verifies `generation` unchanged
- [ ] 5.4 Test updates with different spec, verifies `generation` incremented
- [ ] 5.5 Test updates status, verifies `generation` unchanged
- [ ] 5.6 Test updates with same spec + different labels again, verifies `generation` unchanged
- [ ] 5.7 Run integration tests against both stores — all pass

## 6. Documentation review and enhancement

- [ ] 6.1 Review `src/store/mod.rs` trait documentation — verify generation contract is clearly stated
- [ ] 6.2 Review `src/object/types.rs` — verify `SystemMetadata` doc comment explains `generation` vs `resource_version`
- [ ] 6.3 Review `src/store/memory.rs` — verify inline comments explain the generation bump logic
- [ ] 6.4 Review `src/store/sqlite.rs` — verify inline comments explain the generation bump logic and schema migration
- [ ] 6.5 Correct any deviations between documented behavior and actual implementation
- [ ] 6.6 Verify `cargo doc --no-deps` renders generation documentation correctly

## 7. Add end-to-end test cases to testprompt.md

- [ ] 7.1 Add Test to `docs/testprompt.md`: generation starts at 1 on create
- [ ] 7.2 Add Test to `docs/testprompt.md`: metadata-only update (labels change) does NOT bump generation
- [ ] 7.3 Add Test to `docs/testprompt.md`: spec change bumps generation
- [ ] 7.4 Add Test to `docs/testprompt.md`: status update does NOT bump generation
- [ ] 7.5 Add Test to `docs/testprompt.md`: generation and resource_version are independent counters
- [ ] 7.6 Verify all new test cases run successfully against both InMemoryStore and SQLiteStore

## 8. Verification

- [ ] 8.1 Run `cargo test` — all unit tests pass
- [ ] 8.2 Run integration test binary — all scenarios pass
- [ ] 8.3 Run `cargo clippy -- -D warnings` — no warnings
- [ ] 8.4 Run `cargo doc --no-deps` — docs build cleanly
- [ ] 8.5 Update `roadmap.md` — mark generation field as complete
