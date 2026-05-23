use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use base64::Engine;
use chrono::{DateTime, Utc};
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::Value;

use crate::error::AppError;
use crate::object::types::{ContinueToken, ListOptions, ListResponse, ObjectMetadata, StoredObject, UserData};
use crate::store::{ObjectStore, ResourceKey};

/// SQLite-backed implementation of `ObjectStore`.
///
/// Uses a single connection behind `Arc<Mutex>` with `spawn_blocking`
/// to avoid blocking the async runtime. Version counter is restored
/// from `MAX(resource_version)` on startup.
pub struct SQLiteStore {
    conn: Arc<Mutex<Connection>>,
    next_version: Arc<AtomicU64>,
}

impl SQLiteStore {
    /// Opens or creates the database at `path`, initializing the schema.
    /// Parent directories are created if missing.
    pub fn new(path: &str) -> Result<Self, AppError> {
        if let Some(parent) = Path::new(path).parent()
            && !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|e| AppError::Internal(e.into()))?;
        }

        let conn = Connection::open(path).map_err(|e| AppError::Internal(e.into()))?;
        let store = Self {
            conn: Arc::new(Mutex::new(conn)),
            next_version: Arc::new(AtomicU64::new(1)),
        };
        store.init_schema()?;
        store.init_version_counter()?;
        Ok(store)
    }

    /// Creates the objects table and index if they don't exist. Idempotent.
    fn init_schema(&self) -> Result<(), AppError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "CREATE TABLE IF NOT EXISTS objects (
                resource_group     TEXT    NOT NULL,
                api_version        TEXT    NOT NULL,
                resource_kind      TEXT    NOT NULL,
                name               TEXT    NOT NULL,
                data               TEXT    NOT NULL,
                resource_version   INTEGER NOT NULL,
                created_at         TEXT    NOT NULL,
                updated_at         TEXT    NOT NULL,
                PRIMARY KEY (resource_group, api_version, resource_kind, name)
            )",
            [],
        ).map_err(|e| AppError::Internal(e.into()))?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_objects_gvkn ON objects(resource_group, api_version, resource_kind, name)",
            [],
        ).map_err(|e| AppError::Internal(e.into()))?;
        Ok(())
    }

    /// Restores the version counter from the highest existing resource_version.
    /// Ensures monotonicity across restarts.
    fn init_version_counter(&self) -> Result<(), AppError> {
        let conn = self.conn.lock().unwrap();
        let max_version: Option<u64> = conn
            .query_row("SELECT MAX(resource_version) FROM objects", [], |row| {
                row.get(0)
            })
            .optional()
            .map_err(|e| AppError::Internal(e.into()))?
            .flatten();
        if let Some(max) = max_version {
            self.next_version.store(max + 1, Ordering::Relaxed);
        }
        Ok(())
    }

    /// Converts raw column values from a query row into a `StoredObject`.
    #[allow(clippy::too_many_arguments)]
    fn deserialize_row(
        group: String,
        version: String,
        kind: String,
        name: String,
        data: String,
        resource_version: i64,
        created_at: String,
        updated_at: String,
    ) -> Result<StoredObject, AppError> {
        let data_value: Value = serde_json::from_str(&data).map_err(|e| AppError::Internal(e.into()))?;
        let created_at = DateTime::parse_from_rfc3339(&created_at).map_err(|e| AppError::Internal(e.into()))?;
        let updated_at = DateTime::parse_from_rfc3339(&updated_at).map_err(|e| AppError::Internal(e.into()))?;
        Ok(StoredObject {
            key: ResourceKey { group, version, kind },
            metadata: ObjectMetadata {
                name,
                resource_version: resource_version as u64,
                created_at: created_at.with_timezone(&Utc),
                updated_at: updated_at.with_timezone(&Utc),
            },
            data: UserData { value: data_value },
        })
    }

    fn now() -> DateTime<Utc> {
        Utc::now()
    }
}

