# Finalizer Support — Technical Design

## Data Model Changes

### `ObjectMeta` (`src/object/types.rs`)

Add `finalizers` field:

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectMeta {
    pub name: String,
    #[serde(default)]
    pub labels: HashMap<String, String>,
    #[serde(default)]
    pub annotations: HashMap<String, String>,
    #[serde(default)]
    pub finalizers: Vec<String>,  // NEW
}
```

**Notes**:
- `#[serde(default)]` ensures backward compatibility with existing stored objects
- Serialized as `finalizers` in JSON (camelCase)
- Empty vec by default

### `SystemMetadata` (`src/object/types.rs`)

Add `deletion_timestamp` field:

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemMetadata {
    pub resource_version: u64,
    #[serde(default)]
    pub generation: u64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deletion_timestamp: Option<DateTime<Utc>>,  // NEW
}
```

**Notes**:
- Server-managed, never set by client
- `#[serde(skip_serializing_if = "Option::is_none")]` omits field from JSON when not set
- `#[serde(default)]` ensures backward compatibility (defaults to `None`)
- Serialized as `deletionTimestamp` in JSON (camelCase)

## DELETE Path Implementation

### Current Implementation (`src/object/service.rs:120-131`)

```rust
pub async fn delete(&self, key: ResourceKey, name: String) -> Result<StoredObject, AppError> {
    if key.kind == SCHEMA_KIND {
        self.delete_schema(key, name).await
    } else {
        let deleted =
            self.store.transaction(&key, &name, Box::new(|_existing| TransactionOp::Delete))?;
        self.publish_event(&key, WatchEventType::Deleted, &deleted);
        Ok(deleted)
    }
}
```

### New Implementation

```rust
pub async fn delete(&self, key: ResourceKey, name: String) -> Result<StoredObject, AppError> {
    if key.kind == SCHEMA_KIND {
        self.delete_schema(key, name).await
    } else {
        let result = self.store.transaction(
            &key,
            &name,
            Box::new(|existing| {
                if existing.metadata.finalizers.is_empty() {
                    // No finalizers: hard delete
                    TransactionOp::Delete
                } else if existing.system.deletion_timestamp.is_some() {
                    // Already marked for deletion: idempotent no-op
                    // Return existing object unchanged
                    TransactionOp::Apply(existing.clone())
                } else {
                    // Mark for deletion
                    let mut marked = existing.clone();
                    marked.system.deletion_timestamp = Some(Utc::now());
                    TransactionOp::Apply(marked)
                }
            }),
        )?;

        // Determine what happened
        let was_deleting = result.system.deletion_timestamp.is_some();
        let is_deleting = result.system.deletion_timestamp.is_some();
        let was_deleted = !was_deleting && !is_deleting;  // TransactionOp::Delete case

        if was_deleted {
            // Hard delete: publish Deleted event
            self.publish_event(&key, WatchEventType::Deleted, &result);
        } else if !was_deleting && is_deleting {
            // Newly marked for deletion: publish Modified event
            self.publish_event(&key, WatchEventType::Modified, &result);
        }
        // else: idempotent retry, no event

        Ok(result)
    }
}
```

**Logic**:
1. If `finalizers` is empty → `TransactionOp::Delete` (hard delete)
2. If `deletion_timestamp` is already set → `TransactionOp::Apply(existing)` (idempotent no-op)
3. Otherwise → `TransactionOp::Apply(marked)` with `deletion_timestamp` set

**Event logic**:
- Hard delete → publish `Deleted`
- Newly marked for deletion → publish `Modified`
- Idempotent retry → no event (suppress)

**Note**: The `was_deleted` check is tricky because `TransactionOp::Delete` returns the pre-deletion object. We need to track whether the transaction returned `Delete` or `Apply`. One approach: check if the returned object still exists in the store. Better approach: modify the transaction callback to return a flag, or check the store after the transaction.

