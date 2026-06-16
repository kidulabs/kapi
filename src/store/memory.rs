use async_trait::async_trait;
use base64::Engine;
use dashmap::DashMap;

use crate::error::AppError;
use crate::object::types::{ContinueToken, FieldSelector, ListOptions, ListResponse, StoredObject};
use crate::store::{ObjectStore, ResourceKey, TransactionOp};

pub struct InMemoryStore {
    objects: DashMap<(ResourceKey, String), StoredObject>,
}

impl Default for InMemoryStore {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryStore {
    pub fn new() -> Self {
        Self { objects: DashMap::new() }
    }
}

#[async_trait]
impl ObjectStore for InMemoryStore {
    async fn create(&self, object: StoredObject) -> Result<StoredObject, AppError> {
        let entry = (object.key.clone(), object.metadata.name.clone());
        if self.objects.contains_key(&entry) {
            return Err(AppError::AlreadyExists {
                kind: object.key.kind.clone(),
                name: object.metadata.name.clone(),
            });
        }

        self.objects.insert(entry, object.clone());
        Ok(object)
    }

    async fn get(&self, key: &ResourceKey, name: &str) -> Result<StoredObject, AppError> {
        let entry = (key.clone(), name.to_string());
        self.objects.get(&entry).map(|r| r.clone()).ok_or_else(|| AppError::NotFound {
            what: "object".to_string(),
            identifier: format!("{}/{}", key.kind, name),
        })
    }

    async fn list(&self, key: &ResourceKey, opts: ListOptions) -> Result<ListResponse, AppError> {
        let mut items: Vec<StoredObject> =
            self.objects.iter().filter(|r| r.key().0 == *key).map(|r| r.clone()).collect();

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

        let skip_past = opts.continue_token.as_ref().map(decode_continue_token).transpose()?;

        if let Some(ref skip) = skip_past {
            items.retain(|item| item.metadata.name > *skip);
        }

        let limit = opts.limit.unwrap_or(usize::MAX);
        let has_more = items.len() > limit;
        items.truncate(limit);

        let continue_token = if has_more {
            items.last().map(|last| encode_continue_token(&last.metadata.name))
        } else {
            None
        };

        Ok(ListResponse { items, continue_token })
    }

    fn transaction(
        &self,
        key: &ResourceKey,
        name: &str,
        op: Box<dyn FnOnce(&StoredObject) -> TransactionOp + Send>,
    ) -> Result<StoredObject, AppError> {
        let entry = (key.clone(), name.to_string());

        // Acquire exclusive lock on this specific object via DashMap's per-key locking.
        // The lock is held for the entire transaction (read → callback → write).
        let mut guard = self.objects.get_mut(&entry).ok_or_else(|| AppError::NotFound {
            what: "object".to_string(),
            identifier: format!("{}/{}", key.kind, name),
        })?;

        let existing = guard.clone();
        let txn_op = op(&existing);

        match txn_op {
            TransactionOp::Apply(new_obj) => {
                // Store persists the object as-is — no metadata modifications.
                // The caller (service layer) is responsible for setting all
                // system metadata before returning Apply.
                *guard = new_obj.clone();
                Ok(new_obj)
            }
            TransactionOp::Delete => {
                // Drop the guard before removing to avoid deadlock on the DashMap shard
                let deleted = guard.clone();
                drop(guard);
                self.objects.remove(&entry);
                Ok(deleted)
            }
            TransactionOp::Abort(err) => Err(err),
        }
    }

    async fn exists(&self, key: &ResourceKey) -> Result<bool, AppError> {
        Ok(self.objects.iter().any(|r| r.key().0 == *key))
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
    use crate::object::types::{LabelSelector, ObjectMeta, SystemMetadata};
    use serde_json::json;
    use std::collections::HashMap;

    /// Helper to construct a stored object with initial metadata for tests.
    fn test_obj(key: ResourceKey, name: &str, spec: serde_json::Value) -> StoredObject {
        StoredObject {
            key,
            metadata: ObjectMeta {
                name: name.to_string(),
                labels: HashMap::new(),
                annotations: HashMap::new(),
                finalizers: Vec::new(),
            },
            system: SystemMetadata::initial(),
            spec,
            status: None,
        }
    }

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

        let created = store.create(test_obj(key.clone(), "my-widget", data.clone())).await.unwrap();
        assert_eq!(created.metadata.name, "my-widget");
        assert_eq!(created.spec, data);
        assert_eq!(created.key, key);
        assert_eq!(created.system.resource_version, 1);

        let retrieved = store.get(&key, "my-widget").await.unwrap();
        assert_eq!(retrieved.metadata.name, created.metadata.name);
        assert_eq!(retrieved.spec, created.spec);
        assert_eq!(retrieved.system.resource_version, created.system.resource_version);
    }

