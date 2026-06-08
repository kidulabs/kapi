# Transaction Store Refactor — Design

## Overview

This document provides the detailed technical design for replacing the current `update()`, `delete()`, and `update_status()` methods with a single `transaction()` method that provides atomic read-modify-write semantics.

## Core Types

### TransactionOp Enum

```rust
/// Result of a transaction callback, indicating what action the store should take.
#[derive(Debug)]
pub enum TransactionOp<T> {
    /// Persist the provided object, replacing the existing one.
    /// The store will bump the resource_version automatically.
    Apply(StoredObject),
    
    /// Hard-delete the object from storage.
    Delete,
    
    /// Reject the operation with the provided error.
    /// No changes are made to storage.
    Abort(AppError),
    
    /// Do nothing, return the provided value.
    /// Useful for read-only checks or conditional logic.
    NoOp(T),
}
```

### Store Trait

```rust
#[async_trait]
pub trait ObjectStore: Send + Sync {
    // Existing methods (unchanged)
    async fn create(&self, object: StoredObject) -> Result<StoredObject, AppError>;
    async fn get(&self, key: &ResourceKey, name: &str) -> Result<StoredObject, AppError>;
    async fn list(&self, key: &ResourceKey, filter: Option<WatchFilter>, pagination: Pagination) -> Result<ObjectList, AppError>;
    
    // New method (replaces update, delete, update_status)
    async fn transaction<T>(
        &self,
        key: &ResourceKey,
        name: &str,
        op: impl FnOnce(&StoredObject) -> TransactionOp<T> + Send,
    ) -> Result<T, AppError>;
}
```

## Implementation Details

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
        TransactionOp::Apply(mut new_obj) => {
            // Store bumps resource_version automatically
            new_obj.system.resource_version = self.next_version();
            new_obj.system.updated_at = Self::now();
            *guard = new_obj.clone();
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

**Key Points:**
- Uses DashMap's per-key locking for fine-grained concurrency
- Store automatically bumps `resource_version` and `updated_at` on `Apply`
- Callback receives a clone of the existing object (safe to mutate)

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
    
    // Read existing object (blocking SQLite call, but we hold the lock)
    let existing = self.fetch_object_locked(&conn, key, name)?;
    
    // Execute callback (MUST be fast — no I/O allowed)
    match op(&existing) {
        TransactionOp::Apply(mut new_obj) => {
            // Store bumps resource_version automatically
            new_obj.system.resource_version = self.next_version();
            new_obj.system.updated_at = Self::now();
            
            // Persist the new object (blocking SQLite call)
            self.persist_object_locked(&conn, &new_obj)?;
            Ok(new_obj)
        }
        TransactionOp::Delete => {
            // Hard delete (blocking SQLite call)
            self.delete_object_locked(&conn, key, name)?;
            Ok(existing)
        }
        TransactionOp::Abort(err) => {
            // Return error without modifying anything
            Err(err)
        }
        TransactionOp::NoOp(val) => {
            // Return value without modifying anything
            Ok(val)
        }
    }
    // Lock is released here when `conn` goes out of scope
}
```

**Key Points:**
- Uses `tokio::sync::Mutex` for async-aware locking
- No `spawn_blocking` needed because callback is required to be fast
- Store automatically bumps `resource_version` and `updated_at` on `Apply`
- SQLite transaction ensures atomicity at the database level

### Helper Methods (SQLiteStore)

```rust
impl SQLiteStore {
    /// Fetch an object while holding the connection lock.
    fn fetch_object_locked(
        &self,
        conn: &MutexGuard<'_, Connection>,
        key: &ResourceKey,
        name: &str,
    ) -> Result<StoredObject, AppError> {
        // Implementation: SELECT query + deserialize
    }
    
    /// Persist an object while holding the connection lock.
    fn persist_object_locked(
        &self,
        conn: &MutexGuard<'_, Connection>,
        object: &StoredObject,
    ) -> Result<(), AppError> {
        // Implementation: INSERT OR REPLACE + labels
    }
    