**Simpler approach**: After the transaction, try to `get()` the object. If it's gone, it was a hard delete. If it exists, check if `deletion_timestamp` changed.

```rust
let result = self.store.transaction(...)?;

// Check if object still exists
match self.store.get(&key, &name).await {
    Ok(current) => {
        // Object exists: check if we just marked it for deletion
        if current.system.deletion_timestamp.is_some() {
            // Check if this is a new marking (compare with result)
            // Actually, result is the object returned by the transaction
            // If result.deletion_timestamp is Some and current.deletion_timestamp is Some,
            // we need to know if it was already set before
            // This is getting complicated...
        }
    }
    Err(AppError::NotFound { .. }) => {
        // Object was deleted
        self.publish_event(&key, WatchEventType::Deleted, &result);
    }
    Err(e) => return Err(e),
}
```

**Even simpler**: Track the state before and after in the transaction callback itself. Return a tuple `(StoredObject, DeleteAction)` where `DeleteAction` is an enum: `HardDeleted`, `MarkedForDeletion`, `IdempotentNoOp`.

But `TransactionOp` doesn't support returning extra data. We'd need to modify the store trait.

**Pragmatic approach**: Use a closure that captures a mutable flag:

```rust
let mut action = DeleteAction::Unknown;
let result = self.store.transaction(
    &key,
    &name,
    Box::new(|existing| {
        if existing.metadata.finalizers.is_empty() {
            action = DeleteAction::HardDeleted;
            TransactionOp::Delete
        } else if existing.system.deletion_timestamp.is_some() {
            action = DeleteAction::IdempotentNoOp;
            TransactionOp::Apply(existing.clone())
        } else {
            action = DeleteAction::MarkedForDeletion;
            let mut marked = existing.clone();
            marked.system.deletion_timestamp = Some(Utc::now());
            TransactionOp::Apply(marked)
        }
    }),
)?;

match action {
    DeleteAction::HardDeleted => {
        self.publish_event(&key, WatchEventType::Deleted, &result);
    }
    DeleteAction::MarkedForDeletion => {
        self.publish_event(&key, WatchEventType::Modified, &result);
    }
    DeleteAction::IdempotentNoOp => {
        // No event
    }
    DeleteAction::Unknown => unreachable!(),
}
```

This is clean and doesn't require store trait changes.

## UPDATE Path Implementation

### Current Implementation (`src/object/service.rs:356-395`)

The `validate_and_update_object` method uses a transaction callback that:
1. Checks OCC (resource_version match)
2. Applies metadata and spec changes via `apply_with_metadata`
3. Returns `TransactionOp::Apply(updated)`

### New Implementation

Add finalizer enforcement logic:

```rust
async fn validate_and_update_object(
    &self,
    object: StoredObject,
    spec: Value,
) -> Result<StoredObject, AppError> {
    validate_labels(&object.metadata.labels)?;
    validate_annotations(&object.metadata.annotations)?;
    validate_finalizers(&object.metadata.finalizers)?;  // NEW
    let validator = self.schema_registry.get_validator(&object.key).await?;

    if !validator.is_valid(&spec) {
        let errors = Self::map_validation_errors(validator.validate(&spec));
        return Err(AppError::SchemaValidation(errors));
    }

    let key = object.key.clone();
    let name = object.metadata.name.clone();
    let incoming_rv = object.system.resource_version;
    let incoming_metadata = object.metadata.clone();
    let incoming_spec = object.spec.clone();

    let updated = self.store.transaction(
        &key,
        &name,
        Box::new(move |existing| {
            // OCC check
            if incoming_rv != existing.system.resource_version {
                return TransactionOp::Abort(AppError::Conflict {
                    expected: existing.system.resource_version,
                    actual: incoming_rv,
                });
            }

            // NEW: If object is being deleted, only allow finalizer changes
            if existing.system.deletion_timestamp.is_some() {
                if !Self::only_finalizers_changed(&existing.metadata, &incoming_metadata) {
                    return TransactionOp::Abort(AppError::ObjectBeingDeleted {
                        name: existing.metadata.name.clone(),
                    });
                }
            }

            // Apply changes
            Self::apply_with_metadata(existing, |_existing| {
                let mut updated = existing.clone();
                updated.metadata = incoming_metadata.clone();
                updated.spec = incoming_spec.clone();
                updated
            })
        }),
    )?;

    // NEW: Check if this update should trigger hard delete
    // (finalizers became empty on a deleting object)
    if updated.system.deletion_timestamp.is_some() && updated.metadata.finalizers.is_empty() {
        // Hard delete via second transaction
        let deleted = self.store.transaction(
            &key,
            &name,
            Box::new(|_existing| TransactionOp::Delete),
        )?;
        self.publish_event(&updated.key, WatchEventType::Deleted, &deleted);
        return Ok(deleted);
    }

    self.publish_event(&updated.key, WatchEventType::Modified, &updated);
    Ok(updated)
}

// NEW helper
fn only_finalizers_changed(existing: &ObjectMeta, incoming: &ObjectMeta) -> bool {
    existing.name == incoming.name
        && existing.labels == incoming.labels
        && existing.annotations == incoming.annotations
        && existing.finalizers != incoming.finalizers
}
```

