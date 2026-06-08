# Transaction Store Refactor — Tasks

## Phase 1: Core Types and Trait

### Task 1.1: Add TransactionOp enum
- **File:** `src/store/mod.rs`
- **Description:** Add the `TransactionOp` enum with variants: `Apply`, `Delete`, `Abort`
- **Acceptance:** Enum compiles, is public, and documented
- **Status:** ✅ DONE

### Task 1.2: Add transaction method to ObjectStore trait
- **File:** `src/store/mod.rs`
- **Description:** Add `transaction()` method to the `ObjectStore` trait with proper documentation
- **Acceptance:** Trait compiles, method signature matches design
- **Status:** ✅ DONE

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
- **Status:** ✅ DONE

### Task 2.2: Add unit tests for InMemoryStore transaction
- **File:** `src/store/memory.rs`
- **Description:** Add tests for each `TransactionOp` variant
- **Acceptance:** All tests pass
- **Status:** ✅ DONE

## Phase 3: SQLiteStore Implementation

### Task 3.1: Add helper methods to SQLiteStore
- **File:** `src/store/sqlite.rs`
- **Description:** Add `fetch_object_locked()`, `persist_object_locked()`, `delete_object_locked()` helper methods
- **Acceptance:** Helper methods compile and work correctly
- **Status:** ✅ DONE

### Task 3.2: Implement transaction in SQLiteStore
- **File:** `src/store/sqlite.rs`
- **Description:** Implement the `transaction()` method using connection mutex
- **Acceptance:**
  - Method compiles
  - Lock is held across entire transaction
  - `Apply` bumps `resource_version` and `updated_at`
  - `Delete` removes object
  - `Abort` returns error
- **Status:** ✅ DONE

### Task 3.3: Add unit tests for SQLiteStore transaction
- **File:** `src/store/sqlite.rs`
- **Description:** Add tests for each `TransactionOp` variant
- **Acceptance:** All tests pass
- **Status:** ✅ DONE

## Phase 4: Service Layer Integration

### Task 4.1: Update ObjectService::update to use transaction
- **File:** `src/object/service.rs`
- **Description:** Rewrite `update()` to use `transaction()` with generation bumping logic
- **Acceptance:**
  - Generation bumps only on spec change
  - System metadata is preserved
  - All existing tests pass
- **Status:** ✅ DONE

### Task 4.2: Update ObjectService::delete to use transaction
- **File:** `src/object/service.rs`
- **Description:** Rewrite `delete()` to use `transaction()`
- **Acceptance:**
  - Hard delete (TransactionOp::Delete)
  - All existing tests pass
- **Status:** ✅ DONE

### Task 4.3: Update ObjectService::update_status to use transaction
- **File:** `src/object/service.rs`
- **Description:** Rewrite `update_status()` to use `transaction()`
- **Acceptance:**
  - Status is updated via Apply on existing clone
  - Generation does not bump
  - All existing tests pass
- **Status:** ✅ DONE

## Phase 5: Remove Old Methods

### Task 5.1-5.9: Remove update/delete/update_status from ObjectStore trait and all implementations
- **Files:** `src/store/mod.rs`, `src/store/memory.rs`, `src/store/sqlite.rs`
- **Description:** Remove `update()`, `delete()`, `update_status()` from trait and both implementations
- **Acceptance:** All builds pass
- **Status:** ✅ DONE

## Phase 6: Integration Tests

### Task 6.1: Update unit and integration tests
- **Files:** `src/store/memory.rs`, `src/store/sqlite.rs`, `src/object/service.rs`, `tests/src/`
- **Description:** Updated all tests to use the new `transaction()` API; removed OCP-specific tests
- **Acceptance:** All 199 unit tests pass
- **Status:** ✅ DONE

### Task 6.2: Add concurrent transaction tests
- **File:** `src/store/*.rs`, `tests/src/*.rs`
- **Description:** Added tests for transaction atomicity, abort, and delete behavior
- **Acceptance:** Tests verify correctness
- **Status:** ✅ DONE

## Phase 7: Documentation

### Task 7.1: Update AGENTS.md
- **File:** `AGENTS.md`
- **Description:** Update architecture documentation to reflect transaction-based store
- **Acceptance:** Documentation accurately describes the new design
- **Status:** ✅ DONE

### Task 7.2: Update code comments
- **Files:** `src/store/*.rs`, `src/object/service.rs`
- **Description:** Code comments updated to reflect the new transaction-based design
- **Acceptance:** Comments are accurate and helpful
- **Status:** ✅ DONE

## Verification

### Final Check: Build and Test
- **Command:** `cargo test --lib`
- **Acceptance:** All 199 tests pass
- **Status:** ✅ PASSED

### Final Check: Clippy
- **Command:** `cargo clippy --all-targets -- -D warnings`
- **Acceptance:** No clippy warnings
- **Status:** ✅ PASSED

### Final Check: Full Build
- **Command:** `cargo build --all-targets`
- **Acceptance:** All targets compile
- **Status:** ✅ PASSED
