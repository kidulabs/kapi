pub mod memory;

use async_trait::async_trait;
use serde_json::Value;

use crate::error::AppError;
use crate::object::types::{ListOptions, ListResponse, StoredObject};

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
        name: &str,
        data: Value,
    ) -> Result<StoredObject, AppError>;
    async fn get(&self, key: &ResourceKey, name: &str) -> Result<StoredObject, AppError>;
    async fn list(&self, key: &ResourceKey, opts: ListOptions) -> Result<ListResponse, AppError>;
    async fn update(&self, object: StoredObject) -> Result<StoredObject, AppError>;
    async fn delete(&self, key: &ResourceKey, name: &str) -> Result<StoredObject, AppError>;
}
