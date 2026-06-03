use std::sync::atomic::{AtomicU64, Ordering};

use async_trait::async_trait;
use base64::Engine;
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serde_json::Value;

use crate::error::AppError;
use crate::object::types::{
    ContinueToken, FieldSelector, ListOptions, ListResponse, ObjectMeta, StoredObject,
    SystemMetadata, SpecData,
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
        meta: ObjectMeta,
        spec: Value,
    ) -> Result<StoredObject, AppError> {
        let entry = (key.clone(), meta.name.clone());
        if self.objects.contains_key(&entry) {
            return Err(AppError::AlreadyExists {
                kind: key.kind.clone(),
                name: meta.name.clone(),
            });
        }

        let now = Self::now();
        let object = StoredObject {
            key: key.clone(),
            metadata: meta,
            system: SystemMetadata {
                resource_version: self.next_version(),
                created_at: now,
                updated_at: now,
            },
            spec: SpecData { value: spec },
            status: None,
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

        // Apply field_selector filter
        if let Some(ref selector) = opts.field_selector {
            items.retain(|obj| match selector {
                FieldSelector::NameEquals(name) => obj.metadata.name == *name,
            });
        }

        // Apply label_selector filter
        if let Some(ref selector) = opts.label_selector {
            items.retain(|obj| selector.matches(&obj.metadata.labels));
        }

        // Sort by name (after filtering, before pagination)
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

        let expected = object.system.resource_version;
        if guard.system.resource_version != expected {
            return Err(AppError::Conflict {
                expected,
                actual: guard.system.resource_version,
            });
        }

        guard.metadata.labels = object.metadata.labels;
        guard.spec = object.spec;
        guard.system.resource_version = self.next_version();
        guard.system.updated_at = Self::now();
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

    async fn exists(&self, key: &ResourceKey) -> Result<bool, AppError> {
        Ok(self.objects.iter().any(|r| r.key().0 == *key))
    }

    async fn update_status(
        &self,
        key: &ResourceKey,
        name: &str,
        status: Value,
    ) -> Result<StoredObject, AppError> {
        let entry = (key.clone(), name.to_string());
        let mut guard = self
            .objects
            .get_mut(&entry)
            .ok_or_else(|| AppError::NotFound {
                what: "object".to_string(),
                identifier: format!("{}/{}", key.kind, name),
            })?;

        guard.status = Some(SpecData { value: status });
        guard.system.resource_version = self.next_version();
        guard.system.updated_at = Self::now();
        Ok(guard.clone())
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
    use crate::object::types::LabelSelector;
    use serde_json::json;
    use std::collections::HashMap;

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

        let created = store
            .create(
                &key,
                ObjectMeta {
                    name: "my-widget".to_string(),
                    labels: HashMap::new(),
                },
                data.clone(),
            )
            .await
            .unwrap();
        assert_eq!(created.metadata.name, "my-widget");
        assert_eq!(created.spec.value, data);
        assert_eq!(created.key, key);
        assert_eq!(created.system.resource_version, 1);

        let retrieved = store.get(&key, "my-widget").await.unwrap();
        assert_eq!(retrieved.metadata.name, created.metadata.name);
        assert_eq!(retrieved.spec.value, created.spec.value);
        assert_eq!(
            retrieved.system.resource_version,
            created.system.resource_version
        );
    }

    #[tokio::test]
    async fn create_duplicate_conflict() {
        let store = InMemoryStore::new();
        let key = test_key();

        store
            .create(
                &key,
                ObjectMeta {
                    name: "my-widget".to_string(),
                    labels: HashMap::new(),
                },
                json!({"x": 1}),
            )
            .await
            .unwrap();

        let err = store
            .create(
                &key,
                ObjectMeta {
                    name: "my-widget".to_string(),
                    labels: HashMap::new(),
                },
                json!({"x": 2}),
            )
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

        store
            .create(
                &key,
                ObjectMeta {
                    name: "c".to_string(),
                    labels: HashMap::new(),
                },
                json!({}),
            )
            .await
            .unwrap();
        store
            .create(
                &key,
                ObjectMeta {
                    name: "a".to_string(),
                    labels: HashMap::new(),
                },
                json!({}),
            )
            .await
            .unwrap();
        store
            .create(
                &key,
                ObjectMeta {
                    name: "b".to_string(),
                    labels: HashMap::new(),
                },
                json!({}),
            )
            .await
            .unwrap();

        let res = store
            .list(
                &key,
                ListOptions {
                    limit: None,
                    continue_token: None,
                    ..Default::default()
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
                .create(
                    &key,
                    ObjectMeta {
                        name: format!("item-{i}"),
                        labels: HashMap::new(),
                    },
                    json!({}),
                )
                .await
                .unwrap();
        }

        let res = store
            .list(
                &key,
                ListOptions {
                    limit: Some(2),
                    continue_token: None,
                    ..Default::default()
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
                .create(
                    &key,
                    ObjectMeta {
                        name: format!("item-{i}"),
                        labels: HashMap::new(),
                    },
                    json!({}),
                )
                .await
                .unwrap();
        }

        let first = store
            .list(
                &key,
                ListOptions {
                    limit: Some(2),
                    continue_token: None,
                    ..Default::default()
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
                    ..Default::default()
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
            .create(
                &key,
                ObjectMeta {
                    name: "my-widget".to_string(),
                    labels: HashMap::new(),
                },
                json!({"x": 1}),
            )
            .await
            .unwrap();
        let v1 = created.system.resource_version;

        let object = StoredObject {
            key: key.clone(),
            metadata: ObjectMeta {
                name: "my-widget".to_string(),
                labels: HashMap::new(),
            },
            system: SystemMetadata {
                resource_version: v1,
                created_at: created.system.created_at,
                updated_at: created.system.updated_at,
            },
            spec: SpecData {
                value: json!({"x": 2}),
            },
            status: None,
        };

        let updated = store.update(object).await.unwrap();
        assert!(updated.system.resource_version > v1);
        assert_eq!(updated.spec.value, json!({"x": 2}));
    }

    #[tokio::test]
    async fn update_wrong_version_conflict() {
        let store = InMemoryStore::new();
        let key = test_key();

        let created = store
            .create(
                &key,
                ObjectMeta {
                    name: "my-widget".to_string(),
                    labels: HashMap::new(),
                },
                json!({"x": 1}),
            )
            .await
            .unwrap();

        let object = StoredObject {
            key: key.clone(),
            metadata: ObjectMeta {
                name: "my-widget".to_string(),
                labels: HashMap::new(),
            },
            system: SystemMetadata {
                resource_version: 99,
                created_at: created.system.created_at,
                updated_at: created.system.updated_at,
            },
            spec: SpecData {
                value: json!({"x": 2}),
            },
            status: None,
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
            metadata: ObjectMeta {
                name: "nonexistent".to_string(),
                labels: HashMap::new(),
            },
            system: SystemMetadata {
                resource_version: 1,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            },
            spec: SpecData {
                value: json!({"x": 1}),
            },
            status: None,
        };

        let err = store.update(object).await.unwrap_err();
        assert!(matches!(err, AppError::NotFound { .. }));
    }

    #[tokio::test]
    async fn delete_returns_object_and_get_not_found() {
        let store = InMemoryStore::new();
        let key = test_key();

        let created = store
            .create(
                &key,
                ObjectMeta {
                    name: "my-widget".to_string(),
                    labels: HashMap::new(),
                },
                json!({"x": 1}),
            )
            .await
            .unwrap();

        let deleted = store.delete(&key, "my-widget").await.unwrap();
        assert_eq!(deleted.metadata.name, created.metadata.name);
        assert_eq!(deleted.spec.value, created.spec.value);

        let err = store.get(&key, "my-widget").await.unwrap_err();
        assert!(matches!(err, AppError::NotFound { .. }));
    }

    #[tokio::test]
    async fn delete_none_version_succeeds() {
        let store = InMemoryStore::new();
        let key = test_key();

        store
            .create(
                &key,
                ObjectMeta {
                    name: "my-widget".to_string(),
                    labels: HashMap::new(),
                },
                json!({"x": 1}),
            )
            .await
            .unwrap();
        store
            .create(
                &key,
                ObjectMeta {
                    name: "other".to_string(),
                    labels: HashMap::new(),
                },
                json!({"x": 2}),
            )
            .await
            .unwrap();

        store.delete(&key, "my-widget").await.unwrap();

        let err = store.get(&key, "my-widget").await.unwrap_err();
        assert!(matches!(err, AppError::NotFound { .. }));

        let other = store.get(&key, "other").await.unwrap();
        assert_eq!(other.spec.value, json!({"x": 2}));
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
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert!(res.items.is_empty());
        assert!(res.continue_token.is_none());
    }

    // --- Filtering tests ---

    #[tokio::test]
    async fn list_with_field_selector() {
        let store = InMemoryStore::new();
        let key = test_key();

        store
            .create(
                &key,
                ObjectMeta {
                    name: "foo".to_string(),
                    labels: HashMap::new(),
                },
                json!({}),
            )
            .await
            .unwrap();
        store
            .create(
                &key,
                ObjectMeta {
                    name: "bar".to_string(),
                    labels: HashMap::new(),
                },
                json!({}),
            )
            .await
            .unwrap();
        store
            .create(
                &key,
                ObjectMeta {
                    name: "baz".to_string(),
                    labels: HashMap::new(),
                },
                json!({}),
            )
            .await
            .unwrap();

        let res = store
            .list(
                &key,
                ListOptions {
                    field_selector: Some(FieldSelector::NameEquals("foo".to_string())),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert_eq!(res.items.len(), 1);
        assert_eq!(res.items[0].metadata.name, "foo");
    }

    #[tokio::test]
    async fn list_with_label_selector() {
        let store = InMemoryStore::new();
        let key = test_key();

        let mut labels_nginx = HashMap::new();
        labels_nginx.insert("app".to_string(), "nginx".to_string());
        store
            .create(
                &key,
                ObjectMeta {
                    name: "web-1".to_string(),
                    labels: labels_nginx,
                },
                json!({}),
            )
            .await
            .unwrap();

        let mut labels_apache = HashMap::new();
        labels_apache.insert("app".to_string(), "apache".to_string());
        store
            .create(
                &key,
                ObjectMeta {
                    name: "web-2".to_string(),
                    labels: labels_apache,
                },
                json!({}),
            )
            .await
            .unwrap();

        store
            .create(
                &key,
                ObjectMeta {
                    name: "web-3".to_string(),
                    labels: HashMap::new(),
                },
                json!({}),
            )
            .await
            .unwrap();

        let res = store
            .list(
                &key,
                ListOptions {
                    label_selector: Some(LabelSelector {
                        requirements: vec![crate::object::types::LabelRequirement::Equals {
                            key: "app".to_string(),
                            value: "nginx".to_string(),
                        }],
                    }),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert_eq!(res.items.len(), 1);
        assert_eq!(res.items[0].metadata.name, "web-1");
    }

    #[tokio::test]
    async fn list_with_both_selectors() {
        let store = InMemoryStore::new();
        let key = test_key();

        let mut labels = HashMap::new();
        labels.insert("app".to_string(), "nginx".to_string());
        store
            .create(
                &key,
                ObjectMeta {
                    name: "target".to_string(),
                    labels,
                },
                json!({}),
            )
            .await
            .unwrap();

        let mut labels2 = HashMap::new();
        labels2.insert("app".to_string(), "nginx".to_string());
        store
            .create(
                &key,
                ObjectMeta {
                    name: "other".to_string(),
                    labels: labels2,
                },
                json!({}),
            )
            .await
            .unwrap();

        store
            .create(
                &key,
                ObjectMeta {
                    name: "target".to_string() + "-nolabel",
                    labels: HashMap::new(),
                },
                json!({}),
            )
            .await
            .unwrap();

        let res = store
            .list(
                &key,
                ListOptions {
                    field_selector: Some(FieldSelector::NameEquals("target".to_string())),
                    label_selector: Some(LabelSelector {
                        requirements: vec![crate::object::types::LabelRequirement::Equals {
                            key: "app".to_string(),
                            value: "nginx".to_string(),
                        }],
                    }),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert_eq!(res.items.len(), 1);
        assert_eq!(res.items[0].metadata.name, "target");
    }

    #[tokio::test]
    async fn list_filter_before_pagination() {
        let store = InMemoryStore::new();
        let key = test_key();

        // Create 50 objects, only 3 match the filter
        for i in 0..50 {
            let mut labels = HashMap::new();
            if i < 3 {
                labels.insert("app".to_string(), "nginx".to_string());
            }
            store
                .create(
                    &key,
                    ObjectMeta {
                        name: format!("obj-{i:02}"),
                        labels,
                    },
                    json!({}),
                )
                .await
                .unwrap();
        }

        // Filter to 3, limit 10 → should return 3 (not 10)
        let res = store
            .list(
                &key,
                ListOptions {
                    label_selector: Some(LabelSelector {
                        requirements: vec![crate::object::types::LabelRequirement::Equals {
                            key: "app".to_string(),
                            value: "nginx".to_string(),
                        }],
                    }),
                    limit: Some(10),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert_eq!(res.items.len(), 3);
        assert!(res.continue_token.is_none());
    }

    #[tokio::test]
    async fn list_filter_no_matches() {
        let store = InMemoryStore::new();
        let key = test_key();

        store
            .create(
                &key,
                ObjectMeta {
                    name: "foo".to_string(),
                    labels: HashMap::new(),
                },
                json!({}),
            )
            .await
            .unwrap();

        let res = store
            .list(
                &key,
                ListOptions {
                    field_selector: Some(FieldSelector::NameEquals("nonexistent".to_string())),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert!(res.items.is_empty());
        assert!(res.continue_token.is_none());
    }

    // --- exists tests ---

    #[tokio::test]
    async fn exists_returns_true_when_objects_present() {
        let store = InMemoryStore::new();
        let key = test_key();

        store
            .create(
                &key,
                ObjectMeta {
                    name: "exists-test".to_string(),
                    labels: HashMap::new(),
                },
                json!({"x": 1}),
            )
            .await
            .unwrap();

        assert!(store.exists(&key).await.unwrap());
    }

    #[tokio::test]
    async fn exists_returns_false_when_no_objects() {
        let store = InMemoryStore::new();
        let key = test_key();

        assert!(!store.exists(&key).await.unwrap());
    }

    #[tokio::test]
    async fn exists_returns_false_for_different_key() {
        let store = InMemoryStore::new();
        let key = test_key();

        store
            .create(
                &key,
                ObjectMeta {
                    name: "test".to_string(),
                    labels: HashMap::new(),
                },
                json!({}),
            )
            .await
            .unwrap();

        let other_key = ResourceKey {
            group: "other.io".to_string(),
            version: "v1".to_string(),
            kind: "Other".to_string(),
        };
        assert!(!store.exists(&other_key).await.unwrap());
    }

    // --- update_status tests ---

    #[tokio::test]
    async fn update_status_success() {
        let store = InMemoryStore::new();
        let key = test_key();

        let created = store
            .create(
                &key,
                ObjectMeta {
                    name: "my-widget".to_string(),
                    labels: HashMap::new(),
                },
                json!({"color": "blue"}),
            )
            .await
            .unwrap();
        let v1 = created.system.resource_version;
        assert!(created.status.is_none());

        let updated = store
            .update_status(&key, "my-widget", json!({"phase": "Running"}))
            .await
            .unwrap();
        assert!(updated.status.is_some());
        assert_eq!(updated.status.unwrap().value, json!({"phase": "Running"}));
        assert!(updated.system.resource_version > v1);
        // Spec should be unchanged
        assert_eq!(updated.spec.value, json!({"color": "blue"}));
    }

    #[tokio::test]
    async fn update_status_not_found() {
        let store = InMemoryStore::new();
        let key = test_key();

        let err = store
            .update_status(&key, "nonexistent", json!({"phase": "Running"}))
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::NotFound { .. }));
    }

    #[tokio::test]
    async fn update_status_replaces_existing_status() {
        let store = InMemoryStore::new();
        let key = test_key();

        store
            .create(
                &key,
                ObjectMeta {
                    name: "my-widget".to_string(),
                    labels: HashMap::new(),
                },
                json!({"color": "blue"}),
            )
            .await
            .unwrap();

        store
            .update_status(&key, "my-widget", json!({"phase": "Pending"}))
            .await
            .unwrap();

        let updated = store
            .update_status(&key, "my-widget", json!({"phase": "Running"}))
            .await
            .unwrap();
        assert_eq!(updated.status.unwrap().value, json!({"phase": "Running"}));
    }

    #[tokio::test]
    async fn update_status_bumps_resource_version() {
        let store = InMemoryStore::new();
        let key = test_key();

        let created = store
            .create(
                &key,
                ObjectMeta {
                    name: "my-widget".to_string(),
                    labels: HashMap::new(),
                },
                json!({}),
            )
            .await
            .unwrap();
        let v1 = created.system.resource_version;

        let updated = store
            .update_status(&key, "my-widget", json!({"phase": "Running"}))
            .await
            .unwrap();
        assert!(updated.system.resource_version > v1);
    }

    #[tokio::test]
    async fn update_status_preserves_spec() {
        let store = InMemoryStore::new();
        let key = test_key();

        store
            .create(
                &key,
                ObjectMeta {
                    name: "my-widget".to_string(),
                    labels: HashMap::new(),
                },
                json!({"color": "blue", "size": 10}),
            )
            .await
            .unwrap();

        let updated = store
            .update_status(&key, "my-widget", json!({"phase": "Running"}))
            .await
            .unwrap();
        assert_eq!(updated.spec.value, json!({"color": "blue", "size": 10}));
    }
}
