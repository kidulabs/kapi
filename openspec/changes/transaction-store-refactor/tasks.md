# Transaction Store Refactor — Tasks

## Phase 1: Core Types and Trait

### Task 1.1: Add TransactionOp enum
- **File:** `src/store/mod.rs`
- **Description:** Add the `TransactionOp<T>` enum with variants: `Apply`, `Delete`, `Abort`, `NoOp`
- **Acceptance:** Enum compiles, is public, and documented

### Task 1.2: Add transaction method to ObjectStore trait
- **File:** `src/store/mod.rs`
- **Description:** Add `transaction<T>()` method to the `ObjectStore` trait with proper documentation
- **Acceptance:** Trait compiles, method signature matches design

## Phase 2: InMemoryStore Implementation

### Task 2.1: Implement transaction in InMemoryStore
- **File:** `src/store/memory.rs`
- **Description:** Implement the `transaction()` method using DashMap's per-key locking
- **Acceptance:** 
  - Method compiles
  - Lock is held across entire transaction
  - `Apply` bumps `resource_version` and `updated_at`
  - `Delete` removes object
  - `Abort` returns error
  - `NoOp` returns value

### Task 2.2: Add unit tests for InMemoryStore transaction
- **File:** `src/store/memory.rs`
- **Description:** Add tests for each `TransactionOp` variant
- **Acceptance:** All tests pass

## Phase 3: SQLiteStore Implementation

### Task 3.1: Add helper methods to SQLiteStore
- **File:** `src/store/sqlite.rs`
- **Description:** Add `fetch_object_locked()`, `persist_object_locked()`, `delete_object_locked()` helper methods
- **Acceptance:** Helper methods compile and work correctly

### Task 3.2: Implement transaction in SQLiteStore
- **File:** `src/store/sqlite.rs`
- **Description:** Implement the `transaction()` method using connection mutex
- **Acceptance:**
  - Method compiles
  - Lock is held across entire transaction
  - `Apply` bumps `resource_version` and `updated_at`
  - `Delete` removes object
  - `Abort` returns error
  - `NoOp` returns value

### Task 3.3: Add unit tests for SQLiteStore transaction
- **File:** `src/store/sqlite.rs`
- **Description:** Add tests for each `TransactionOp` variant
- **Acceptance:** All tests pass

## Phase 4: Service Layer Integration

### Task 4.1: Update ObjectService::update to use transaction
- **File:** `src/object/service.rs`
- **Description:** Rewrite `update()` to use `transaction()` with generation bumping logic
- **Acceptance:**
  - Generation bumps only on spec change
  - System metadata is preserved
  - All existing tests pass

### Task 4.2: Update ObjectService::delete to use transaction
- **File:** `src/object/service.rs`
- **Description:** Rewrite `delete()` to use `transaction()` with finalizer check
- **Acceptance:**
  - Hard delete when no finalizers
  - Soft delete (set deletionTimestamp) when finalizers present
  - All existing tests pass

### Task 4.3: Update ObjectService::update_status to use transaction
- **File:** `src/object/service.rs`
- **Description:** Rewrite `update_status()` to use `transaction()`
- **Acceptance:**
  - Status is updated
  - Generation does not bump
  - All existing tests pass

## Phase 5: Remove Old Methods

### Task 5.1: Remove update from ObjectStore trait
- **File:** `src/store/mod.rs`
- **Description:** Remove `update()` method from trait
- **Acceptance:** Trait compiles without `update()`

### Task 5.2: Remove update from InMemoryStore
- **File:** `src/store/memory.rs`
- **Description:** Remove `update()` implementation
- **Acceptance:** InMemoryStore compiles without `update()`

### Task 5.3: Remove update from SQLiteStore
- **File:** `src/store/sqlite.rs`
- **Description:** Remove `update()` implementation
- **Acceptance:** SQLiteStore compiles without `update()`

### Task 5.4: Remove delete from ObjectStore trait
- **File:** `src/store/mod.rs`
- **Description:** Remove `delete()` method from trait
- **Acceptance:** Trait compiles without `delete()`

### Task 5.5: Remove delete from InMemoryStore
- **File:** `src/store/memory.rs`
- **Description:** Remove `delete()` implementation
- **Acceptance:** InMemoryStore compiles without `delete()`

### Task 5.6: Remove delete from SQLiteStore
- **File:** `src/store/sqlite.rs`
- **Description:** Remove `delete()` implementation
- **Acceptance:** SQLiteStore compiles without `delete()`

### Task 5.7: Remove update_status from ObjectStore trait
- **File:** `src/store/mod.rs`
- **Description:** Remove `update_status()` method from trait
- **Acceptance:** Trait compiles without `update_status()`

### Task 5.8: Remove update_status from InMemoryStore
- **File:** `src/store/memory.rs`
- **Description:** Remove `update_status()` implementation
- **Acceptance:** InMemoryStore compiles without `update_status()`

### Task 5.9: Remove update_status from SQLiteStore
- **File:** `src/store/sqlite.rs`
- **Description:** Remove `update_status()` implementation
- **Acceptance:** SQLiteStore compiles without `update_status()`

## Phase 6: Integration Tests

### Task 6.1: Update integration tests
- **File:** `tests/src/*.rs`
- **Description:** Update all integration tests to use the new `transaction()` API
- **Acceptance:** All integration tests pass

### Task 6.2: Add concurrent transaction tests
- **File:** `tests/src/*.rs`
- **Description:** Add tests for concurrent transactions on the same object
- **Acceptance:** Tests verify atomicity and serialization

## Phase 7: Documentation

### Task 7.1: Update AGENTS.md
- **File:** `AGENTS.md`
- **Description:** Update architecture documentation to reflect transaction-based store
- **Acceptance:** Documentation accurately describes the new design

### Task 7.2: Update code comments
- **Files:** `src/store/*.rs`, `src/object/service.rs`
- **Description:** Ensure all code comments reflect the new design
- **Acceptance:** Comments are accurate and helpful

## Verification

### Final Check: Build and Test
- **Command:** `cargo build && cargo test`
- **Acceptance:** All builds and tests pass

### Final Check: Clippy
- **Command:** `cargo clippy -- -D warnings`
- **Acceptance:** No clippy warnings

### Final Check: Documentation
- **Command:** `cargo doc --no-deps`
- **Acceptance:** Documentation builds without errors
