## Purpose

The `transaction()` method provides atomic read-modify-write semantics for object mutations. It replaces the previous `update()`, `delete()`, and `update_status()` methods with a single, composable API on the `ObjectStore` trait.

## Requirements

### Requirement: Atomic read-modify-write semantics

The transaction method SHALL provide atomic read-modify-write semantics. No other operations on the same object SHALL interleave between reading the existing object and applying the result.

#### Scenario: Concurrent transactions serialize on the same object
- **WHEN** two concurrent transactions are issued on the same object
- **THEN** they SHALL serialize — one completes before the other begins

#### Scenario: No race conditions between read and write
- **WHEN** a transaction reads an object, applies a callback, and writes the result
- **THEN** no other operation SHALL modify the object between the read and write

### Requirement: Callback-based API

The transaction method SHALL accept a callback that receives a reference to the existing object and returns a `TransactionOp` indicating the desired action.

#### Scenario: Callback receives existing object
- **WHEN** `transaction()` is called for an existing object
- **THEN** the callback SHALL receive a reference to the stored object

#### Scenario: Callback returns TransactionOp
- **WHEN** the callback executes
- **THEN** its return value of `TransactionOp<T>` SHALL determine the store's action

### Requirement: TransactionOp enum variants

The `TransactionOp` enum SHALL define four variants: `Apply(StoredObject)`, `Delete`, `Abort(AppError)`, and `NoOp(T)`.

#### Scenario: Apply persists and bumps version
- **WHEN** the callback returns `TransactionOp::Apply(obj)`
- **THEN** the store SHALL persist the provided object, bump `resource_version`, update `updated_at`, and return `Ok(obj)`

#### Scenario: Delete removes the object
- **WHEN** the callback returns `TransactionOp::Delete`
- **THEN** the store SHALL hard-delete the object and return the deleted object

#### Scenario: Abort returns error without changes
- **WHEN** the callback returns `TransactionOp::Abort(err)`
- **THEN** the store SHALL NOT modify the object and SHALL return `Err(err)`

#### Scenario: NoOp returns value without changes
- **WHEN** the callback returns `TransactionOp::NoOp(val)`
- **THEN** the store SHALL NOT modify the object and SHALL return `Ok(val)`

### Requirement: Fast callback requirement

The callback MUST be fast and non-blocking. It MUST NOT perform I/O operations, network calls, or database queries. The store holds an exclusive lock on the object while the callback executes.

#### Scenario: Allowed operations in callback
- **WHEN** the callback performs field validation, business logic checks, finalizer checks, or object mutation (cloning and modifying fields)
- **THEN** these operations are permitted

#### Scenario: Forbidden operations in callback
- **WHEN** the callback performs HTTP requests, database queries, file I/O, or blocking operations
- **THEN** these MAY block all other operations on the same object

### Requirement: Automatic resource version bumping

The store SHALL automatically bump `resource_version` and update `updated_at` when `TransactionOp::Apply` is returned.

#### Scenario: Version bumps on Apply
- **WHEN** `TransactionOp::Apply(obj)` is returned
- **THEN** the stored object's `resource_version` SHALL be incremented and `updated_at` SHALL be set to the current time

#### Scenario: No version bump on other ops
- **WHEN** `TransactionOp::Delete`, `Abort`, or `NoOp` is returned
- **THEN** `resource_version` and `updated_at` SHALL NOT be modified

### Requirement: NotFound handling for non-existent objects

If the object does not exist, the store SHALL return `AppError::NotFound` before calling the callback.

#### Scenario: Transaction on non-existent object returns NotFound
- **WHEN** `transaction()` is called for a key/name pair that does not exist
- **THEN** the error SHALL be `AppError::NotFound` and the callback SHALL NOT be invoked

### Requirement: Lock release on panic

If the callback panics, the store SHALL release the lock and leave the object unchanged.

#### Scenario: Panicking callback does not corrupt state
- **WHEN** the callback panics during a transaction
- **THEN** the object SHALL remain unchanged and the lock SHALL be released

### Requirement: Single object scope

The transaction method operates on a single object identified by `(key, name)`. It does not support multi-object transactions.

#### Scenario: Transaction scoped to single object
- **WHEN** `transaction(key, name, callback)` is called
- **THEN** only the object identified by `(key, name)` is read or modified

#### Scenario: No nested transactions
- **WHEN** a callback attempts to call `transaction()` again
- **THEN** the behavior is undefined (the trait does not support nesting)

### Requirement: InMemoryStore per-key locking

`InMemoryStore` SHALL implement `transaction()` using DashMap's per-key locking. Multiple transactions on different objects SHALL proceed in parallel.

#### Scenario: InMemoryStore parallel transactions on different objects
- **WHEN** concurrent transactions target different objects
- **THEN** both SHALL proceed in parallel without blocking

#### Scenario: InMemoryStore serializes transactions on same object
- **WHEN** concurrent transactions target the same object
- **THEN** they SHALL serialize via DashMap's entry lock

### Requirement: SQLiteStore connection-level locking

`SQLiteStore` SHALL implement `transaction()` using a `tokio::sync::Mutex` on the connection. Only one transaction SHALL execute at a time across all objects.

#### Scenario: SQLiteStore serializes all transactions
- **WHEN** concurrent transactions target any objects in the SQLiteStore
- **THEN** they SHALL serialize via the connection mutex
