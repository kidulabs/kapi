use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use base64::Engine;
use chrono::{DateTime, Utc};
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::Value;

use std::collections::HashMap;

use crate::error::AppError;
use crate::object::types::{
    ContinueToken, ListOptions, ListResponse, ObjectMeta, StoredObject, SystemMetadata, UserData,
};
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
            && !parent.as_os_str().is_empty()
        {
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

    /// Creates the objects table, labels table, and indexes if they don't exist. Idempotent.
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
        )
        .map_err(|e| AppError::Internal(e.into()))?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_objects_gvkn ON objects(resource_group, api_version, resource_kind, name)",
            [],
        ).map_err(|e| AppError::Internal(e.into()))?;

        // Enable foreign key support (required for ON DELETE CASCADE)
        conn.execute("PRAGMA foreign_keys = ON", [])
            .map_err(|e| AppError::Internal(e.into()))?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS labels (
                resource_group  TEXT NOT NULL,
                api_version     TEXT NOT NULL,
                resource_kind   TEXT NOT NULL,
                name            TEXT NOT NULL,
                label_key       TEXT NOT NULL,
                label_value     TEXT NOT NULL,
                PRIMARY KEY (resource_group, api_version, resource_kind, name, label_key),
                FOREIGN KEY (resource_group, api_version, resource_kind, name)
                    REFERENCES objects(resource_group, api_version, resource_kind, name)
                    ON DELETE CASCADE
            )",
            [],
        )
        .map_err(|e| AppError::Internal(e.into()))?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_labels_gvkn ON labels(resource_group, api_version, resource_kind, name)",
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
    /// Labels are set to empty — callers must populate them via `query_labels()`.
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
        let data_value: Value =
            serde_json::from_str(&data).map_err(|e| AppError::Internal(e.into()))?;
        let created_at =
            DateTime::parse_from_rfc3339(&created_at).map_err(|e| AppError::Internal(e.into()))?;
        let updated_at =
            DateTime::parse_from_rfc3339(&updated_at).map_err(|e| AppError::Internal(e.into()))?;
        Ok(StoredObject {
            key: ResourceKey {
                group,
                version,
                kind,
            },
            metadata: ObjectMeta {
                name,
                labels: HashMap::new(),
            },
            system: SystemMetadata {
                resource_version: resource_version as u64,
                created_at: created_at.with_timezone(&Utc),
                updated_at: updated_at.with_timezone(&Utc),
            },
            data: UserData { value: data_value },
        })
    }

    /// Queries labels from the `labels` table for a single object.
    fn query_labels(
        conn: &Connection,
        group: &str,
        version: &str,
        kind: &str,
        name: &str,
    ) -> Result<HashMap<String, String>, AppError> {
        let mut stmt = conn.prepare(
            "SELECT label_key, label_value FROM labels WHERE resource_group = ?1 AND api_version = ?2 AND resource_kind = ?3 AND name = ?4"
        ).map_err(|e| AppError::Internal(e.into()))?;
        let rows = stmt
            .query_map(params![group, version, kind, name], |row| {
                let key: String = row.get(0)?;
                let value: String = row.get(1)?;
                Ok((key, value))
            })
            .map_err(|e| AppError::Internal(e.into()))?;

        let mut labels = HashMap::new();
        for row in rows {
            let (k, v) = row.map_err(|e| AppError::Internal(e.into()))?;
            labels.insert(k, v);
        }
        Ok(labels)
    }

    /// Batch-fetches labels for multiple objects identified by (group, version, kind, name).
    /// Returns a map from name → labels (all objects share the same group/version/kind).
    fn batch_query_labels(
        conn: &Connection,
        group: &str,
        version: &str,
        kind: &str,
        names: &[String],
    ) -> Result<HashMap<String, HashMap<String, String>>, AppError> {
        if names.is_empty() {
            return Ok(HashMap::new());
        }

        // Build IN clause with placeholders
        let placeholders: Vec<String> = (1..=names.len()).map(|i| format!("?{}", i)).collect();
        let sql = format!(
            "SELECT name, label_key, label_value FROM labels WHERE resource_group = ?{} AND api_version = ?{} AND resource_kind = ?{} AND name IN ({})",
            names.len() + 1,
            names.len() + 2,
            names.len() + 3,
            placeholders.join(", ")
        );

        let mut stmt = conn
            .prepare(&sql)
            .map_err(|e| AppError::Internal(e.into()))?;

        let mut params_vec: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        for name in names {
            params_vec.push(Box::new(name.clone()));
        }
        params_vec.push(Box::new(group.to_string()));
        params_vec.push(Box::new(version.to_string()));
        params_vec.push(Box::new(kind.to_string()));

        let params_refs: Vec<&dyn rusqlite::types::ToSql> =
            params_vec.iter().map(|b| b.as_ref()).collect();

        let rows = stmt
            .query_map(params_refs.as_slice(), |row| {
                let name: String = row.get(0)?;
                let key: String = row.get(1)?;
                let value: String = row.get(2)?;
                Ok((name, key, value))
            })
            .map_err(|e| AppError::Internal(e.into()))?;

        let mut result: HashMap<String, HashMap<String, String>> = HashMap::new();
        for row in rows {
            let (name, key, value) = row.map_err(|e| AppError::Internal(e.into()))?;
            result.entry(name).or_default().insert(key, value);
        }
        Ok(result)
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
    SQLiteStore::deserialize_row(
        group,
        version,
        kind,
        name,
        data,
        resource_version,
        created_at,
        updated_at,
    )
    .map_err(|e| rusqlite::Error::ToSqlConversionFailure(e.into()))
}

