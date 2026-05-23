use std::sync::atomic::{AtomicU64, Ordering};

use async_trait::async_trait;
use base64::Engine;
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serde_json::Value;

use crate::error::AppError;
use crate::object::types::{
    ContinueToken, ListOptions, ListResponse, ObjectMetadata, StoredObject, UserData,
};
use crate::store::{ObjectStore, ResourceKey};

pub struct InMemoryStore {
    objects: DashMap<(ResourceKey, String), StoredObject>,
    next_version: AtomicU64,
}

impl Default for InMemoryStore {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryStore {
    pub fn new() -> Self {
        Self {
            objects: DashMap::new(),
            next_version: AtomicU64::new(1),
        }
    }

    fn next_version(&self) -> u64 {
        self.next_version.fetch_add(1, Ordering::Relaxed)
    }

    fn now() -> DateTime<Utc> {
        Utc::now()
    }
}

#[async_trait]
impl ObjectStore for InMemoryStore {
    async fn create(
        &self,
        key: &ResourceKey,
        name: &str,
        data: Value,
    ) -> Result<StoredObject, AppError> {
        let entry = (key.clone(), name.to_string());
        if self.objects.contains_key(&entry) {
            return Err(AppError::AlreadyExists {
                kind: key.kind.clone(),
                name: name.to_string(),
            });
        }

        let now = Self::now();
        let object = StoredObject {
            key: key.clone(),
            metadata: ObjectMetadata {
                name: name.to_string(),
                resource_version: self.next_version(),
                created_at: now,
                updated_at: now,
            },
            data: UserData { value: data },
        };

        self.objects.insert(entry, object.clone());
        Ok(object)
    }

    async fn get(&self, key: &ResourceKey, name: &str) -> Result<StoredObject, AppError> {
        let entry = (key.clone(), name.to_string());
        self.objects
            .get(&entry)
            .map(|r| r.clone())
            .ok_or_else(|| AppError::NotFound {
                what: "object".to_string(),
                identifier: format!("{}/{}", key.kind, name),
            })
    }

    async fn list(&self, key: &ResourceKey, opts: ListOptions) -> Result<ListResponse, AppError> {
        let mut items: Vec<StoredObject> = self
            .objects
            .iter()
            .filter(|r| r.key().0 == *key)
            .map(|r| r.clone())
            .collect();

        items.sort_by(|a, b| a.metadata.name.cmp(&b.metadata.name));

        let skip_past = opts
            .continue_token
            .as_ref()
            .map(decode_continue_token)
            .transpose()?;

        if let Some(ref skip) = skip_past {
            items.retain(|item| item.metadata.name > *skip);
        }

        let limit = opts.limit.unwrap_or(usize::MAX);
        let has_more = items.len() > limit;
        items.truncate(limit);

        let continue_token = if has_more {
            items
                .last()
                .map(|last| encode_continue_token(&last.metadata.name))
        } else {
            None
        };

        Ok(ListResponse {
            items,
            continue_token,
        })
    }

    async fn update(&self, object: StoredObject) -> Result<StoredObject, AppError> {
        let name = &object.metadata.name;
        let entry = (object.key.clone(), name.to_string());
        let mut guard = self
            .objects
            .get_mut(&entry)
            .ok_or_else(|| AppError::NotFound {
                what: "object".to_string(),
                identifier: format!("{}/{}", object.key.kind, name),
            })?;

        let expected = object.metadata.resource_version;
        if guard.metadata.resource_version != expected {
            return Err(AppError::Conflict {
                expected,
                actual: guard.metadata.resource_version,
            });
        }

        guard.data = object.data;
        guard.metadata.resource_version = self.next_version();
        guard.metadata.updated_at = Self::now();
        Ok(guard.clone())
    }