**Issue**: The two-transaction approach (update then delete) has a race condition. Between the two transactions, another client could modify the object.

**Better approach**: Return `TransactionOp::Delete` from the update callback when finalizers become empty:

```rust
Box::new(move |existing| {
    // OCC check
    if incoming_rv != existing.system.resource_version {
        return TransactionOp::Abort(AppError::Conflict { ... });
    }

    // If object is being deleted, only allow finalizer changes
    if existing.system.deletion_timestamp.is_some() {
        if !Self::only_finalizers_changed(&existing.metadata, &incoming_metadata) {
            return TransactionOp::Abort(AppError::ObjectBeingDeleted { ... });
        }
    }

    // Apply changes
    let mut updated = incoming_metadata.clone();
    updated.spec = incoming_spec.clone();
    
    // Check if this should trigger hard delete
    if existing.system.deletion_timestamp.is_some() && updated.finalizers.is_empty() {
        return TransactionOp::Delete;
    }

    // Otherwise, apply with metadata management
    Self::apply_with_metadata(existing, |_| {
        let mut new_obj = existing.clone();
        new_obj.metadata = updated;
        new_obj
    })
})
```

Wait, this doesn't work because `apply_with_metadata` returns `TransactionOp::Apply`, but we need to return `Delete` or `Apply` from the callback.

**Correct approach**: Don't use `apply_with_metadata` for the delete case. Handle it separately:

```rust
Box::new(move |existing| {
    // OCC check
    if incoming_rv != existing.system.resource_version {
        return TransactionOp::Abort(AppError::Conflict { ... });
    }

    // If object is being deleted, only allow finalizer changes
    if existing.system.deletion_timestamp.is_some() {
        if !Self::only_finalizers_changed(&existing.metadata, &incoming_metadata) {
            return TransactionOp::Abort(AppError::ObjectBeingDeleted { ... });
        }
    }

    // Build the updated object
    let mut new_obj = existing.clone();
    new_obj.metadata = incoming_metadata.clone();
    new_obj.spec = incoming_spec.clone();

    // Check if this should trigger hard delete
    if existing.system.deletion_timestamp.is_some() && new_obj.metadata.finalizers.is_empty() {
        return TransactionOp::Delete;
    }

    // Otherwise, apply metadata management
    new_obj.system.resource_version = existing.system.resource_version + 1;
    new_obj.system.updated_at = Utc::now();
    new_obj.system.created_at = existing.system.created_at;
    new_obj.system.deletion_timestamp = existing.system.deletion_timestamp;  // preserve
    if new_obj.spec != existing.spec {
        new_obj.system.generation = existing.system.generation + 1;
    } else {
        new_obj.system.generation = existing.system.generation;
    }

    TransactionOp::Apply(new_obj)
})
```

