pub mod memory;
pub mod sqlite;

use async_trait::async_trait;

use crate::error::AppError;
use crate::object::types::{ListOptions, ListResponse, StoredObject};

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
///
/// The store persists the object as-is when `Apply` is returned. It does NOT
/// modify system metadata — the caller (service layer) is responsible for
/// setting `resource_version`, `generation`, and timestamps before returning
/// `Apply`.
#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum TransactionOp {
    /// Persist the provided object, replacing the existing one.
    /// The store does NOT modify any system metadata fields. The caller is
    /// responsible for setting all metadata before returning Apply.
    Apply(StoredObject),

    /// Hard-delete the object from storage.
    Delete,

    /// Reject the operation with the provided error.
    /// No changes are made to storage.
    Abort(AppError),
}

/// Pluggable object storage trait.
///
/// Implementations persist objects as-is without modifying system metadata
/// (resource_version, generation, created_at, updated_at). The caller is
/// responsible for setting all system metadata before calling create() or
/// returning TransactionOp::Apply from a transaction callback.
#[async_trait]
pub trait ObjectStore: Send + Sync {
    /// Persist a complete StoredObject as-is.
    ///
    /// The implementation SHALL NOT modify any system metadata fields.
    /// If an object with the same key/name already exists, returns
    /// AppError::AlreadyExists.
    async fn create(&self, object: StoredObject) -> Result<StoredObject, AppError>;
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