    async fn delete(&self, key: &ResourceKey, name: &str) -> Result<StoredObject, AppError> {
        let entry = (key.clone(), name.to_string());
        let (_, object) = self
            .objects
            .remove(&entry)
            .ok_or_else(|| AppError::NotFound {
                what: "object".to_string(),
                identifier: format!("{}/{}", key.kind, name),
            })?;

        Ok(object)
    }
}

fn decode_continue_token(token: &ContinueToken) -> Result<String, AppError> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(&token.0)
        .map_err(|_| AppError::Internal(anyhow::anyhow!("invalid continue token")))?;
    String::from_utf8(bytes)
        .map_err(|_| AppError::Internal(anyhow::anyhow!("invalid continue token")))
}

fn encode_continue_token(name: &str) -> ContinueToken {
    let encoded = base64::engine::general_purpose::STANDARD.encode(name);
    ContinueToken(encoded)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn test_key() -> ResourceKey {
        ResourceKey {
            group: "test.io".to_string(),
            version: "v1".to_string(),
            kind: "Widget".to_string(),
        }
    }

    #[tokio::test]
    async fn create_get_round_trip() {
        let store = InMemoryStore::new();
        let key = test_key();
        let data = json!({"color": "blue", "size": 10});

        let created = store.create(&key, "my-widget", data.clone()).await.unwrap();
        assert_eq!(created.metadata.name, "my-widget");
        assert_eq!(created.data.value, data);
        assert_eq!(created.key, key);
        assert_eq!(created.metadata.resource_version, 1);

        let retrieved = store.get(&key, "my-widget").await.unwrap();
        assert_eq!(retrieved.metadata.name, created.metadata.name);
        assert_eq!(retrieved.data.value, created.data.value);
        assert_eq!(
            retrieved.metadata.resource_version,
            created.metadata.resource_version
        );
    }

    #[tokio::test]
    async fn create_duplicate_conflict() {
        let store = InMemoryStore::new();
        let key = test_key();

        store
            .create(&key, "my-widget", json!({"x": 1}))
            .await
            .unwrap();

        let err = store
            .create(&key, "my-widget", json!({"x": 2}))
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::AlreadyExists { .. }));
    }

    #[tokio::test]
    async fn get_missing_not_found() {
        let store = InMemoryStore::new();
        let key = test_key();

        let err = store.get(&key, "nonexistent").await.unwrap_err();
        assert!(matches!(err, AppError::NotFound { .. }));
    }

    #[tokio::test]
    async fn list_sorted_by_name() {
        let store = InMemoryStore::new();
        let key = test_key();

        store.create(&key, "c", json!({})).await.unwrap();
        store.create(&key, "a", json!({})).await.unwrap();
        store.create(&key, "b", json!({})).await.unwrap();

        let res = store
            .list(
                &key,
                ListOptions {
                    limit: None,
                    continue_token: None,
                },
            )
            .await
            .unwrap();
        let names: Vec<&str> = res.items.iter().map(|o| o.metadata.name.as_str()).collect();
        assert_eq!(names, vec!["a", "b", "c"]);
        assert!(res.continue_token.is_none());
    }

    #[tokio::test]
    async fn list_with_limit_and_continue_token() {
        let store = InMemoryStore::new();
        let key = test_key();

        for i in 0..5 {
            store
                .create(&key, &format!("item-{i}"), json!({}))
                .await
                .unwrap();
        }

        let res = store
            .list(
                &key,
                ListOptions {
                    limit: Some(2),
                    continue_token: None,
                },
            )
            .await
            .unwrap();
        assert_eq!(res.items.len(), 2);
        assert_eq!(res.items[0].metadata.name, "item-0");
        assert_eq!(res.items[1].metadata.name, "item-1");
        assert!(res.continue_token.is_some());
    }

    #[tokio::test]
    async fn list_continue_token_resumes() {
        let store = InMemoryStore::new();
        let key = test_key();

        for i in 0..5 {
            store
                .create(&key, &format!("item-{i}"), json!({}))
                .await
                .unwrap();
        }

        let first = store
            .list(
                &key,
                ListOptions {
                    limit: Some(2),
                    continue_token: None,
                },
            )
            .await
            .unwrap();
        let token = first.continue_token.unwrap();

        let second = store
            .list(
                &key,
                ListOptions {
                    limit: Some(2),
                    continue_token: Some(token),
                },
            )
            .await
            .unwrap();
        assert_eq!(second.items.len(), 2);
        assert_eq!(second.items[0].metadata.name, "item-2");
        assert_eq!(second.items[1].metadata.name, "item-3");
        assert!(second.continue_token.is_some());
    }

    #[tokio::test]
    async fn update_correct_version_succeeds() {
        let store = InMemoryStore::new();
        let key = test_key();

        let created = store
            .create(&key, "my-widget", json!({"x": 1}))
            .await
            .unwrap();
        let v1 = created.metadata.resource_version;

        let object = StoredObject {
            key: key.clone(),
            metadata: ObjectMetadata {
                name: "my-widget".to_string(),
                resource_version: v1,
                created_at: created.metadata.created_at,
                updated_at: created.metadata.updated_at,
            },
            data: UserData {
                value: json!({"x": 2}),
            },
        };

        let updated = store.update(object).await.unwrap();
        assert!(updated.metadata.resource_version > v1);
        assert_eq!(updated.data.value, json!({"x": 2}));
    }

    #[tokio::test]
    async fn update_wrong_version_conflict() {
        let store = InMemoryStore::new();
        let key = test_key();

        let created = store
            .create(&key, "my-widget", json!({"x": 1}))
            .await
            .unwrap();

        let object = StoredObject {
            key: key.clone(),
            metadata: ObjectMetadata {
                name: "my-widget".to_string(),
                resource_version: 99,
                created_at: created.metadata.created_at,
                updated_at: created.metadata.updated_at,
            },
            data: UserData {
                value: json!({"x": 2}),
            },
        };

        let err = store.update(object).await.unwrap_err();
        assert!(matches!(
            err,
            AppError::Conflict {
                expected: 99,
                actual: 1
            }
        ));
    }

    #[tokio::test]
    async fn update_missing_not_found() {
        let store = InMemoryStore::new();
        let key = test_key();

        let object = StoredObject {
            key: key.clone(),
            metadata: ObjectMetadata {
                name: "nonexistent".to_string(),
                resource_version: 1,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            },
            data: UserData {
                value: json!({"x": 1}),
            },
        };

        let err = store.update(object).await.unwrap_err();
        assert!(matches!(err, AppError::NotFound { .. }));
    }

    #[tokio::test]
    async fn delete_returns_object_and_get_not_found() {
        let store = InMemoryStore::new();
        let key = test_key();

        let created = store
            .create(&key, "my-widget", json!({"x": 1}))
            .await
            .unwrap();

        let deleted = store.delete(&key, "my-widget").await.unwrap();
        assert_eq!(deleted.metadata.name, created.metadata.name);
        assert_eq!(deleted.data.value, created.data.value);

        let err = store.get(&key, "my-widget").await.unwrap_err();
        assert!(matches!(err, AppError::NotFound { .. }));
    }

    #[tokio::test]
    async fn delete_none_version_succeeds() {
        let store = InMemoryStore::new();
        let key = test_key();

        store
            .create(&key, "my-widget", json!({"x": 1}))
            .await
            .unwrap();
        store.create(&key, "other", json!({"x": 2})).await.unwrap();

        store.delete(&key, "my-widget").await.unwrap();

        let err = store.get(&key, "my-widget").await.unwrap_err();
        assert!(matches!(err, AppError::NotFound { .. }));

        let other = store.get(&key, "other").await.unwrap();
        assert_eq!(other.data.value, json!({"x": 2}));
    }

    #[tokio::test]
    async fn delete_missing_not_found() {
        let store = InMemoryStore::new();
        let key = test_key();

        let err = store.delete(&key, "nonexistent").await.unwrap_err();
        assert!(matches!(err, AppError::NotFound { .. }));
    }

    #[tokio::test]
    async fn list_empty_key() {
        let store = InMemoryStore::new();
        let key = test_key();

        let res = store
            .list(
                &key,
                ListOptions {
                    limit: None,
                    continue_token: None,
                },
            )
            .await
            .unwrap();
        assert!(res.items.is_empty());
        assert!(res.continue_token.is_none());
    }
}