This is cleaner. The callback decides between `Delete` and `Apply` based on the finalizer state.

## Validation Function

### `src/validation/mod.rs`

Add `validate_finalizers`:

```rust
/// Validates a list of finalizers: max 20, each name must be label-key-shaped.
pub fn validate_finalizers(finalizers: &[String]) -> Result<(), AppError> {
    if finalizers.len() > 20 {
        return Err(AppError::InvalidFinalizer(format!(
            "too many finalizers: {} (max 20)",
            finalizers.len()
        )));
    }
    for finalizer in finalizers {
        validate_label_key(finalizer)?;
    }
    Ok(())
}
```

**Notes**:
- Reuses `validate_label_key` for name format
- Max 20 finalizers (defense-in-depth)
- Pure function, no side effects

## Error Variant

### `src/error.rs`

Add `ObjectBeingDeleted` variant:

```rust
#[derive(thiserror::Error, Debug)]
pub enum AppError {
    // ... existing variants ...

    // Object is being deleted; only finalizer modifications are allowed
    #[error("object '{name}' is being deleted; only finalizer modifications are allowed")]
    ObjectBeingDeleted { name: String },
}
```

Add HTTP mapping in `IntoResponse`:

```rust
AppError::ObjectBeingDeleted { name } => (
    StatusCode::CONFLICT,
    "ObjectBeingDeleted",
    format!("object '{name}' is being deleted; only finalizer modifications are allowed"),
    json!({ "name": name }),
),
```

**HTTP status**: 409 Conflict (the object is in a conflicting state for this operation).

## `apply_with_metadata` Preservation

### `src/object/service.rs:230-248`

Update `apply_with_metadata` to preserve `deletion_timestamp`:

```rust
fn apply_with_metadata<F>(existing: &StoredObject, mutator: F) -> TransactionOp
where
    F: FnOnce(&StoredObject) -> StoredObject,
{
    let mut new_obj = mutator(existing);
    // Bump resource_version on every mutation
    new_obj.system.resource_version = existing.system.resource_version + 1;
    // Update the timestamp
    new_obj.system.updated_at = Utc::now();
    // Preserve the original creation timestamp
    new_obj.system.created_at = existing.system.created_at;
    // NEW: Preserve deletion_timestamp (server-managed)
    new_obj.system.deletion_timestamp = existing.system.deletion_timestamp;
    // Bump generation only if spec changed
    if new_obj.spec != existing.spec {
        new_obj.system.generation = existing.system.generation + 1;
    } else {
        new_obj.system.generation = existing.system.generation;
    }
    TransactionOp::Apply(new_obj)
}
```

**Critical**: Never let the incoming object's `deletion_timestamp` through. This is server-managed.

## Event Semantics

### Mark for Deletion