    /// Delete an object while holding the connection lock.
    fn delete_object_locked(
        &self,
        conn: &MutexGuard<'_, Connection>,
        key: &ResourceKey,
        name: &str,
    ) -> Result<(), AppError> {
        // Implementation: DELETE + labels
    }
}
```

## Service Layer Integration

### Update

```rust
async fn update(&self, key: &ResourceKey, name: &str, new_obj: StoredObject) -> Result<StoredObject, AppError> {
    self.store.transaction(key, name, |existing| {
        let mut updated = new_obj.clone();
        
        // Business logic: bump generation only on spec change
        if updated.spec.value != existing.spec.value {
            updated.system.generation = existing.system.generation + 1;
        } else {
            updated.system.generation = existing.system.generation;
        }
        
        // Preserve system metadata
        updated.system.resource_version = existing.system.resource_version;
        updated.system.created_at = existing.system.created_at;
        
        TransactionOp::Apply(updated)
    }).await
}
```

### Delete

```rust
async fn delete(&self, key: &ResourceKey, name: &str) -> Result<StoredObject, AppError> {
    self.store.transaction(key, name, |existing| {
        if existing.metadata.finalizers.is_empty() {
            TransactionOp::Delete
        } else {
            let mut updated = existing.clone();
            updated.system.deletion_timestamp = Some(Utc::now());
            TransactionOp::Apply(updated)
        }
    }).await
}
```

### Update Status

```rust
async fn update_status(&self, key: &ResourceKey, name: &str, new_status: SpecData) -> Result<StoredObject, AppError> {
    self.store.transaction(key, name, |existing| {
        let mut updated = existing.clone();
        updated.status = Some(new_status);
        // Generation stays the same (status change doesn't bump)
        TransactionOp::Apply(updated)
    }).await
}
```

## Error Handling

### NotFound

If the object doesn't exist, the store returns `AppError::NotFound` before calling the callback.

### Conflict

The current OCP (optimistic concurrency control) is removed. The transaction method provides atomicity through locking, not version checking.

### Abort

The callback can return `TransactionOp::Abort(err)` to reject the operation with a specific error. This is useful for validation failures.

## Edge Cases

### Concurrent Transactions

**InMemoryStore:** DashMap provides per-key locking. Two concurrent transactions on the same object will serialize.

**SQLiteStore:** The connection mutex ensures only one transaction at a time. Two concurrent transactions will serialize.

### Callback Panics

If the callback panics, the lock is released (via `Drop`), but the transaction is not committed. The object remains unchanged.

### Slow Callbacks

If the callback is slow (violating the "fast callback" requirement), it will block other operations on the same object. This is documented in the trait method's doc comment.

## Testing Strategy

### Unit Tests

- Test each `TransactionOp` variant
- Test concurrent transactions
- Test callback validation logic
- Test error handling

### Integration Tests

- Test update flow (spec change, status change, metadata change)
- Test delete flow (with and without finalizers)
- Test concurrent updates
- Test race conditions

## Migration Checklist

- [ ] Add `TransactionOp` enum to `src/store/mod.rs`
- [ ] Add `transaction()` method to `ObjectStore` trait
- [ ] Implement `transaction()` in `InMemoryStore`
- [ ] Implement `transaction()` in `SQLiteStore`
- [ ] Add helper methods to `SQLiteStore` (`fetch_object_locked`, `persist_object_locked`, `delete_object_locked`)
- [ ] Update `ObjectService::update()` to use `transaction()`
- [ ] Update `ObjectService::delete()` to use `transaction()`
- [ ] Update `ObjectService::update_status()` to use `transaction()`
- [ ] Remove `update()` from `ObjectStore` trait and implementations
- [ ] Remove `delete()` from `ObjectStore` trait and implementations
- [ ] Remove `update_status()` from `ObjectStore` trait and implementations
- [ ] Update all tests to use new API
- [ ] Update documentation
