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

#[async_trait]
pub trait ObjectStore: Send + Sync {
    async fn create(
        &self,
        key: &ResourceKey,
        meta: ObjectMeta,
        data: Value,
    ) -> Result<StoredObject, AppError>;
    async fn get(&self, key: &ResourceKey, name: &str) -> Result<StoredObject, AppError>;
    /// Lists objects for a resource key with optional filtering and pagination.
    ///
    /// The `ListOptions` may include `field_selector` and `label_selector` for
    /// store-level filtering. Filtering is applied before pagination to ensure
    /// correct page sizes.
    async fn list(&self, key: &ResourceKey, opts: ListOptions) -> Result<ListResponse, AppError>;
    async fn update(&self, object: StoredObject) -> Result<StoredObject, AppError>;
    async fn delete(&self, key: &ResourceKey, name: &str) -> Result<StoredObject, AppError>;
    async fn exists(&self, key: &ResourceKey) -> Result<bool, AppError>;
}
