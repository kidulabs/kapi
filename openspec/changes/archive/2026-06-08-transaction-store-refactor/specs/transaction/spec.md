# Transaction Capability Specification

## Overview

The `transaction()` method provides atomic read-modify-write semantics for object mutations. It replaces the previous `update()`, `delete()`, and `update_status()` methods with a single, composable API.

## Requirements

### REQ-1: Atomicity

The transaction method MUST provide atomic read-modify-write semantics. No other operations on the same object can interleave between reading the existing object and applying the result.

**Verification:**
- Concurrent transactions on the same object must serialize
- No race conditions between read and write

### REQ-2: Callback-Based API

The transaction method MUST accept a callback that receives the existing object and returns a `TransactionOp` indicating the desired action.

**Signature:**
```rust
async fn transaction<T>(
    &self,
    key: &ResourceKey,
    name: &str,
    op: impl FnOnce(&StoredObject) -> TransactionOp<T> + Send,
) -> Result<T, AppError>;
```

### REQ-3: Transaction Operations

The callback MUST return one of the following `TransactionOp` variants:

- `Apply(StoredObject)` — Persist the provided object
- `Delete` — Hard-delete the object
- `Abort(AppError)` — Reject the operation with an error
- `NoOp(T)` — Do nothing, return the provided value

### REQ-4: Fast Callback Requirement

The callback MUST be fast and non-blocking. It MUST NOT perform I/O operations, network calls, or database queries.

**Rationale:** The store holds an exclusive lock on the object while the callback executes. Slow callbacks will block all other operations on the same object.

**Allowed:**
- Field validation
- Business logic checks
- Finalizer checks
- Object mutation (cloning and modifying)

**Forbidden:**
- HTTP requests
- Database queries
- File I/O
- Sleep or blocking operations

### REQ-5: Automatic Resource Version Bumping

The store MUST automatically bump the `resource_version` and update `updated_at` when `TransactionOp::Apply` is returned.

**Rationale:** This ensures consistency and prevents the service layer from needing to manage versioning.

### REQ-6: NotFound Handling

If the object does not exist, the store MUST return `AppError::NotFound` before calling the callback.

**Rationale:** The callback requires an existing object to operate on.

### REQ-7: Lock Release on Panic

If the callback panics, the store MUST release the lock and leave the object unchanged.

**Rationale:** Panics should not corrupt the object or leave locks held.

## Behavior

### Apply Operation

When the callback returns `TransactionOp::Apply(obj)`:
1. The store persists the provided object
2. The store bumps `resource_version`
3. The store updates `updated_at`
4. The store returns the persisted object

### Delete Operation

When the callback returns `TransactionOp::Delete`:
1. The store removes the object from storage
2. The store returns the deleted object

### Abort Operation

When the callback returns `TransactionOp::Abort(err)`:
1. The store does not modify the object
2. The store returns the provided error

### NoOp Operation

When the callback returns `TransactionOp::NoOp(val)`:
1. The store does not modify the object
2. The store returns the provided value

## Examples

### Update with Validation

```rust
store.transaction(key, name, |existing| {
    if existing.system.deletion_timestamp.is_some() {
        return TransactionOp::Abort(AppError::InvalidOperation(
            "cannot update terminating object".into()
        ));
    }
    
    let mut updated = new_obj.clone();
    if updated.spec.value != existing.spec.value {
        updated.system.generation = existing.system.generation + 1;
    }
    
    TransactionOp::Apply(updated)
}).await
```

### Conditional Delete

```rust
store.transaction(key, name, |existing| {
    if existing.metadata.finalizers.is_empty() {
        TransactionOp::Delete
    } else {
        let mut updated = existing.clone();
        updated.system.deletion_timestamp = Some(Utc::now());
        TransactionOp::Apply(updated)
    }
}).await
```

### Read-Only Check

```rust
let exists = store.transaction(key, name, |_| {
    TransactionOp::NoOp(true)
}).await?;
```

## Constraints

### C1: Single Object Scope

The transaction method operates on a single object identified by `(key, name)`. It does not support multi-object transactions.

### C2: No Nested Transactions

The transaction method does not support nested transactions. A callback cannot call `transaction()` again.

### C3: No Cross-Store Transactions

The transaction method does not support transactions across multiple stores.

## Performance Characteristics

### InMemoryStore

- **Lock granularity:** Per-object (DashMap entry)
- **Concurrency:** Multiple transactions on different objects can proceed in parallel
- **Blocking:** Slow callbacks block other operations on the same object

### SQLiteStore

- **Lock granularity:** Global (connection mutex)
- **Concurrency:** Only one transaction at a time across all objects
- **Blocking:** Slow callbacks block all operations on the store

## Testing Requirements

### T1: Atomicity Test

Verify that concurrent transactions on the same object serialize correctly.

### T2: Callback Validation Test

Verify that the callback receives the correct existing object.

### T3: Apply Test

Verify that `TransactionOp::Apply` persists the object and bumps `resource_version`.

### T4: Delete Test

Verify that `TransactionOp::Delete` removes the object.

### T5: Abort Test

Verify that `TransactionOp::Abort` returns the error without modifying the object.

### T6: NoOp Test

Verify that `TransactionOp::NoOp` returns the value without modifying the object.

### T7: NotFound Test

Verify that the store returns `NotFound` for non-existent objects.

### T8: Panic Recovery Test

Verify that a panicking callback releases the lock and leaves the object unchanged.
