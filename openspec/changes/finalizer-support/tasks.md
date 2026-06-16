# Finalizer Support — Implementation Tasks

## Phase 1: Data Model and Validation

### Task 1.1: Add `finalizers` field to `ObjectMeta`
- **File**: `src/object/types.rs`
- **Change**: Add `#[serde(default)] pub finalizers: Vec<String>` to `ObjectMeta` struct
- **Test**: Verify existing objects without `finalizers` deserialize correctly (backward compatibility)
- **Dependencies**: None

### Task 1.2: Add `deletion_timestamp` field to `SystemMetadata`
- **File**: `src/object/types.rs`
- **Change**: Add `#[serde(skip_serializing_if = "Option::is_none")] #[serde(default)] pub deletion_timestamp: Option<DateTime<Utc>>` to `SystemMetadata` struct
- **Test**: Verify existing objects without `deletion_timestamp` deserialize correctly
- **Dependencies**: None

### Task 1.3: Add `validate_finalizers` function
- **File**: `src/validation/mod.rs`
- **Change**: Add `pub fn validate_finalizers(finalizers: &[String]) -> Result<(), AppError>` that checks max 20 and reuses `validate_label_key` for each finalizer
- **Test**: Unit tests for valid names, invalid names, too many finalizers
- **Dependencies**: Task 1.1

### Task 1.4: Add `InvalidFinalizer` error variant
- **File**: `src/error.rs`
- **Change**: Add `InvalidFinalizer(String)` variant to `AppError` enum with HTTP 400 mapping
- **Test**: Verify error serialization and HTTP status
- **Dependencies**: None

### Task 1.5: Add `ObjectBeingDeleted` error variant
- **File**: `src/error.rs`
- **Change**: Add `ObjectBeingDeleted { name: String }` variant to `AppError` enum with HTTP 409 mapping
- **Test**: Verify error serialization and HTTP status
- **Dependencies**: None

## Phase 2: DELETE Path

### Task 2.1: Implement finalizer-aware DELETE
- **File**: `src/object/service.rs`
- **Change**: Modify `delete()` method to check `finalizers` and either hard-delete or mark for deletion
- **Logic**:
  - If `finalizers.is_empty()` → `TransactionOp::Delete`
  - If `deletion_timestamp.is_some()` → idempotent no-op (`TransactionOp::Apply(existing)`)
  - Otherwise → set `deletion_timestamp` and `TransactionOp::Apply(marked)`
- **Event logic**: Use a mutable flag to track action (HardDeleted, MarkedForDeletion, IdempotentNoOp) and publish events accordingly
- **Test**: Unit tests for all three paths
- **Dependencies**: Tasks 1.1, 1.2

### Task 2.2: Add DELETE integration tests
- **File**: `tests/integration_tests.rs` (or wherever integration tests live)
- **Tests**:
  - DELETE object without finalizers → hard delete, Deleted event
  - DELETE object with finalizers → mark for deletion, Modified event, `deletion_timestamp` set
  - DELETE object with `deletion_timestamp` already set → idempotent 200, no event
  - Verify `resource_version` bumps on mark-for-deletion
- **Dependencies**: Task 2.1

## Phase 3: UPDATE Path

### Task 3.1: Add finalizer validation to UPDATE
- **File**: `src/object/service.rs`
- **Change**: Add `validate_finalizers(&object.metadata.finalizers)?` to `validate_and_update_object`
- **Test**: Verify invalid finalizers are rejected on update
- **Dependencies**: Task 1.3

### Task 3.2: Implement update-during-deletion enforcement
- **File**: `src/object/service.rs`
- **Change**: In `validate_and_update_object` transaction callback, check if `existing.system.deletion_timestamp.is_some()` and reject non-finalizer changes with `ObjectBeingDeleted` error
- **Helper**: Add `fn only_finalizers_changed(existing: &ObjectMeta, incoming: &ObjectMeta) -> bool`
- **Test**: Unit tests for rejection of spec/labels/annotations changes when deleting
- **Dependencies**: Tasks 1.2, 1.5

### Task 3.3: Implement hard-delete on finalizer removal
- **File**: `src/object/service.rs`
- **Change**: In `validate_and_update_object` transaction callback, if `existing.system.deletion_timestamp.is_some()` and `incoming.metadata.finalizers.is_empty()`, return `TransactionOp::Delete` instead of `Apply`
- **Event**: Publish `Deleted` event after hard delete
- **Test**: Unit test for finalizer removal triggering hard delete
- **Dependencies**: Tasks 1.2, 3.2