#[async_trait]
impl ObjectStore for SQLiteStore {
    /// Inserts a new object. Returns `Conflict` if the composite key already exists.
    /// Labels are inserted into the `labels` table within the same transaction.
    async fn create(
        &self,
        key: &ResourceKey,
        meta: ObjectMeta,
        data: Value,
    ) -> Result<StoredObject, AppError> {
        let key = key.clone();
        let conn = Arc::clone(&self.conn);
        let next_version = Arc::clone(&self.next_version);

        tokio::task::spawn_blocking(move || {
            let now = SQLiteStore::now();
            let version = next_version.fetch_add(1, Ordering::Relaxed);

            let data_json = serde_json::to_string(&data).map_err(|e| AppError::Internal(e.into()))?;
            let created_at = now.to_rfc3339();
            let updated_at = now.to_rfc3339();

            let c = conn.lock().unwrap();

            // Use immediate transaction for atomicity of object + labels
            let tx = c.unchecked_transaction().map_err(|e| AppError::Internal(e.into()))?;

            let result = tx.execute(
                "INSERT INTO objects (resource_group, api_version, resource_kind, name, data, resource_version, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    key.group, key.version, key.kind, meta.name,
                    data_json, version as i64, created_at, updated_at
                ],
            );

            match result {
                Ok(_) => {
                    // Insert labels if non-empty
                    if !meta.labels.is_empty() {
                        let mut stmt = tx.prepare(
                            "INSERT INTO labels (resource_group, api_version, resource_kind, name, label_key, label_value)
                             VALUES (?1, ?2, ?3, ?4, ?5, ?6)"
                        ).map_err(|e| AppError::Internal(e.into()))?;
                        for (label_key, label_value) in &meta.labels {
                            stmt.execute(params![
                                key.group, key.version, key.kind, meta.name,
                                label_key, label_value
                            ]).map_err(|e| AppError::Internal(e.into()))?;
                        }
                    }

                    tx.commit().map_err(|e| AppError::Internal(e.into()))?;

                    Ok(StoredObject {
                        key,
                        metadata: meta,
                        system: SystemMetadata {
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
                        name: meta.name.clone(),
                    })
                }
                Err(e) => Err(AppError::Internal(e.into())),
            }
        })
        .await
        .map_err(|e| AppError::Internal(e.into()))?
    }

    /// Fetches a single object by composite key. Returns `NotFound` if missing.
    /// Labels are queried from the `labels` table and populated on the returned object.
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
            let mut obj = stmt
                .query_row(
                    params![key.group, key.version, key.kind, name],
                    row_to_object,
                )
                .optional()
                .map_err(|e| AppError::Internal(e.into()))?;

