## 1. Add Helper Functions

- [ ] 1.1 Add `SystemMetadata::initial()` constructor in `src/object/types.rs` that sets `resource_version = 1`, `generation = 1`, `created_at = Utc::now()`, `updated_at = Utc::now()`
- [ ] 1.2 Add `test_stored_object(key, name, spec)` helper function in test modules to reduce boilerplate when constructing `StoredObject` in tests

## 2. Change ObjectStore::create() Signature

- [ ] 2.1 Update `ObjectStore` trait in `src/store/mod.rs`: change `create(key, meta, spec)` to `create(object: StoredObject)`
- [ ] 2.2 Update `InMemoryStore::create()` in `src/store/memory.rs` to accept `StoredObject` and persist it as-is without modifying metadata
- [ ] 2.3 Update `SQLiteStore::create()` in `src/store/sqlite.rs` to accept `StoredObject` and persist it as-is without modifying metadata
- [ ] 2.4 Run `cargo check` to identify all compilation errors from the signature change

## 3. Remove Global State from Stores

- [ ] 3.1 Remove `next_version: AtomicU64` field from `InMemoryStore` struct in `src/store/memory.rs`
- [ ] 3.2 Remove `next_version()` method from `InMemoryStore` in `src/store/memory.rs`
- [ ] 3.3 Remove `next_version: Arc<AtomicU64>` field from `SQLiteStore` struct in `src/store/sqlite.rs`
- [ ] 3.4 Remove `init_version_counter()` method from `SQLiteStore` in `src/store/sqlite.rs`
- [ ] 3.5 Remove call to `init_version_counter()` in `SQLiteStore::new()` in `src/store/sqlite.rs`

## 4. Remove Metadata Bumping from transaction()

- [ ] 4.1 Update `InMemoryStore::transaction()` in `src/store/memory.rs`: remove `resource_version` bumping and `updated_at` setting in `TransactionOp::Apply` arm — persist object as-is
- [ ] 4.2 Update `SQLiteStore::transaction()` in `src/store/sqlite.rs`: remove `resource_version` bumping and `updated_at` setting in `TransactionOp::Apply` arm — persist object as-is
- [ ] 4.3 Update `TransactionOp::Apply` doc comment in `src/store/mod.rs` to reflect that store no longer auto-bumps metadata

## 5. Add Centralized Metadata Wrapper in Service

- [ ] 5.1 Add `apply_with_metadata()` helper function in `src/object/service.rs` that wraps transaction callbacks and automatically handles rv increment, generation bump (if spec changed), timestamp updates, and created_at preservation
- [ ] 5.2 Add comment explaining the wrapper's purpose: centralizes metadata logic to eliminate the "update_status landmine" and ensure consistency

## 6. Update Service Callbacks to Use Wrapper

- [ ] 6.1 Update `validate_and_update_object()` in `src/object/service.rs` to use `apply_with_metadata()` wrapper instead of manually setting metadata
- [ ] 6.2 Update `validate_and_update_schema()` in `src/object/service.rs` to use `apply_with_metadata()` wrapper instead of manually setting metadata
- [ ] 6.3 Update `update_status()` in `src/object/service.rs` to use `apply_with_metadata()` wrapper — generation will be automatically preserved because spec doesn't change
- [ ] 6.4 Remove the "preserve metadata" code smell: delete lines that set `resource_version` and `created_at` from `existing` in callbacks (the wrapper handles this)

## 7. Move OCC Check to Service Layer

- [ ] 7.1 Add OCC check in `validate_and_update_object()` callback: compare incoming object's `resource_version` with existing, return `TransactionOp::Abort(AppError::Conflict)` if mismatch
- [ ] 7.2 Add OCC check in `validate_and_update_schema()` callback: compare incoming object's `resource_version` with existing, return `TransactionOp::Abort(AppError::Conflict)` if mismatch
- [ ] 7.3 Verify that `update_status()` does NOT perform OCC check (status updates are unconditional per spec)

## 8. Update Service create() to Set Initial Metadata

- [ ] 8.1 Update `validate_and_create_object()` in `src/object/service.rs` to construct `StoredObject` with `SystemMetadata::initial()` before calling `store.create()`
- [ ] 8.2 Update `validate_and_create_schema()` in `src/object/service.rs` to construct `StoredObject` with `SystemMetadata::initial()` before calling `store.create()`

## 9. Update Store Tests

- [ ] 9.1 Update `InMemoryStore` tests in `src/store/memory.rs` to construct full `StoredObject` using `test_stored_object()` helper instead of relying on store to populate metadata
- [ ] 9.2 Update `SQLiteStore` tests in `src/store/sqlite.rs` to construct full `StoredObject` using `test_stored_object()` helper instead of relying on store to populate metadata
- [ ] 9.3 Update store tests to assert that stored metadata matches what was passed in (not auto-generated)
- [ ] 9.4 Remove or update tests that verify store-level metadata bumping (e.g., `transaction_bumps_resource_version_on_apply`) — this behavior is now in the service

## 10. Update Service Tests

- [ ] 10.1 Verify service tests in `src/object/service.rs` still pass — they should verify metadata behavior through the service API
- [ ] 10.2 Add test for `apply_with_metadata()` wrapper: verify it increments rv, preserves created_at, updates updated_at
- [ ] 10.3 Add test for `apply_with_metadata()` wrapper: verify it bumps generation on spec change
- [ ] 10.4 Add test for `apply_with_metadata()` wrapper: verify it preserves generation on no spec change (update_status scenario)
- [ ] 10.5 Add test for OCC check: verify update with wrong version returns Conflict

## 11. Update Integration Tests

- [ ] 11.1 Update integration tests in `tests/src/` that bypass service and call store directly — construct `SystemMetadata` when needed
- [ ] 11.2 Verify generation semantics test still passes (create → update same spec → update different spec → update status)
- [ ] 11.3 Run full integration test suite against both InMemoryStore and SQLiteStore

## 12. Update Documentation

- [ ] 12.1 Update `SystemMetadata.resource_version` doc comment in `src/object/types.rs` to reflect it's for CAS only, not watch ordering
- [ ] 12.2 Update `ObjectStore` trait doc comment in `src/store/mod.rs` to clarify that store does not modify metadata
- [ ] 12.3 Check `docs/` directory for any documentation that needs updating
- [ ] 12.4 Update `roadmap.md`: mark "Make the store dumb" as completed, verify no other items are impacted

## 13. Final Verification

- [ ] 13.1 Run `cargo clippy --all-targets --all-features -- -D warnings` and fix any issues
- [ ] 13.2 Run `cargo test` and verify all tests pass
- [ ] 13.3 Run integration tests: `cargo test --package kapi-tests`
- [ ] 13.4 Verify no compilation errors or warnings
- [ ] 13.5 Review changes to ensure store implementations are truly "dumb" (no metadata logic)
