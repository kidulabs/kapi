# Transaction-Based Store Refactoring

## Summary

Replace the current `update()`, `delete()`, and `update_status()` methods in the `ObjectStore` trait with a single `transaction()` method that provides atomic read-modify-write semantics via a callback-based API.

## Motivation

### Current Problem: Race Windows

The current store interface requires a read-then-write pattern that creates race windows:

```rust
// Current pattern (has race window)
let existing = store.get(key, name).await?;
// ← RACE WINDOW: another update can happen here
validate(existing, new_obj)?;
store.update(key, name, new_obj).await?;
```

While the store's optimistic concurrency control (OCP) catches conflicts, the validation logic operates on potentially stale data.

### Current Problem: Business Logic in Store

The store currently implements some business logic (OCP checks, generation bumping). This violates the principle that the store should be a dumb persistence layer.

### Current Problem: Multiple Mutation Methods

Three separate mutation methods (`update`, `delete`, `update_status`) with overlapping concerns make the store trait complex and hard to extend.

## Design

### Core API

```rust
/// Transaction operation result.
pub enum TransactionOp<T> {
    /// Persist the provided object (replaces existing).
    Apply(StoredObject),
    
    /// Hard-delete the object.
    Delete,
    
    /// Reject the operation with the provided error.
    Abort(AppError),
    
    /// Do nothing, return the provided value.
    NoOp(T),
}

/// Atomic read-modify-write operation.
/// 
/// # Callback Requirements
/// 
/// The callback `op` **MUST be fast and non-blocking**:
/// 
/// - ✅ **Allowed:** Field validation, finalizer checks, business logic
/// - ❌ **Forbidden:** I/O operations, network calls, database queries
/// 
/// **Why:** The store holds an exclusive lock on the object while the callback
/// executes. Slow callbacks will block all other operations on the same object.
/// 
/// # Atomicity
/// 
/// The entire operation (read → callback → write) is atomic. No other
/// operations can interleave between reading the existing object and
/// applying the result.
pub async fn transaction<T>(
    &self,
    key: &ResourceKey,
    name: &str,
    op: impl FnOnce(&StoredObject) -> TransactionOp<T> + Send,
) -> Result<T, AppError>;
```

### What Stays

| Method | Reason |
|--------|--------|
| `create()` | No existing object to read — can't be a transaction |
| `get()` | Read-only, no mutation |
| `list()` | Read-only, no mutation |

### What Goes

| Method | Replaced By |
|--------|-------------|
| `update()` | `transaction(..., TransactionOp::Apply)` |
| `delete()` | `transaction(..., TransactionOp::Delete)` |
| `update_status()` | `transaction(..., TransactionOp::Apply)` with status-only mutation |

## Implementation

### InMemoryStore

```rust
async fn transaction<T>(
    &self,
    key: &ResourceKey,
    name: &str,
    op: impl FnOnce(&StoredObject) -> TransactionOp<T> + Send,
) -> Result<T, AppError> {
    let entry = (key.clone(), name.to_string());
    
    // Acquire exclusive lock on this specific object.
    // This lock is held for the entire transaction (read → callback → write).
    // 
    // IMPORTANT: The callback `op` MUST be fast (no I/O). If the callback
    // is slow, it will block all other operations on this object.
    let mut guard = self.objects.get_mut(&entry)
        .ok_or_else(|| AppError::NotFound {
            what: "object".to_string(),
            identifier: format!("{}/{}", key.kind, name),
        })?;
    
    let existing = guard.clone();
    
    match op(&existing) {
        TransactionOp::Apply(new_obj) => {
            *guard = new_obj.clone();
            guard.system.resource_version = self.next_version();
            Ok(new_obj)
        }
        TransactionOp::Delete => {
            let deleted = guard.clone();
            self.objects.remove(&entry);
            Ok(deleted)
        }
        TransactionOp::Abort(err) => Err(err),
        TransactionOp::NoOp(val) => Ok(val),
    }
}
```

### SQLiteStore