            match obj {
                Some(ref mut obj) => {
                    obj.metadata.labels = SQLiteStore::query_labels(
                        &c, &key.group, &key.version, &key.kind, &name
                    )?;
                    Ok(obj.clone())
                }
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
            let mut items: Vec<StoredObject> = if has_more {
                items[..limit].to_vec()
            } else {
                items
            };

            // Batch-fetch labels for all returned objects
            let names: Vec<String> = items.iter().map(|o| o.metadata.name.clone()).collect();
            let labels_map = SQLiteStore::batch_query_labels(
                &c, &key.group, &key.version, &key.kind, &names
            )?;
            for item in &mut items {
                if let Some(labels) = labels_map.get(&item.metadata.name) {
                    item.metadata.labels = labels.clone();
                }
            }

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
    /// Labels are updated via diff-based strategy within the same transaction.
    async fn update(&self, object: StoredObject) -> Result<StoredObject, AppError> {
        let conn = Arc::clone(&self.conn);
        let next_version = Arc::clone(&self.next_version);

        tokio::task::spawn_blocking(move || {
            let now = SQLiteStore::now();
            let new_version = next_version.fetch_add(1, Ordering::Relaxed);

            let data_json = serde_json::to_string(&object.data.value).map_err(|e| AppError::Internal(e.into()))?;
            let updated_at = now.to_rfc3339();
            let expected_version = object.system.resource_version as i64;

            let c = conn.lock().unwrap();

            // Use immediate transaction for atomicity of object update + label diff
            let tx = c.unchecked_transaction().map_err(|e| AppError::Internal(e.into()))?;

            let rows = tx.execute(
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
                let exists = tx.query_row(
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
                        let actual: u64 = tx.query_row(
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

            // Diff-based label update: read existing, compute diff, apply
            let existing_labels = SQLiteStore::query_labels(
                &tx, &object.key.group, &object.key.version, &object.key.kind, &object.metadata.name
            )?;
            let new_labels = &object.metadata.labels;

            // Keys to delete: in existing but not in new
            let to_delete: Vec<&String> = existing_labels
                .keys()
                .filter(|k| !new_labels.contains_key(*k))
                .collect();

            // Keys to upsert: in new but value differs from existing, or not in existing
            let to_upsert: Vec<(&String, &String)> = new_labels
                .iter()
                .filter(|(k, v)| existing_labels.get(*k) != Some(*v))
                .collect();

            // Apply deletes
            if !to_delete.is_empty() {
                let mut del_stmt = tx.prepare(
                    "DELETE FROM labels WHERE resource_group = ?1 AND api_version = ?2 AND resource_kind = ?3 AND name = ?4 AND label_key = ?5"
                ).map_err(|e| AppError::Internal(e.into()))?;
                for key in &to_delete {
                    del_stmt.execute(params![
                        object.key.group, object.key.version, object.key.kind,
                        object.metadata.name, key
                    ]).map_err(|e| AppError::Internal(e.into()))?;
                }
            }

            // Apply upserts
            if !to_upsert.is_empty() {
                let mut upsert_stmt = tx.prepare(
                    "INSERT OR REPLACE INTO labels (resource_group, api_version, resource_kind, name, label_key, label_value)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)"
                ).map_err(|e| AppError::Internal(e.into()))?;
                for (key, value) in &to_upsert {
                    upsert_stmt.execute(params![
                        object.key.group, object.key.version, object.key.kind,
                        object.metadata.name, key, value
                    ]).map_err(|e| AppError::Internal(e.into()))?;
                }
            }

            tx.commit().map_err(|e| AppError::Internal(e.into()))?;

            Ok(StoredObject {
                key: object.key,
                metadata: object.metadata,
                system: SystemMetadata {
                    resource_version: new_version,
                    created_at: object.system.created_at,
                    updated_at: now,
                },
                data: object.data,
            })
        })
        .await
        .map_err(|e| AppError::Internal(e.into()))?
    }

    /// Deletes an object unconditionally (no version check). Returns the deleted object.
    /// Labels are automatically removed via ON DELETE CASCADE.
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
            let mut obj = stmt
                .query_row(
                    params![key.group, key.version, key.kind, name],
                    row_to_object,
                )
                .optional()
                .map_err(|e| AppError::Internal(e.into()))?;

            let obj = match obj {
                Some(ref mut obj) => {
                    // Query labels before deletion (CASCADE will remove them)
                    obj.metadata.labels = SQLiteStore::query_labels(
                        &c, &key.group, &key.version, &key.kind, &name
                    )?;
                    obj.clone()
                }
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
        assert_eq!(created.data.value, data);
        assert_eq!(created.key, key);
        assert_eq!(created.system.resource_version, 1);

        let retrieved = store.get(&key, "my-widget").await.unwrap();
        assert_eq!(retrieved.metadata.name, created.metadata.name);
        assert_eq!(retrieved.data.value, created.data.value);
        assert_eq!(
            retrieved.system.resource_version,
            created.system.resource_version
        );
    }

    #[tokio::test]
    async fn create_duplicate_conflict() {
        let (store, _dir) = temp_store();
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
        let (store, _dir) = temp_store();
        let key = test_key();

        let err = store.get(&key, "nonexistent").await.unwrap_err();
        assert!(matches!(err, AppError::NotFound { .. }));
    }

    #[tokio::test]
    async fn list_sorted_by_name() {
        let (store, _dir) = temp_store();
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
            data: UserData {
                value: json!({"x": 2}),
            },
        };

        let updated = store.update(object).await.unwrap();
        assert!(updated.system.resource_version > v1);
        assert_eq!(updated.data.value, json!({"x": 2}));
    }

    #[tokio::test]
    async fn update_wrong_version_conflict() {
        let (store, _dir) = temp_store();
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
            metadata: ObjectMeta {
                name: "nonexistent".to_string(),
                labels: HashMap::new(),
            },
            system: SystemMetadata {
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
                .create(
                    &key,
                    ObjectMeta {
                        name: "persistent".to_string(),
                        labels: HashMap::new(),
                    },
                    json!({"data": "hello"}),
                )
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