/// Maps a rusqlite row to `StoredObject`. Used as the row callback in `query_row` / `query_map`.
fn row_to_object(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredObject> {
    let group: String = row.get("resource_group")?;
    let version: String = row.get("api_version")?;
    let kind: String = row.get("resource_kind")?;
    let name: String = row.get("name")?;
    let data: String = row.get("data")?;
    let resource_version: i64 = row.get("resource_version")?;
    let created_at: String = row.get("created_at")?;
    let updated_at: String = row.get("updated_at")?;
    SQLiteStore::deserialize_row(group, version, kind, name, data, resource_version, created_at, updated_at)
        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(e.into()))
}

#[async_trait]
impl ObjectStore for SQLiteStore {
    /// Inserts a new object. Returns `Conflict` if the composite key already exists.
    async fn create(
        &self,
        key: &ResourceKey,
        name: &str,
        data: Value,
    ) -> Result<StoredObject, AppError> {
        let key = key.clone();
        let name = name.to_string();
        let conn = Arc::clone(&self.conn);
        let next_version = Arc::clone(&self.next_version);

        tokio::task::spawn_blocking(move || {
            let now = SQLiteStore::now();
            let version = next_version.fetch_add(1, Ordering::Relaxed);

            let data_json = serde_json::to_string(&data).map_err(|e| AppError::Internal(e.into()))?;
            let created_at = now.to_rfc3339();
            let updated_at = now.to_rfc3339();

            let c = conn.lock().unwrap();
            let result = c.execute(
                "INSERT INTO objects (resource_group, api_version, resource_kind, name, data, resource_version, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    key.group, key.version, key.kind, name,
                    data_json, version as i64, created_at, updated_at
                ],
            );

            match result {
                Ok(_) => {
                    Ok(StoredObject {
                        key,
                        metadata: ObjectMetadata {
                            name,
                            resource_version: version,
                            created_at: now,
                            updated_at: now,
                        },
                        data: UserData { value: data },
                    })
                }
                Err(rusqlite::Error::SqliteFailure(err, _))
                    if err.code == rusqlite::ErrorCode::ConstraintViolation =>
                {
                    // Primary key conflict → duplicate
                    Err(AppError::AlreadyExists {
                        kind: key.kind.clone(),
                        name: name.clone(),
                    })
                }
                Err(e) => Err(AppError::Internal(e.into())),
            }
        })
        .await
        .map_err(|e| AppError::Internal(e.into()))?
    }

    /// Fetches a single object by composite key. Returns `NotFound` if missing.
    async fn get(&self, key: &ResourceKey, name: &str) -> Result<StoredObject, AppError> {
        let key = key.clone();
        let name = name.to_string();
        let conn = Arc::clone(&self.conn);

        tokio::task::spawn_blocking(move || {
            let c = conn.lock().unwrap();
            let mut stmt = c.prepare(
                "SELECT resource_group, api_version, resource_kind, name, data, resource_version, created_at, updated_at
                 FROM objects WHERE resource_group = ?1 AND api_version = ?2 AND resource_kind = ?3 AND name = ?4",
            ).map_err(|e| AppError::Internal(e.into()))?;
            let obj = stmt
                .query_row(
                    params![key.group, key.version, key.kind, name],
                    row_to_object,
                )
                .optional()
                .map_err(|e| AppError::Internal(e.into()))?;

            match obj {
                Some(obj) => Ok(obj),
                None => Err(AppError::NotFound {
                    what: "object".to_string(),
                    identifier: format!("{}/{}", key.kind, name),
                }),
            }
        })
        .await
        .map_err(|e| AppError::Internal(e.into()))?
    }

    /// Lists objects for a key, sorted by name. Supports limit and cursor-based pagination.
    async fn list(&self, key: &ResourceKey, opts: ListOptions) -> Result<ListResponse, AppError> {
        let key = key.clone();
        let conn = Arc::clone(&self.conn);

        tokio::task::spawn_blocking(move || {
            let skip_past = opts
                .continue_token
                .as_ref()
                .map(decode_continue_token)
                .transpose()?;

            let limit = opts.limit.unwrap_or(usize::MAX);
            // Fetch one extra to detect if more pages exist
            let query_limit = limit.saturating_add(1);

            let c = conn.lock().unwrap();

            // Build query: use `name > ?` skip condition when a continue token is present
            let (sql, has_skip) = if skip_past.is_some() {
                (
                    "SELECT resource_group, api_version, resource_kind, name, data, resource_version, created_at, updated_at
                     FROM objects
                     WHERE resource_group = ?1 AND api_version = ?2 AND resource_kind = ?3 AND name > ?4
                     ORDER BY name ASC
                     LIMIT ?5",
                    true,
                )
            } else {
                (
                    "SELECT resource_group, api_version, resource_kind, name, data, resource_version, created_at, updated_at
                     FROM objects
                     WHERE resource_group = ?1 AND api_version = ?2 AND resource_kind = ?3
                     ORDER BY name ASC
                     LIMIT ?4",
                    false,
                )
            };

            let mut stmt = c.prepare(sql).map_err(|e| AppError::Internal(e.into()))?;

            let rows = if has_skip {
                stmt.query_map(
                    params![key.group, key.version, key.kind, skip_past.unwrap(), query_limit as i64],
                    row_to_object,
                ).map_err(|e| AppError::Internal(e.into()))?
            } else {
                stmt.query_map(
                    params![key.group, key.version, key.kind, query_limit as i64],
                    row_to_object,
                ).map_err(|e| AppError::Internal(e.into()))?
            };

            let items: Vec<StoredObject> = rows
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| AppError::Internal(e.into()))?;

            let has_more = items.len() > limit;
            let items: Vec<StoredObject> = if has_more {
                items[..limit].to_vec()
            } else {
                items
            };

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
        })
        .await
        .map_err(|e| AppError::Internal(e.into()))?
    }

    /// Updates an object with optimistic concurrency control.
    /// Returns `Conflict` if the resource_version doesn't match, `NotFound` if missing.
    async fn update(&self, object: StoredObject) -> Result<StoredObject, AppError> {
        let conn = Arc::clone(&self.conn);
        let next_version = Arc::clone(&self.next_version);

        tokio::task::spawn_blocking(move || {
            let now = SQLiteStore::now();
            let new_version = next_version.fetch_add(1, Ordering::Relaxed);

            let data_json = serde_json::to_string(&object.data.value).map_err(|e| AppError::Internal(e.into()))?;
            let updated_at = now.to_rfc3339();
            let expected_version = object.metadata.resource_version as i64;

            let c = conn.lock().unwrap();
            let rows = c.execute(
                "UPDATE objects SET data = ?1, resource_version = ?2, updated_at = ?3
                 WHERE resource_group = ?4 AND api_version = ?5 AND resource_kind = ?6 AND name = ?7
                 AND resource_version = ?8",
                params![
                    data_json,
                    new_version as i64,
                    updated_at,
                    object.key.group,
                    object.key.version,
                    object.key.kind,
                    object.metadata.name,
                    expected_version,
                ],
            ).map_err(|e| AppError::Internal(e.into()))?;

            if rows == 0 {
                // Zero rows updated: either version mismatch or object doesn't exist
                let exists = c.query_row(
                    "SELECT 1 FROM objects WHERE resource_group = ?1 AND api_version = ?2 AND resource_kind = ?3 AND name = ?4",
                    params![
                        object.key.group,
                        object.key.version,
                        object.key.kind,
                        object.metadata.name,
                    ],
                    |_| Ok(()),
                ).optional().map_err(|e| AppError::Internal(e.into()))?;

                return match exists {
                    Some(_) => {
                        // Object exists but version didn't match — return conflict with actual version
                        let actual: u64 = c.query_row(
                            "SELECT resource_version FROM objects WHERE resource_group = ?1 AND api_version = ?2 AND resource_kind = ?3 AND name = ?4",
                            params![
                                object.key.group,
                                object.key.version,
                                object.key.kind,
                                object.metadata.name,
                            ],
                            |row| row.get(0),
                        ).map_err(|e| AppError::Internal(e.into()))?;
                        Err(AppError::Conflict {
                            expected: expected_version as u64,
                            actual,
                        })
                    }
                    None => Err(AppError::NotFound {
                        what: "object".to_string(),
                        identifier: format!("{}/{}", object.key.kind, object.metadata.name),
                    }),
                };
            }

            Ok(StoredObject {
                key: object.key,
                metadata: ObjectMetadata {
                    name: object.metadata.name,
                    resource_version: new_version,
                    created_at: object.metadata.created_at,
                    updated_at: now,
                },
                data: object.data,
            })
        })
        .await
        .map_err(|e| AppError::Internal(e.into()))?
    }

    /// Deletes an object unconditionally (no version check). Returns the deleted object.
    async fn delete(&self, key: &ResourceKey, name: &str) -> Result<StoredObject, AppError> {
        let key = key.clone();
        let name = name.to_string();
        let conn = Arc::clone(&self.conn);

        tokio::task::spawn_blocking(move || {
            let c = conn.lock().unwrap();

            // Fetch the object first so we can return it after deletion
            let mut stmt = c.prepare(
                "SELECT resource_group, api_version, resource_kind, name, data, resource_version, created_at, updated_at
                 FROM objects WHERE resource_group = ?1 AND api_version = ?2 AND resource_kind = ?3 AND name = ?4",
            ).map_err(|e| AppError::Internal(e.into()))?;
            let obj = stmt
                .query_row(
                    params![key.group, key.version, key.kind, name],
                    row_to_object,
                )
                .optional()
                .map_err(|e| AppError::Internal(e.into()))?;

            let obj = match obj {
                Some(obj) => obj,
                None => {
                    return Err(AppError::NotFound {
                        what: "object".to_string(),
                        identifier: format!("{}/{}", key.kind, name),
                    });
                }
            };

            c.execute(
                "DELETE FROM objects WHERE resource_group = ?1 AND api_version = ?2 AND resource_kind = ?3 AND name = ?4",
                params![key.group, key.version, key.kind, name],
            ).map_err(|e| AppError::Internal(e.into()))?;

            Ok(obj)
        })
        .await
        .map_err(|e| AppError::Internal(e.into()))?
    }
}