### Task 3.4: Add UPDATE integration tests
- **File**: `tests/integration_tests.rs`
- **Tests**:
  - UPDATE spec on deleting object → 409 ObjectBeingDeleted
  - UPDATE labels on deleting object → 409 ObjectBeingDeleted
  - UPDATE finalizers on deleting object → success, Modified event
  - UPDATE finalizers to empty on deleting object → hard delete, Deleted event
  - UPDATE finalizers to add new finalizer on deleting object → 409 ObjectBeingDeleted (wait, is this right? Let me check the design...)
  
  **Correction**: The design says "You cannot add finalizers to an object that is being deleted." But the `only_finalizers_changed` check allows any change to `finalizers` as long as other fields are unchanged. We need to add a check: if `deletion_timestamp.is_some()` and `incoming.finalizers.len() > existing.finalizers.len()`, reject.
  
  **Updated Task 3.2**: Add check for finalizer addition when deleting:
  ```rust
  if existing.system.deletion_timestamp.is_some() {
      if !Self::only_finalizers_changed(&existing.metadata, &incoming_metadata) {
          return TransactionOp::Abort(AppError::ObjectBeingDeleted { ... });
      }
      if incoming_metadata.finalizers.len() > existing.metadata.finalizers.len() {
          return TransactionOp::Abort(AppError::ObjectBeingDeleted { ... });
      }
  }
  ```
  
  Actually, simpler: check if any new finalizers were added:
  ```rust
  if existing.system.deletion_timestamp.is_some() {
      // Only allow finalizer removal, not addition
      for f in &incoming_metadata.finalizers {
          if !existing.metadata.finalizers.contains(f) {
              return TransactionOp::Abort(AppError::ObjectBeingDeleted { ... });
          }
      }
      // Also check that only finalizers changed
      if !Self::only_finalizers_changed(&existing.metadata, &incoming_metadata) {
          return TransactionOp::Abort(AppError::ObjectBeingDeleted { ... });
      }
  }
  ```
  
  **Dependencies**: Tasks 1.2, 1.5

## Phase 4: Metadata Preservation

### Task 4.1: Update `apply_with_metadata` to preserve `deletion_timestamp`
- **File**: `src/object/service.rs`
- **Change**: Add `new_obj.system.deletion_timestamp = existing.system.deletion_timestamp;` to `apply_with_metadata`
- **Test**: Verify that an update doesn't accidentally clear `deletion_timestamp`
- **Dependencies**: Task 1.2

## Phase 5: CREATE Path