    #[tokio::test]
    async fn create_duplicate_conflict() {
        let store = InMemoryStore::new();
        let key = test_key();

        store.create(test_obj(key.clone(), "my-widget", json!({"x": 1}))).await.unwrap();

        let err =
            store.create(test_obj(key.clone(), "my-widget", json!({"x": 2}))).await.unwrap_err();
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

        store.create(test_obj(key.clone(), "c", json!({}))).await.unwrap();
        store.create(test_obj(key.clone(), "a", json!({}))).await.unwrap();
        store.create(test_obj(key.clone(), "b", json!({}))).await.unwrap();

        let res = store
            .list(&key, ListOptions { limit: None, continue_token: None, ..Default::default() })
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
            store.create(test_obj(key.clone(), &format!("item-{i}"), json!({}))).await.unwrap();
        }

        let res = store
            .list(&key, ListOptions { limit: Some(2), continue_token: None, ..Default::default() })
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
            store.create(test_obj(key.clone(), &format!("item-{i}"), json!({}))).await.unwrap();
        }

        let first = store
            .list(&key, ListOptions { limit: Some(2), continue_token: None, ..Default::default() })
            .await
            .unwrap();
        let token = first.continue_token.unwrap();

        let second = store
            .list(
                &key,
                ListOptions { limit: Some(2), continue_token: Some(token), ..Default::default() },
            )
            .await
            .unwrap();
        assert_eq!(second.items.len(), 2);
        assert_eq!(second.items[0].metadata.name, "item-2");
        assert_eq!(second.items[1].metadata.name, "item-3");
        assert!(second.continue_token.is_some());
    }

    // --- Transaction-based tests (replacing old update/delete/update_status) ---

    #[tokio::test]
    async fn transaction_apply_persists_as_is() {
        let store = InMemoryStore::new();
        let key = test_key();

        let created = store.create(test_obj(key.clone(), "test", json!({"x": 1}))).await.unwrap();

        let result = store
            .transaction(
                &key,
                "test",
                Box::new(|existing| {
                    // Caller is responsible for setting metadata before Apply
                    let mut updated = existing.clone();
                    updated.spec = json!({"x": 2});
                    updated.system.resource_version = existing.system.resource_version + 1;
                    TransactionOp::Apply(updated)
                }),
            )
            .unwrap();

        // Store persists the object as-is — rv is exactly what the callback set
        assert_eq!(result.system.resource_version, created.system.resource_version + 1);
        assert_eq!(result.spec, json!({"x": 2}));
    }

    #[tokio::test]
    async fn transaction_not_found() {
        let store = InMemoryStore::new();
        let key = test_key();

        let err = store
            .transaction(&key, "nonexistent", Box::new(|_existing| unreachable!()))
            .unwrap_err();
        assert!(matches!(err, AppError::NotFound { .. }));
    }

    #[tokio::test]
    async fn delete_returns_object_and_get_not_found() {
        let store = InMemoryStore::new();
        let key = test_key();

        let created =
            store.create(test_obj(key.clone(), "my-widget", json!({"x": 1}))).await.unwrap();

        let deleted = store
            .transaction(&key, "my-widget", Box::new(|_existing| TransactionOp::Delete))
            .unwrap();
        assert_eq!(deleted.metadata.name, created.metadata.name);
        assert_eq!(deleted.spec, created.spec);

        let err = store.get(&key, "my-widget").await.unwrap_err();
        assert!(matches!(err, AppError::NotFound { .. }));
    }

    #[tokio::test]
    async fn delete_none_version_succeeds() {
        let store = InMemoryStore::new();
        let key = test_key();

        store.create(test_obj(key.clone(), "my-widget", json!({"x": 1}))).await.unwrap();
        store.create(test_obj(key.clone(), "other", json!({"x": 2}))).await.unwrap();

        store.transaction(&key, "my-widget", Box::new(|_existing| TransactionOp::Delete)).unwrap();

        let err = store.get(&key, "my-widget").await.unwrap_err();
        assert!(matches!(err, AppError::NotFound { .. }));

        let other = store.get(&key, "other").await.unwrap();
        assert_eq!(other.spec, json!({"x": 2}));
    }

    #[tokio::test]
    async fn delete_missing_not_found() {
        let store = InMemoryStore::new();
        let key = test_key();

        let err = store
            .transaction(&key, "nonexistent", Box::new(|_existing| unreachable!()))
            .unwrap_err();
        assert!(matches!(err, AppError::NotFound { .. }));
    }

    #[tokio::test]
    async fn list_empty_key() {
        let store = InMemoryStore::new();
        let key = test_key();

        let res = store
            .list(&key, ListOptions { limit: None, continue_token: None, ..Default::default() })
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

        store.create(test_obj(key.clone(), "foo", json!({}))).await.unwrap();
        store.create(test_obj(key.clone(), "bar", json!({}))).await.unwrap();
        store.create(test_obj(key.clone(), "baz", json!({}))).await.unwrap();

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
        let mut obj = test_obj(key.clone(), "web-1", json!({}));
        obj.metadata.labels = labels_nginx;
        store.create(obj).await.unwrap();

        let mut labels_apache = HashMap::new();
        labels_apache.insert("app".to_string(), "apache".to_string());
        let mut obj = test_obj(key.clone(), "web-2", json!({}));
        obj.metadata.labels = labels_apache;
        store.create(obj).await.unwrap();

        store.create(test_obj(key.clone(), "web-3", json!({}))).await.unwrap();

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
        let mut obj = test_obj(key.clone(), "target", json!({}));
        obj.metadata.labels = labels;
        store.create(obj).await.unwrap();

        let mut labels2 = HashMap::new();
        labels2.insert("app".to_string(), "nginx".to_string());
        let mut obj = test_obj(key.clone(), "other", json!({}));
        obj.metadata.labels = labels2;
        store.create(obj).await.unwrap();

        store.create(test_obj(key.clone(), "target-nolabel", json!({}))).await.unwrap();

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
            let mut obj = test_obj(key.clone(), &format!("obj-{i:02}"), json!({}));
            if i < 3 {
                obj.metadata.labels.insert("app".to_string(), "nginx".to_string());
            }
            store.create(obj).await.unwrap();
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

        store.create(test_obj(key.clone(), "foo", json!({}))).await.unwrap();

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

        store.create(test_obj(key.clone(), "exists-test", json!({"x": 1}))).await.unwrap();

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

        store.create(test_obj(key.clone(), "test", json!({}))).await.unwrap();

        let other_key = ResourceKey {
            group: "other.io".to_string(),
            version: "v1".to_string(),
            kind: "Other".to_string(),
        };
        assert!(!store.exists(&other_key).await.unwrap());
    }

    // --- Status transaction tests (replacing update_status) ---

    #[tokio::test]
    async fn update_status_success() {
        let store = InMemoryStore::new();
        let key = test_key();

        let created = store
            .create(test_obj(key.clone(), "my-widget", json!({"color": "blue"})))
            .await
            .unwrap();
        let v1 = created.system.resource_version;
        assert!(created.status.is_none());

        let updated = store
            .transaction(
                &key,
                "my-widget",
                Box::new(|existing| {
                    let mut obj = existing.clone();
                    obj.status = Some(json!({"phase": "Running"}));
                    // Caller sets metadata before Apply
                    obj.system.resource_version = existing.system.resource_version + 1;
                    TransactionOp::Apply(obj)
                }),
            )
            .unwrap();

        assert!(updated.status.is_some());
        assert_eq!(updated.status.clone().unwrap(), json!({"phase": "Running"}));
        assert_eq!(updated.system.resource_version, v1 + 1);
        // Spec should be unchanged
        assert_eq!(updated.spec, json!({"color": "blue"}));
    }

    #[tokio::test]
    async fn update_status_not_found() {
        let store = InMemoryStore::new();
        let key = test_key();

        let err = store
            .transaction(&key, "nonexistent", Box::new(|_existing| unreachable!()))
            .unwrap_err();
        assert!(matches!(err, AppError::NotFound { .. }));
    }

    #[tokio::test]
    async fn update_status_replaces_existing_status() {
        let store = InMemoryStore::new();
        let key = test_key();

        store.create(test_obj(key.clone(), "my-widget", json!({"color": "blue"}))).await.unwrap();

        // First status update
        store
            .transaction(
                &key,
                "my-widget",
                Box::new(|existing| {
                    let mut obj = existing.clone();
                    obj.status = Some(json!({"phase": "Pending"}));
                    TransactionOp::Apply(obj)
                }),
            )
            .unwrap();

        // Second status update (replace)
        let updated = store
            .transaction(
                &key,
                "my-widget",
                Box::new(|existing| {
                    let mut obj = existing.clone();
                    obj.status = Some(json!({"phase": "Running"}));
                    TransactionOp::Apply(obj)
                }),
            )
            .unwrap();

        assert_eq!(updated.status.unwrap(), json!({"phase": "Running"}));
    }

    #[tokio::test]
    async fn update_status_bumps_resource_version() {
        let store = InMemoryStore::new();
        let key = test_key();

        let created = store.create(test_obj(key.clone(), "my-widget", json!({}))).await.unwrap();
        let v1 = created.system.resource_version;

        let updated = store
            .transaction(
                &key,
                "my-widget",
                Box::new(|existing| {
                    let mut obj = existing.clone();
                    obj.status = Some(json!({"phase": "Running"}));
                    // Caller bumps resource_version
                    obj.system.resource_version = existing.system.resource_version + 1;
                    TransactionOp::Apply(obj)
                }),
            )
            .unwrap();

        assert_eq!(updated.system.resource_version, v1 + 1);
    }

    #[tokio::test]
    async fn update_status_preserves_spec() {
        let store = InMemoryStore::new();
        let key = test_key();

        store
            .create(test_obj(key.clone(), "my-widget", json!({"color": "blue", "size": 10})))
            .await
            .unwrap();

        let updated = store
            .transaction(
                &key,
                "my-widget",
                Box::new(|existing| {
                    let mut obj = existing.clone();
                    obj.status = Some(json!({"phase": "Running"}));
                    TransactionOp::Apply(obj)
                }),
            )
            .unwrap();

        assert_eq!(updated.spec, json!({"color": "blue", "size": 10}));
    }

    // --- Generation transaction tests (replacing old update generation tests) ---

    #[tokio::test]
    async fn update_metadata_only_does_not_bump_generation() {
        let store = InMemoryStore::new();
        let key = test_key();

        let created = store
            .create(test_obj(key.clone(), "my-widget", json!({"color": "blue"})))
            .await
            .unwrap();
        let gen_before = created.system.generation;
        assert_eq!(gen_before, 1);

        // Update with same spec but different labels; caller keeps generation unchanged
        let updated = store
            .transaction(
                &key,
                "my-widget",
                Box::new(move |existing| {
                    let mut obj = existing.clone();
                    let mut labels = HashMap::new();
                    labels.insert("env".to_string(), "prod".to_string());
                    obj.metadata.labels = labels;
                    // Keep same spec, keep same generation
                    obj.system.generation = gen_before;
                    TransactionOp::Apply(obj)
                }),
            )
            .unwrap();

        assert_eq!(updated.system.generation, gen_before);
    }

    #[tokio::test]
    async fn update_spec_change_bumps_generation() {
        let store = InMemoryStore::new();
        let key = test_key();

        let created = store
            .create(test_obj(key.clone(), "my-widget", json!({"color": "blue"})))
            .await
            .unwrap();
        let gen_before = created.system.generation;
        assert_eq!(gen_before, 1);

        // Update with different spec; caller bumps generation
        let updated = store
            .transaction(
                &key,
                "my-widget",
                Box::new(move |existing| {
                    let mut obj = existing.clone();
                    obj.spec = json!({"color": "red"});
                    obj.system.generation = gen_before + 1;
                    TransactionOp::Apply(obj)
                }),
            )
            .unwrap();

        assert_eq!(updated.system.generation, gen_before + 1);
    }

    // --- New transaction tests ---

    #[tokio::test]
    async fn transaction_abort_does_not_modify() {
        let store = InMemoryStore::new();
        let key = test_key();

        store.create(test_obj(key.clone(), "my-widget", json!({"x": 1}))).await.unwrap();

        // Attempt transaction that aborts
        let err = store
            .transaction(
                &key,
                "my-widget",
                Box::new(|_existing| {
                    TransactionOp::Abort(AppError::Internal(anyhow::anyhow!("aborted")))
                }),
            )
            .unwrap_err();

        assert!(matches!(err, AppError::Internal(_)));

        // Object should be unchanged
        let obj = store.get(&key, "my-widget").await.unwrap();
        assert_eq!(obj.spec, json!({"x": 1}));
    }

    #[tokio::test]
    async fn transaction_persists_caller_provided_rv() {
        let store = InMemoryStore::new();
        let key = test_key();

        store.create(test_obj(key.clone(), "my-widget", json!({"x": 1}))).await.unwrap();

        let result = store
            .transaction(
                &key,
                "my-widget",
                Box::new(|existing| {
                    let mut obj = existing.clone();
                    obj.spec = json!({"x": 2});
                    obj.system.resource_version = existing.system.resource_version + 1;
                    TransactionOp::Apply(obj)
                }),
            )
            .unwrap();

        assert_eq!(result.system.resource_version, 2);

        let result2 = store
            .transaction(
                &key,
                "my-widget",
                Box::new(|existing| {
                    let mut obj = existing.clone();
                    obj.spec = json!({"x": 3});
                    obj.system.resource_version = existing.system.resource_version + 1;
                    TransactionOp::Apply(obj)
                }),
            )
            .unwrap();

        assert_eq!(result2.system.resource_version, 3);
    }

    #[tokio::test]
    async fn transaction_delete_removes_object() {
        let store = InMemoryStore::new();
        let key = test_key();

        store.create(test_obj(key.clone(), "my-widget", json!({"x": 1}))).await.unwrap();

        // Delete via transaction
        store.transaction(&key, "my-widget", Box::new(|_existing| TransactionOp::Delete)).unwrap();

        // Get should now fail with NotFound
        let err = store.get(&key, "my-widget").await.unwrap_err();
        assert!(matches!(err, AppError::NotFound { .. }));
    }
}