```rust
async fn transaction<T>(
    &self,
    key: &ResourceKey,
    name: &str,
    op: impl FnOnce(&StoredObject) -> TransactionOp<T> + Send,
) -> Result<T, AppError> {
    // Acquire exclusive lock on the connection.
    // This lock is held for the entire transaction (read → callback → write).
    // 
    // IMPORTANT: The callback `op` MUST be fast (no I/O). If the callback
    // is slow, it will block all other operations on this store.
    let conn = self.conn.lock().await;
    
    let existing = self.fetch_object_locked(&conn, key, name)?;
    
    match op(&existing) {
        TransactionOp::Apply(new_obj) => {
            self.persist_object_locked(&conn, &new_obj)?;
            Ok(new_obj)
        }
        TransactionOp::Delete => {
            self.delete_object_locked(&conn, key, name)?;
            Ok(existing)
        }
        TransactionOp::Abort(err) => Err(err),
        TransactionOp::NoOp(val) => Ok(val),
    }
}
```

**Note:** No `spawn_blocking` is needed because the callback is required to be fast (no I/O). The lock is held across the entire operation, ensuring atomicity.

## Service Layer Usage

### Update

```rust
async fn update(&self, key, name, new_obj) -> Result<StoredObject, AppError> {
    self.store.transaction(key, name, |existing| {
        let mut updated = new_obj.clone();
        
        // Business logic: bump generation only on spec change
        if updated.spec.value != existing.spec.value {
            updated.system.generation = existing.system.generation + 1;
        } else {
            updated.system.generation = existing.system.generation;
        }
        
        TransactionOp::Apply(updated)
    }).await
}
```

### Delete

```rust
async fn delete(&self, key, name) -> Result<StoredObject, AppError> {
    self.store.transaction(key, name, |existing| {
        if existing.metadata.finalizers.is_empty() {
            TransactionOp::Delete
        } else {
            let mut updated = existing.clone();
            updated.system.deletion_timestamp = Some(Utc::now());
            updated.system.resource_version = self.next_version();
            TransactionOp::Apply(updated)
        }
    }).await
}
```

### Update Status

```rust
async fn update_status(&self, key, name, new_status) -> Result<StoredObject, AppError> {
    self.store.transaction(key, name, |existing| {
        let mut updated = existing.clone();
        updated.status = Some(new_status);
        // Generation stays the same (status change doesn't bump)
        TransactionOp::Apply(updated)
    }).await
}
```

## Store Becomes Truly Dumb

The store no longer contains business logic like generation bumping. It just persists whatever object it's given:

```rust
// SQLiteStore::transaction() — no business logic
match op(&existing) {
    TransactionOp::Apply(new_obj) => {
        // Just persist whatever we're given
        self.persist_object_locked(&conn, &new_obj)?;
        Ok(new_obj)
    }
    // ...
}
```

All business rules (generation bumping, validation, finalizer checks) live in the service layer callbacks.

## Benefits

| Aspect | Before | After |
|--------|--------|-------|
| **Race window** | get → validate → update (gap) | Atomic read-validate-write |
| **Store methods** | 3 mutation methods | 1 mutation method |
| **Business logic** | Split between service + store | All in service callbacks |
| **Extensibility** | New method per operation | Same method, new callback |
| **Store complexity** | OCP checks, generation logic | Dumb persistence only |

## Migration

Since kapi is still in active development, we can replace the old methods entirely without a phased migration:

1. Add `transaction()` method to `ObjectStore` trait
2. Implement in `InMemoryStore` and `SQLiteStore`
3. Update `ObjectService` to use `transaction()` for all mutations
4. Remove `update()`, `delete()`, `update_status()` from trait and implementations
5. Update all callers (handlers, tests)

## Future Work

This refactoring enables future features that require atomic read-modify-write:

- **Finalizer support** — atomic check-and-delete when finalizers empty
- **Conditional updates** — update only if certain conditions are met
- **Multi-field atomic updates** — update spec + status + metadata in one operation

## Out of Scope

- Changes to `create()`, `get()`, `list()` methods
- Changes to the event bus or schema validation
- Finalizer implementation (separate change)