When DELETE is called on an object with finalizers:
- `deletion_timestamp` is set
- `resource_version` bumps (it's an `Apply`)
- Publish `Modified` event
- Controllers watch for `system.deletionTimestamp` to detect deletion-in-progress

### Hard Delete

When finalizers become empty on a deleting object:
- Object is removed from storage
- Publish `Deleted` event with the pre-deletion object (including `deletion_timestamp`)

### Idempotent DELETE

When DELETE is called on an object that already has `deletion_timestamp` set:
- No state change
- No event published
- Return 200 with object

## Edge Cases and Concurrency

### 1. Two DELETEs in flight

- First DELETE marks for deletion (sets `deletion_timestamp`)
- Second DELETE sees `deletion_timestamp` already set → idempotent no-op
- Both return 200 with object
- Only the first publishes a `Modified` event

### 2. DELETE racing with controller removing finalizers

- DELETE sets `deletion_timestamp`
- Controller concurrently sets `finalizers: []`
- Transaction orders them:
  - If DELETE wins first: object is marked for deletion, controller's update sees `deletion_timestamp` and is allowed (only finalizers change), finalizers become empty → hard delete
  - If controller wins first: finalizers are empty, DELETE hard-deletes immediately
- Both outcomes are correct

### 3. CREATE same-name after DELETE-with-finalizers

- Object with `deletion_timestamp` set still exists in store
- CREATE with same name → `AlreadyExists` error
- This is correct: the object hasn't been fully deleted yet

### 4. Adding finalizer when `deletion_timestamp` is set

- Rejected with `ObjectBeingDeleted` error
- Rationale: once cleanup has started, a new finalizer means "I now claim this object" — the cleanup is already racing, so this is almost always a bug

### 5. Removing finalizer that doesn't exist

- Silently ignored (K8s behavior)
- The `only_finalizers_changed` check compares the full `finalizers` vec, so if the incoming vec is different (even if it's just removing a non-existent finalizer), it's allowed

## Testing Strategy

### Unit Tests

1. **Validation**: `validate_finalizers` with valid names, invalid names, too many finalizers
2. **DELETE path**: hard delete (no finalizers), mark for deletion (with finalizers), idempotent retry
3. **UPDATE path**: reject non-finalizer changes when deleting, allow finalizer changes, hard delete when finalizers empty
4. **Event suppression**: no event on idempotent DELETE
5. **Backward compatibility**: deserialize existing objects without `finalizers` field

### Integration Tests

1. **Full lifecycle**: create with finalizers → DELETE → verify `deletion_timestamp` → update finalizers → verify hard delete
2. **Concurrency**: two DELETEs in flight, DELETE racing with finalizer removal
3. **Error cases**: update spec when deleting, add finalizer when deleting
4. **Event verification**: check that correct events are published (or suppressed)

### Test Helpers

Add helper functions to `src/object/types.rs` tests:

```rust
pub(crate) fn test_stored_object_with_finalizers(
    key: ResourceKey,
    name: &str,
    spec: serde_json::Value,
    finalizers: Vec<String>,
) -> StoredObject {
    StoredObject {
        key,
        metadata: ObjectMeta {
            name: name.to_string(),
            labels: HashMap::new(),
            annotations: HashMap::new(),
            finalizers,
        },
        system: SystemMetadata::initial(),
        spec,
        status: None,
    }
}
```

## Migration and Backward Compatibility

### Existing Data

- SQLite objects don't have `finalizers` field
- `#[serde(default)]` on `finalizers` ensures deserialization defaults to empty vec
- `#[serde(default)]` on `deletion_timestamp` ensures deserialization defaults to `None`
- No migration needed

### API Compatibility

- New optional field in `ObjectMeta`: `finalizers`
- New optional field in `SystemMetadata`: `deletionTimestamp`
- Existing clients that don't send `finalizers` get empty vec (no change in behavior)
- Existing clients that don't expect `deletionTimestamp` ignore it (backward compatible)

## Documentation

### For Controller Authors

Add documentation to the API spec (OpenAPI) and user guide:

> **Finalizers** are strings in `metadata.finalizers` that register interest in an object's cleanup. When you DELETE an object with finalizers, the object is marked for deletion (`system.deletionTimestamp` is set) but not immediately removed. Your controller should watch for objects with `deletionTimestamp` set, perform cleanup, and then remove its finalizer from the list. When all finalizers are removed, the object is hard-deleted.
>
> **Constraints**:
> - Max 20 finalizers per object
> - Finalizer names must be label-key-shaped (e.g., `example.io/cleanup`)
> - Once `deletionTimestamp` is set, only `finalizers` can be modified
> - You cannot add finalizers to an object that is being deleted

### For kapi Developers

Add inline documentation to the code:

- `ObjectMeta.finalizers`: "Finalizers register interest in an object's cleanup. When non-empty, DELETE marks the object for deletion instead of hard-deleting it."
- `SystemMetadata.deletion_timestamp`: "Server-set timestamp indicating the object is being deleted. Only `finalizers` can be modified while this is set."