/// Decodes a base64-encoded continue token back to a name string.
fn decode_continue_token(token: &ContinueToken) -> Result<String, AppError> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(&token.0)
        .map_err(|_| AppError::Internal(anyhow::anyhow!("invalid continue token")))?;
    String::from_utf8(bytes)
        .map_err(|_| AppError::Internal(anyhow::anyhow!("invalid continue token")))
}

/// Encodes a name string into a base64 continue token.
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

    /// Creates a SQLiteStore backed by a temp file. The TempDir is returned
    /// to keep the file alive until the test drops it.
    fn temp_store() -> (SQLiteStore, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let store = SQLiteStore::new(db_path.to_str().unwrap()).unwrap();
        (store, dir)
    }

    #[tokio::test]
    async fn create_get_round_trip() {
        let (store, _dir) = temp_store();
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
        let (store, _dir) = temp_store();
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
        let (store, _dir) = temp_store();
        let key = test_key();

        let err = store.get(&key, "nonexistent").await.unwrap_err();
        assert!(matches!(err, AppError::NotFound { .. }));
    }

    #[tokio::test]
    async fn list_sorted_by_name() {
        let (store, _dir) = temp_store();
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
        let (store, _dir) = temp_store();
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
        let (store, _dir) = temp_store();
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
        let (store, _dir) = temp_store();
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
        let (store, _dir) = temp_store();
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
        assert!(matches!(err, AppError::Conflict { .. }));
    }

    #[tokio::test]
    async fn update_missing_not_found() {
        let (store, _dir) = temp_store();
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
        let (store, _dir) = temp_store();
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
    async fn delete_missing_not_found() {
        let (store, _dir) = temp_store();
        let key = test_key();

        let err = store.delete(&key, "nonexistent").await.unwrap_err();
        assert!(matches!(err, AppError::NotFound { .. }));
    }

    #[tokio::test]
    async fn list_empty_key() {
        let (store, _dir) = temp_store();
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

    /// Verifies that data persists after the store is dropped and recreated.
    #[tokio::test]
    async fn persistence_survives_restart() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");

        {
            let store = SQLiteStore::new(db_path.to_str().unwrap()).unwrap();
            let key = test_key();
            store
                .create(&key, "persistent", json!({"data": "hello"}))
                .await
                .unwrap();
        }

        {
            let store = SQLiteStore::new(db_path.to_str().unwrap()).unwrap();
            let key = test_key();
            let retrieved = store.get(&key, "persistent").await.unwrap();
            assert_eq!(retrieved.data.value, json!({"data": "hello"}));
        }
    }
}
