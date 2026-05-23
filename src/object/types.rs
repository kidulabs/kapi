pub use crate::store::ResourceKey;

use chrono::{DateTime, Utc};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ContinueToken(pub String);

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ListOptions {
    pub limit: Option<usize>,
    pub continue_token: Option<ContinueToken>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ListResponse {
    pub items: Vec<StoredObject>,
    pub continue_token: Option<ContinueToken>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ValidationError {
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub enum WatchEventType {
    Added,
    Modified,
    Deleted,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WatchEvent {
    pub event_type: WatchEventType,
    pub object: StoredObject,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UserData {
    pub value: serde_json::Value,
}

// Schema data struct for type-safe access to Schema registration payloads
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaData {
    pub target_group: String,
    pub target_version: String,
    pub target_kind: String,
    pub json_schema: serde_json::Value,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectMeta {
    pub name: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemMetadata {
    pub resource_version: u64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StoredObject {
    pub key: ResourceKey,
    pub metadata: ObjectMeta,
    pub system: SystemMetadata,
    pub data: UserData,
}
