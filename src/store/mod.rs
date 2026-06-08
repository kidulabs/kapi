pub mod memory;
pub mod sqlite;

use async_trait::async_trait;
use serde_json::Value;

use crate::error::AppError;
use crate::object::types::{ListOptions, ListResponse, ObjectMeta, StoredObject};

#[derive(Debug, Clone, Hash, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ResourceKey {
    pub group: String,
    pub version: String,
    pub kind: String,
}

/// Result of a transaction callback, indicating what action the store should take.
///
/// The store holds an exclusive lock while the callback executes (see
/// [`ObjectStore::transaction()`]). The callback MUST be fast and non-blocking.
#[derive(Debug)]
pub enum TransactionOp {
    /// Persist the provided object, replacing the existing one.
    /// The store will bump `resource_version` and `updated_at` automatically.
    Apply(StoredObject),

    /// Hard-delete the object from storage.
    Delete,

    /// Reject the operation with the provided error.
    /// No changes are made to storage.
    Abort(AppError),
}

/// Pluggable object storage trait.
#[async_trait]
pub trait ObjectStore: Send + Sync {
    async fn create(
        &self,
        key: &ResourceKey,
        meta: ObjectMeta,
        spec: Value,
    ) -> Result<StoredObject, AppError>;
    async fn get(&self, key: &ResourceKey, name: &str) -> Result<StoredObject, AppError>;
    /// Lists objects for a resource key with optional filtering and pagination.
    ///
    /// The `ListOptions` may include `field_selector` and `label_selector` for
    /// store-level filtering. Filtering is applied before pagination to ensure
    /// correct page sizes.
    async fn list(&self, key: &ResourceKey, opts: ListOptions) -> Result<ListResponse, AppError>;
    /// Atomic read-modify-write transaction.
    ///
    /// Reads the existing object identified by `(key, name)`, passes it to the
    /// callback `op`, then applies the [`TransactionOp`] returned by the callback.
    ///
    /// # Callback Requirements
    ///
    /// The callback `op` **MUST be fast and non-blocking**:
    ///
    /// -  **Allowed:** Field validation, finalizer checks, business logic
    /// -  **Forbidden:** I/O operations, network calls, database queries
    ///
    /// The store holds an exclusive lock on the object while the callback
    /// executes. Slow callbacks will block all other operations on the same
    /// object.
    ///
    /// # Errors
    ///
    /// Returns [`AppError::NotFound`] if the object does not exist (before
    /// calling the callback).
    fn transaction(
        &self,
        key: &ResourceKey,
        name: &str,
        op: Box<dyn FnOnce(&StoredObject) -> TransactionOp + Send>,
    ) -> Result<StoredObject, AppError>;
    async fn exists(&self, key: &ResourceKey) -> Result<bool, AppError>;
}