### Task 5.1: Add finalizer validation to CREATE
- **File**: `src/object/service.rs`
- **Change**: Add `validate_finalizers(&meta.finalizers)?` to `validate_and_create_object` and `validate_and_create_schema` (even though schemas don't support finalizers in v1, validate for consistency)
- **Test**: Verify invalid finalizers are rejected on create
- **Dependencies**: Task 1.3

### Task 5.2: Add CREATE integration tests
- **File**: `tests/integration_tests.rs`
- **Tests**:
  - CREATE object with valid finalizers → success
  - CREATE object with invalid finalizer name → 400 InvalidFinalizer
  - CREATE object with >20 finalizers → 400 InvalidFinalizer
- **Dependencies**: Task 5.1

## Phase 6: Handler Edge Validation

### Task 6.1: Add finalizer validation to handler
- **File**: `src/object/handler.rs`
- **Change**: Add `validate_finalizers(&meta.finalizers)?` to `create` and `update` handlers (defense-in-depth)
- **Test**: Verify handler rejects invalid finalizers before reaching service
- **Dependencies**: Task 1.3

## Phase 7: OpenAPI Spec

### Task 7.1: Update OpenAPI spec for `ObjectMeta`
- **File**: `src/openapi/components.rs` or wherever OpenAPI is generated
- **Change**: Add `finalizers` field to `ObjectMeta` schema (array of strings, optional)
- **Test**: Verify OpenAPI spec includes the new field
- **Dependencies**: Task 1.1

### Task 7.2: Update OpenAPI spec for `SystemMetadata`
- **File**: `src/openapi/components.rs`
- **Change**: Add `deletionTimestamp` field to `SystemMetadata` schema (string, date-time, optional)
- **Test**: Verify OpenAPI spec includes the new field
- **Dependencies**: Task 1.2

### Task 7.3: Update OpenAPI spec for error responses
- **File**: `src/openapi/components.rs`
- **Change**: Add `ObjectBeingDeleted` and `InvalidFinalizer` error responses
- **Test**: Verify OpenAPI spec includes the new error types
- **Dependencies**: Tasks 1.4, 1.5

## Phase 8: Documentation

### Task 8.1: Add inline documentation
- **Files**: `src/object/types.rs`, `src/object/service.rs`
- **Change**: Add doc comments explaining finalizer semantics, `deletion_timestamp` behavior, and update-during-deletion enforcement
- **Dependencies**: All implementation tasks

### Task 8.2: Update user guide (if exists)
- **File**: `docs/` or `README.md`
- **Change**: Add section on finalizers for controller authors
- **Dependencies**: All implementation tasks

## Phase 9: Edge Cases and Concurrency

### Task 9.1: Test concurrent DELETEs
- **File**: `tests/integration_tests.rs`
- **Test**: Two DELETEs in flight on same object → first marks for deletion, second is idempotent
- **Dependencies**: Task 2.1

### Task 9.2: Test DELETE racing with finalizer removal
- **File**: `tests/integration_tests.rs`
- **Test**: DELETE and UPDATE (removing finalizers) in flight → both outcomes are valid
- **Dependencies**: Tasks 2.1, 3.3

### Task 9.3: Test CREATE same-name after DELETE-with-finalizers
- **File**: `tests/integration_tests.rs`
- **Test**: DELETE object with finalizers, then CREATE same name → AlreadyExists error
- **Dependencies**: Task 2.1

## Phase 10: Backward Compatibility

### Task 10.1: Test deserialization of existing objects
- **File**: `src/object/types.rs` (unit tests)
- **Test**: Deserialize JSON without `finalizers` field → defaults to empty vec
- **Test**: Deserialize JSON without `deletionTimestamp` field → defaults to None
- **Dependencies**: Tasks 1.1, 1.2

### Task 10.2: Test SQLite migration (if needed)
- **File**: `src/store/sqlite.rs`
- **Change**: Verify that existing SQLite data can be read without migration (should work due to `#[serde(default)]`)
- **Test**: Create object with old schema, upgrade code, read object → should work
- **Dependencies**: Tasks 1.1, 1.2

## Phase 11: End-to-End Test Prompts

### Task 11.1: Update test index in `docs/testprompt.md`
- **File**: `docs/testprompt.md`
- **Change**: Add new row to the Test Index table for finalizer tests
- **Add**: `| Finalizers | create / delete / update / validation / watch | 52, 53, 54, 55, 56, 57, 58, 59 |`
- **Dependencies**: None (can be done in parallel with implementation)

### Task 11.2: Add Test 52 — Create object with finalizers
- **File**: `docs/testprompt.md`
- **Test**: Create object with `metadata.finalizers: ["example.io/cleanup", "kapi.io/finalizer"]`
- **Verify**: Response contains finalizers, GET returns same finalizers
- **Dependencies**: Task 1.1

### Task 11.3: Add Test 53 — Create object without finalizers (verify empty array)
- **File**: `docs/testprompt.md`
- **Test**: Create object without `metadata.finalizers`
- **Verify**: Response contains `"finalizers": []`
- **Dependencies**: Task 1.1

### Task 11.4: Add Test 54 — DELETE object with finalizers (mark for deletion)
- **File**: `docs/testprompt.md`
- **Test**: DELETE object with `finalizers: ["example.io/cleanup"]`
- **Verify**: 
  - Response is 200 with object
  - `system.deletionTimestamp` is set
  - Object still exists (GET returns it)
  - Watch receives Modified event (not Deleted)
- **Dependencies**: Task 2.1

### Task 11.5: Add Test 55 — DELETE object without finalizers (hard delete)
- **File**: `docs/testprompt.md`
- **Test**: DELETE object with `finalizers: []`
- **Verify**: 
  - Response is 200 with object
  - Object no longer exists (GET returns 404)
  - Watch receives Deleted event
- **Note**: This is already covered by Test 2 (lifecycle test), but add explicit finalizer context
- **Dependencies**: Task 2.1

### Task 11.6: Add Test 56 — Idempotent DELETE on already-deleting object
- **File**: `docs/testprompt.md`
- **Test**: DELETE object that already has `deletionTimestamp` set
- **Verify**: 
  - Response is 200 with object
  - No new Modified event published (check watch log)
  - `deletionTimestamp` unchanged
- **Dependencies**: Task 2.1

### Task 11.7: Add Test 57 — UPDATE spec on deleting object (rejected)
- **File**: `docs/testprompt.md`
- **Test**: 
  1. Create object with finalizers
  2. DELETE it (marks for deletion)
  3. Try to UPDATE spec
- **Verify**: 
  - Response is 409 Conflict
  - Error code is `ObjectBeingDeleted`
- **Dependencies**: Task 3.2

### Task 11.8: Add Test 58 — UPDATE finalizers on deleting object (allowed)
- **File**: `docs/testprompt.md`
- **Test**: 
  1. Create object with `finalizers: ["example.io/cleanup", "kapi.io/finalizer"]`
  2. DELETE it (marks for deletion)
  3. UPDATE to remove one finalizer: `finalizers: ["example.io/cleanup"]`
- **Verify**: 
  - Response is 200
  - `finalizers` updated
  - `deletionTimestamp` still set
  - Watch receives Modified event
- **Dependencies**: Task 3.2

### Task 11.9: Add Test 59 — UPDATE to empty finalizers triggers hard delete
- **File**: `docs/testprompt.md`
- **Test**: 
  1. Create object with `finalizers: ["example.io/cleanup"]`
  2. DELETE it (marks for deletion)
  3. UPDATE to `finalizers: []`
- **Verify**: 
  - Response is 200 with object (including `deletionTimestamp`)
  - Object no longer exists (GET returns 404)
  - Watch receives Deleted event
- **Dependencies**: Task 3.3

### Task 11.10: Add Test 60 — Finalizer validation (invalid name)
- **File**: `docs/testprompt.md`
- **Test**: Create object with `finalizers: ["invalid name with spaces"]`
- **Verify**: 
  - Response is 400 Bad Request
  - Error code is `InvalidFinalizer`
- **Dependencies**: Task 1.3

### Task 11.11: Add Test 61 — Finalizer validation (too many finalizers)
- **File**: `docs/testprompt.md`
- **Test**: Create object with 21 finalizers
- **Verify**: 
  - Response is 400 Bad Request
  - Error code is `InvalidFinalizer`
  - Error message mentions "max 20"
- **Dependencies**: Task 1.3

### Task 11.12: Add Test 62 — Watch events for finalizer lifecycle
- **File**: `docs/testprompt.md`
- **Test**: 
  1. Start watch
  2. Create object with finalizers
  3. DELETE it (mark for deletion)
  4. UPDATE to remove finalizers (hard delete)
- **Verify**: Watch receives:
  - Added event (create)
  - Modified event (mark for deletion, `deletionTimestamp` set)
  - Deleted event (hard delete)
- **Dependencies**: Tasks 2.1, 3.3

### Task 11.13: Add Test 63 — CREATE same-name after DELETE-with-finalizers
- **File**: `docs/testprompt.md`
- **Test**: 
  1. Create object with finalizers
  2. DELETE it (marks for deletion, object still exists)
  3. Try to CREATE with same name
- **Verify**: 
  - Response is 409 Conflict
  - Error code is `AlreadyExists`
- **Dependencies**: Task 2.1

### Task 11.14: Add Test 64 — UPDATE adds finalizer on deleting object (rejected)
- **File**: `docs/testprompt.md`
- **Test**: 
  1. Create object with `finalizers: ["example.io/cleanup"]`
  2. DELETE it (marks for deletion)
  3. Try to UPDATE to add finalizer: `finalizers: ["example.io/cleanup", "kapi.io/new"]`
- **Verify**: 
  - Response is 409 Conflict
  - Error code is `ObjectBeingDeleted`
- **Dependencies**: Task 3.2

### Task 11.15: Add Test 65 — SQLite persistence with finalizers
- **File**: `docs/testprompt.md`
- **Test**: 
  1. Start server with SQLite
  2. Create object with finalizers
  3. DELETE it (marks for deletion)
  4. Restart server
  5. GET object
- **Verify**: 
  - Object still exists after restart
  - `finalizers` preserved
  - `deletionTimestamp` preserved
- **Dependencies**: Tasks 1.1, 1.2, 10.2

### Task 11.16: Update Test 2 (lifecycle) to verify finalizers field
- **File**: `docs/testprompt.md`
- **Change**: In Test 2 (watch all events), verify that created object has `"finalizers": []` in the response
- **Note**: This is a minor addition to an existing test to verify backward compatibility
- **Dependencies**: Task 1.1

## Summary

**Total tasks**: 43 (27 implementation + 16 testprompt updates)
**Critical path**: Tasks 1.1 → 1.2 → 2.1 → 3.2 → 3.3 (data model → DELETE → UPDATE enforcement → hard-delete on finalizer removal)

**Estimated effort**: 3-4 days for a focused implementation (2-3 days implementation + 1 day testprompt updates)

**Risk areas**:
- Concurrency testing (Tasks 9.1, 9.2) — may require careful test setup
- Event suppression logic (Task 2.1) — needs careful state tracking
- Update-during-deletion enforcement (Task 3.2) — needs to handle both "only finalizers changed" and "no finalizers added" checks
- Test 56 (idempotent DELETE) — needs to verify no event is published, which requires careful watch log inspection
