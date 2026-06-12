use std::path::Path;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use base64::Engine;
use chrono::{DateTime, Utc};
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::Value;

use std::collections::HashMap;

use crate::error::AppError;
use crate::object::types::{
    ContinueToken, FieldSelector, LabelRequirement, ListOptions, ListResponse, ObjectMeta,
    SpecData, StoredObject, SystemMetadata,
};
use crate::store::{ObjectStore, ResourceKey, TransactionOp};

/// SQLite-backed implementation of `ObjectStore`.
///
/// Uses a single connection behind `Arc<Mutex>` with `spawn_blocking`
/// to avoid blocking the async runtime. Does NOT maintain any global
/// version counter — the store persists objects as-is, and the service
/// layer is responsible for all system metadata.
pub struct SQLiteStore {
    conn: Arc<Mutex<Connection>>,
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
        };
        store.init_schema()?;
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
                spec               TEXT    NOT NULL,
                status             TEXT,
                resource_version   INTEGER NOT NULL,
                generation         INTEGER NOT NULL DEFAULT 1,
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

    /// Converts raw column values from a query row into a `StoredObject`.
    /// Labels are set to empty — callers must populate them via `query_labels()`.
    #[allow(clippy::too_many_arguments)]
    fn deserialize_row(
        group: String,
        version: String,
        kind: String,
        name: String,
        spec: String,
        status: Option<String>,
        resource_version: i64,
        generation: i64,
        created_at: String,
        updated_at: String,
    ) -> Result<StoredObject, AppError> {
        let spec_value: Value =
            serde_json::from_str(&spec).map_err(|e| AppError::Internal(e.into()))?;
        let status_value: Option<SpecData> = status
            .map(|s| {
                serde_json::from_str(&s)
                    .map(|v| SpecData { value: v })
                    .map_err(|e| AppError::Internal(e.into()))
            })
            .transpose()?;
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
                generation: generation as u64,
                created_at: created_at.with_timezone(&Utc),
                updated_at: updated_at.with_timezone(&Utc),
            },
            spec: SpecData { value: spec_value },
            status: status_value,
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

    /// Fetch an object while holding the connection lock.
    fn fetch_object_locked(
        conn: &Connection,
        key: &ResourceKey,
        name: &str,
    ) -> Result<StoredObject, AppError> {
        let mut stmt = conn
            .prepare(
                "SELECT resource_group, api_version, resource_kind, name, spec, status, \
                 resource_version, generation, created_at, updated_at \
                 FROM objects WHERE resource_group = ?1 AND api_version = ?2 \
                 AND resource_kind = ?3 AND name = ?4",
            )
            .map_err(|e| AppError::Internal(e.into()))?;
        let mut obj = stmt
            .query_row(params![key.group, key.version, key.kind, name], row_to_object)
            .optional()
            .map_err(|e| AppError::Internal(e.into()))?;

        match obj {
            Some(ref mut obj) => {
                obj.metadata.labels =
                    Self::query_labels(conn, &key.group, &key.version, &key.kind, name)?;
                Ok(obj.clone())
            }
            None => Err(AppError::NotFound {
                what: "object".to_string(),
                identifier: format!("{}/{}", key.kind, name),
            }),
        }
    }

    /// Persist an object while holding the connection lock.
    /// Replaces the existing object and its labels.
    fn persist_object_locked(
        conn: &Connection,
        object: &StoredObject,
    ) -> Result<(), AppError> {
        let spec_json =
            serde_json::to_string(&object.spec.value).map_err(|e| AppError::Internal(e.into()))?;
        let status_json = object
            .status
            .as_ref()
            .map(|s| serde_json::to_string(&s.value))
            .transpose()
            .map_err(|e| AppError::Internal(e.into()))?;
        let created_at = object.system.created_at.to_rfc3339();
        let updated_at = object.system.updated_at.to_rfc3339();

        let tx = conn
            .unchecked_transaction()
            .map_err(|e| AppError::Internal(e.into()))?;

        tx.execute(
            "INSERT OR REPLACE INTO objects \
             (resource_group, api_version, resource_kind, name, spec, status, \
              resource_version, generation, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                object.key.group,
                object.key.version,
                object.key.kind,
                object.metadata.name,
                spec_json,
                status_json,
                object.system.resource_version as i64,
                object.system.generation as i64,
                created_at,
                updated_at,
            ],
        )
        .map_err(|e| AppError::Internal(e.into()))?;

        // Full label replacement: delete all existing, insert new
        tx.execute(
            "DELETE FROM labels WHERE resource_group = ?1 AND api_version = ?2 \
             AND resource_kind = ?3 AND name = ?4",
            params![
                object.key.group,
                object.key.version,
                object.key.kind,
                object.metadata.name,
            ],
        )
        .map_err(|e| AppError::Internal(e.into()))?;

        if !object.metadata.labels.is_empty() {
            let mut stmt = tx
                .prepare(
                    "INSERT INTO labels (resource_group, api_version, resource_kind, \
                     name, label_key, label_value) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                )
                .map_err(|e| AppError::Internal(e.into()))?;
            for (label_key, label_value) in &object.metadata.labels {
                stmt.execute(params![
                    object.key.group,
                    object.key.version,
                    object.key.kind,
                    object.metadata.name,
                    label_key,
                    label_value,
                ])
                .map_err(|e| AppError::Internal(e.into()))?;
            }
        }

        tx.commit().map_err(|e| AppError::Internal(e.into()))?;
        Ok(())
    }

    /// Delete an object while holding the connection lock.
    /// Labels are removed via ON DELETE CASCADE.
    fn delete_object_locked(
        conn: &Connection,
        key: &ResourceKey,
        name: &str,
    ) -> Result<(), AppError> {
        let rows = conn
            .execute(
                "DELETE FROM objects WHERE resource_group = ?1 AND api_version = ?2 \
                 AND resource_kind = ?3 AND name = ?4",
                params![key.group, key.version, key.kind, name],
            )
            .map_err(|e| AppError::Internal(e.into()))?;
        if rows == 0 {
            return Err(AppError::NotFound {
                what: "object".to_string(),
                identifier: format!("{}/{}", key.kind, name),
            });
        }
        Ok(())
    }
}

/// Maps a rusqlite row to `StoredObject`. Used as the row callback in `query_row` / `query_map`.
fn row_to_object(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredObject> {
    let group: String = row.get("resource_group")?;
    let version: String = row.get("api_version")?;
    let kind: String = row.get("resource_kind")?;
    let name: String = row.get("name")?;
    let spec: String = row.get("spec")?;
    let status: Option<String> = row.get("status")?;
    let resource_version: i64 = row.get("resource_version")?;
    let generation: i64 = row.get("generation")?;
    let created_at: String = row.get("created_at")?;
    let updated_at: String = row.get("updated_at")?;
    SQLiteStore::deserialize_row(
        group,
        version,
        kind,
        name,
        spec,
        status,
        resource_version,
        generation,
        created_at,
        updated_at,
    )
    .map_err(|e| rusqlite::Error::ToSqlConversionFailure(e.into()))
}

#[async_trait]
impl ObjectStore for SQLiteStore {
    /// Inserts a new object. Returns `Conflict` if the composite key already exists.
    /// Labels are inserted into the `labels` table within the same transaction.
    /// Does NOT modify system metadata — persists the object as-is.
    async fn create(&self, object: StoredObject) -> Result<StoredObject, AppError> {
        let conn = Arc::clone(&self.conn);

        tokio::task::spawn_blocking(move || {
            let spec_json =
                serde_json::to_string(&object.spec.value).map_err(|e| AppError::Internal(e.into()))?;
            let status_json = object
                .status
                .as_ref()
                .map(|s| serde_json::to_string(&s.value))
                .transpose()
                .map_err(|e| AppError::Internal(e.into()))?;
            let created_at = object.system.created_at.to_rfc3339();
            let updated_at = object.system.updated_at.to_rfc3339();

            let c = conn.lock().unwrap();

            // Use immediate transaction for atomicity of object + labels
            let tx = c.unchecked_transaction().map_err(|e| AppError::Internal(e.into()))?;

            let result = tx.execute(
                "INSERT INTO objects (resource_group, api_version, resource_kind, name, spec, status, resource_version, generation, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    object.key.group, object.key.version, object.key.kind, object.metadata.name,
                    spec_json, status_json,
                    object.system.resource_version as i64, object.system.generation as i64,
                    created_at, updated_at
                ],
            );

            match result {
                Ok(_) => {
                    // Insert labels if non-empty
                    if !object.metadata.labels.is_empty() {
                        let mut stmt = tx.prepare(
                            "INSERT INTO labels (resource_group, api_version, resource_kind, name, label_key, label_value)
                             VALUES (?1, ?2, ?3, ?4, ?5, ?6)"
                        ).map_err(|e| AppError::Internal(e.into()))?;
                        for (label_key, label_value) in &object.metadata.labels {
                            stmt.execute(params![
                                object.key.group, object.key.version, object.key.kind, object.metadata.name,
                                label_key, label_value
                            ]).map_err(|e| AppError::Internal(e.into()))?;
                        }
                    }

                    tx.commit().map_err(|e| AppError::Internal(e.into()))?;

                    Ok(object)
                }
                Err(rusqlite::Error::SqliteFailure(err, _))
                    if err.code == rusqlite::ErrorCode::ConstraintViolation =>
                {
                    // Primary key conflict → duplicate
                    Err(AppError::AlreadyExists {
                        kind: object.key.kind.clone(),
                        name: object.metadata.name.clone(),
                    })
                }
                Err(e) => Err(AppError::Internal(e.into())),
            }
        })
        .await
        .map_err(|e| AppError::Internal(e.into()))?
    }

    /// Atomic read-modify-write transaction using connection-level locking.
    ///
    /// The callback MUST be fast and non-blocking — it runs while holding the
    /// connection lock.
    fn transaction(
        &self,
        key: &ResourceKey,
        name: &str,
        op: Box<dyn FnOnce(&StoredObject) -> TransactionOp + Send>,
    ) -> Result<StoredObject, AppError> {
        // Acquire exclusive lock on the connection.
        // The lock is held for the entire transaction (read → callback → write).
        let conn = self.conn.lock().unwrap();

        // Read existing object (blocking SQLite call, but we hold the lock)
        let existing = Self::fetch_object_locked(&conn, key, name)?;

        // Execute callback (MUST be fast — no I/O allowed)
        match op(&existing) {
            TransactionOp::Apply(new_obj) => {
                // Store persists the object as-is — no metadata modifications.
                // The caller (service layer) is responsible for setting all
                // system metadata before returning Apply.
                Self::persist_object_locked(&conn, &new_obj)?;
                Ok(new_obj)
            }
            TransactionOp::Delete => {
                // Capture deleted object before removing
                let deleted = existing.clone();

                // Hard delete (blocking SQLite call)
                Self::delete_object_locked(&conn, key, name)?;
                Ok(deleted)
            }
            TransactionOp::Abort(err) => {
                // Return error without modifying anything
                Err(err)
            }
        }
        // Lock is released here when `conn` goes out of scope
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
                "SELECT resource_group, api_version, resource_kind, name, spec, status, resource_version, generation, created_at, updated_at
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
    /// Applies field_selector and label_selector filters before pagination.
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

            // Build dynamic SQL with filter WHERE clauses
            let base_where =
                "resource_group = ?1 AND api_version = ?2 AND resource_kind = ?3".to_string();

            let mut where_clauses = vec![base_where];
            let mut params_vec: Vec<Box<dyn rusqlite::types::ToSql>> = vec![
                Box::new(key.group.clone()),
                Box::new(key.version.clone()),
                Box::new(key.kind.clone()),
            ];
            let mut param_idx: usize = 4; // next parameter index

            // Add continue_token skip condition
            if let Some(ref skip) = skip_past {
                where_clauses.push(format!("name > ?{}", param_idx));
                params_vec.push(Box::new(skip.clone()));
                param_idx += 1;
            }

            // Add field_selector filter
            if let Some(ref selector) = opts.field_selector {
                match selector {
                    FieldSelector::NameEquals(name) => {
                        where_clauses.push(format!("name = ?{}", param_idx));
                        params_vec.push(Box::new(name.clone()));
                        param_idx += 1;
                    }
                }
            }

            // Add label_selector filters using EXISTS subqueries
            if let Some(ref selector) = opts.label_selector {
                for req in &selector.requirements {
                    match req {
                        LabelRequirement::Equals { key: k, value: v } => {
                            let clause = format!(
                                "EXISTS (SELECT 1 FROM labels l WHERE l.resource_group = o.resource_group \
                                 AND l.api_version = o.api_version AND l.resource_kind = o.resource_kind \
                                 AND l.name = o.name AND l.label_key = ?{} AND l.label_value = ?{})",
                                param_idx,
                                param_idx + 1
                            );
                            where_clauses.push(clause);
                            params_vec.push(Box::new(k.clone()));
                            params_vec.push(Box::new(v.clone()));
                            param_idx += 2;
                        }
                        LabelRequirement::NotEquals { key: k, value: v } => {
                            // NOT EXISTS (label key) OR EXISTS (label key with different value)
                            let clause = format!(
                                "(NOT EXISTS (SELECT 1 FROM labels l WHERE l.resource_group = o.resource_group \
                                 AND l.api_version = o.api_version AND l.resource_kind = o.resource_kind \
                                 AND l.name = o.name AND l.label_key = ?{}) \
                                 OR EXISTS (SELECT 1 FROM labels l WHERE l.resource_group = o.resource_group \
                                 AND l.api_version = o.api_version AND l.resource_kind = o.resource_kind \
                                 AND l.name = o.name AND l.label_key = ?{} AND l.label_value != ?{}))",
                                param_idx, param_idx + 1, param_idx + 2
                            );
                            where_clauses.push(clause);
                            params_vec.push(Box::new(k.clone()));
                            params_vec.push(Box::new(k.clone()));
                            params_vec.push(Box::new(v.clone()));
                            param_idx += 3;
                        }
                        LabelRequirement::Exists { key: k } => {
                            let clause = format!(
                                "EXISTS (SELECT 1 FROM labels l WHERE l.resource_group = o.resource_group \
                                 AND l.api_version = o.api_version AND l.resource_kind = o.resource_kind \
                                 AND l.name = o.name AND l.label_key = ?{})",
                                param_idx
                            );
                            where_clauses.push(clause);
                            params_vec.push(Box::new(k.clone()));
                            param_idx += 1;
                        }
                        LabelRequirement::NotExists { key: k } => {
                            let clause = format!(
                                "NOT EXISTS (SELECT 1 FROM labels l WHERE l.resource_group = o.resource_group \
                                 AND l.api_version = o.api_version AND l.resource_kind = o.resource_kind \
                                 AND l.name = o.name AND l.label_key = ?{})",
                                param_idx
                            );
                            where_clauses.push(clause);
                            params_vec.push(Box::new(k.clone()));
                            param_idx += 1;
                        }
                    }
                }
            }

            let where_sql = where_clauses.join(" AND ");
            let limit_param_idx = param_idx;

            // Add limit parameter
            params_vec.push(Box::new(query_limit as i64));

            let sql = format!(
                "SELECT o.resource_group, o.api_version, o.resource_kind, o.name, o.spec, o.status, \
                 o.resource_version, o.generation, o.created_at, o.updated_at \
                 FROM objects o \
                 WHERE {where_sql} \
                 ORDER BY o.name ASC \
                 LIMIT ?{limit_param_idx}"
            );

            let mut stmt = c.prepare(&sql).map_err(|e| AppError::Internal(e.into()))?;

            let params_refs: Vec<&dyn rusqlite::types::ToSql> =
                params_vec.iter().map(|b| b.as_ref()).collect();

            let rows = stmt
                .query_map(params_refs.as_slice(), row_to_object)
                .map_err(|e| AppError::Internal(e.into()))?;

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

    /// Checks whether any objects exist for the given resource key.
    async fn exists(&self, key: &ResourceKey) -> Result<bool, AppError> {
        let key = key.clone();
        let conn = Arc::clone(&self.conn);
        tokio::task::spawn_blocking(move || {
            let c = conn.lock().unwrap();
            let count: i64 = c.query_row(
                "SELECT EXISTS(SELECT 1 FROM objects WHERE resource_group = ?1 AND api_version = ?2 AND resource_kind = ?3)",
                params![key.group, key.version, key.kind],
                |row| row.get(0),
            ).map_err(|e| AppError::Internal(e.into()))?;
            Ok(count == 1)
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
    use crate::object::types::LabelSelector;
    use serde_json::json;

    /// Helper to construct a stored object with initial metadata for tests.
    fn test_obj(key: ResourceKey, name: &str, spec: serde_json::Value) -> StoredObject {
        StoredObject {
            key,
            metadata: ObjectMeta {
                name: name.to_string(),
                labels: HashMap::new(),
            },
            system: SystemMetadata::initial(),
            spec: SpecData { value: spec },
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
            .create(test_obj(key.clone(), "my-widget", data.clone()))
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
        let (store, _dir) = temp_store();
        let key = test_key();

        store
            .create(test_obj(key.clone(), "my-widget", json!({"x": 1})))
            .await
            .unwrap();

        let err = store
            .create(test_obj(key.clone(), "my-widget", json!({"x": 2})))
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
            .create(test_obj(key.clone(), "c", json!({})))
            .await
            .unwrap();
        store
            .create(test_obj(key.clone(), "a", json!({})))
            .await
            .unwrap();
        store
            .create(test_obj(key.clone(), "b", json!({})))
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
        let (store, _dir) = temp_store();
        let key = test_key();

        for i in 0..5 {
            store
                .create(test_obj(key.clone(), &format!("item-{i}"), json!({})))
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
        let (store, _dir) = temp_store();
        let key = test_key();

        for i in 0..5 {
            store
                .create(test_obj(key.clone(), &format!("item-{i}"), json!({})))
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
        let (store, _dir) = temp_store();
        let key = test_key();
        let created = store
            .create(test_obj(key.clone(), "my-widget", json!({"x": 1})))
            .await
            .unwrap();
        let v1 = created.system.resource_version;

        let updated = store
            .transaction(&key, "my-widget", Box::new(|existing| {
                let mut updated = existing.clone();
                updated.spec.value = json!({"x": 2});
                // Caller bumps resource_version
                updated.system.resource_version = existing.system.resource_version + 1;
                TransactionOp::Apply(updated)
            }))
            .unwrap();

        assert_eq!(updated.system.resource_version, v1 + 1);
        assert_eq!(updated.spec.value, json!({"x": 2}));
    }

    #[tokio::test]
    async fn update_missing_not_found() {
        let (store, _dir) = temp_store();
        let key = test_key();

        let err = store
            .transaction(&key, "nonexistent", Box::new(|_| TransactionOp::Delete))
            .unwrap_err();
        assert!(matches!(err, AppError::NotFound { .. }));
    }

    #[tokio::test]
    async fn delete_returns_object_and_get_not_found() {
        let (store, _dir) = temp_store();
        let key = test_key();
        let created = store
            .create(test_obj(key.clone(), "my-widget", json!({"x": 1})))
            .await
            .unwrap();

        let deleted = store
            .transaction(&key, "my-widget", Box::new(|_| TransactionOp::Delete))
            .unwrap();
        assert_eq!(deleted.metadata.name, created.metadata.name);

        let err = store.get(&key, "my-widget").await.unwrap_err();
        assert!(matches!(err, AppError::NotFound { .. }));
    }

    #[tokio::test]
    async fn delete_missing_not_found() {
        let (store, _dir) = temp_store();
        let key = test_key();

        let err = store
            .transaction(&key, "nonexistent", Box::new(|_| TransactionOp::Delete))
            .unwrap_err();
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
                    ..Default::default()
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
                .create(test_obj(key.clone(), "persistent", json!({"data": "hello"})))
                .await
                .unwrap();
        }

        {
            let store = SQLiteStore::new(db_path.to_str().unwrap()).unwrap();
            let key = test_key();
            let retrieved = store.get(&key, "persistent").await.unwrap();
            assert_eq!(retrieved.spec.value, json!({"data": "hello"}));
        }
    }

    // --- Filtering tests ---

    #[tokio::test]
    async fn list_with_field_selector() {
        let (store, _dir) = temp_store();
        let key = test_key();

        store
            .create(test_obj(key.clone(), "foo", json!({})))
            .await
            .unwrap();
        store
            .create(test_obj(key.clone(), "bar", json!({})))
            .await
            .unwrap();
        store
            .create(test_obj(key.clone(), "baz", json!({})))
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
    async fn list_with_label_selector_equals() {
        let (store, _dir) = temp_store();
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

        store
            .create(test_obj(key.clone(), "web-3", json!({})))
            .await
            .unwrap();

        let res = store
            .list(
                &key,
                ListOptions {
                    label_selector: Some(LabelSelector {
                        requirements: vec![LabelRequirement::Equals {
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
    async fn list_with_label_selector_not_equals() {
        let (store, _dir) = temp_store();
        let key = test_key();

        let mut labels = HashMap::new();
        labels.insert("env".to_string(), "prod".to_string());
        let mut obj = test_obj(key.clone(), "prod-app", json!({}));
        obj.metadata.labels = labels;
        store.create(obj).await.unwrap();

        let mut labels2 = HashMap::new();
        labels2.insert("env".to_string(), "staging".to_string());
        let mut obj = test_obj(key.clone(), "staging-app", json!({}));
        obj.metadata.labels = labels2;
        store.create(obj).await.unwrap();

        store
            .create(test_obj(key.clone(), "no-env-app", json!({})))
            .await
            .unwrap();

        let res = store
            .list(
                &key,
                ListOptions {
                    label_selector: Some(LabelSelector {
                        requirements: vec![LabelRequirement::NotEquals {
                            key: "env".to_string(),
                            value: "prod".to_string(),
                        }],
                    }),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        // Should match: staging-app (env!=prod) and no-env-app (no env label)
        assert_eq!(res.items.len(), 2);
    }

    #[tokio::test]
    async fn list_with_label_selector_exists() {
        let (store, _dir) = temp_store();
        let key = test_key();

        let mut labels = HashMap::new();
        labels.insert("gpu".to_string(), "true".to_string());
        let mut obj = test_obj(key.clone(), "gpu-node", json!({}));
        obj.metadata.labels = labels;
        store.create(obj).await.unwrap();

        store
            .create(test_obj(key.clone(), "cpu-node", json!({})))
            .await
            .unwrap();

        let res = store
            .list(
                &key,
                ListOptions {
                    label_selector: Some(LabelSelector {
                        requirements: vec![LabelRequirement::Exists {
                            key: "gpu".to_string(),
                        }],
                    }),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert_eq!(res.items.len(), 1);
        assert_eq!(res.items[0].metadata.name, "gpu-node");
    }

    #[tokio::test]
    async fn list_with_label_selector_not_exists() {
        let (store, _dir) = temp_store();
        let key = test_key();

        let mut labels = HashMap::new();
        labels.insert("experimental".to_string(), "true".to_string());
        let mut obj = test_obj(key.clone(), "exp-app", json!({}));
        obj.metadata.labels = labels;
        store.create(obj).await.unwrap();

        store
            .create(test_obj(key.clone(), "stable-app", json!({})))
            .await
            .unwrap();

        let res = store
            .list(
                &key,
                ListOptions {
                    label_selector: Some(LabelSelector {
                        requirements: vec![LabelRequirement::NotExists {
                            key: "experimental".to_string(),
                        }],
                    }),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert_eq!(res.items.len(), 1);
        assert_eq!(res.items[0].metadata.name, "stable-app");
    }

    #[tokio::test]
    async fn list_with_both_selectors() {
        let (store, _dir) = temp_store();
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

        store
            .create(test_obj(key.clone(), "target-nolabel", json!({})))
            .await
            .unwrap();

        let res = store
            .list(
                &key,
                ListOptions {
                    field_selector: Some(FieldSelector::NameEquals("target".to_string())),
                    label_selector: Some(LabelSelector {
                        requirements: vec![LabelRequirement::Equals {
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
        let (store, _dir) = temp_store();
        let key = test_key();

        // Create 50 objects, only 3 match the filter
        for i in 0..50 {
            let mut obj = test_obj(key.clone(), &format!("obj-{i:02}"), json!({}));
            if i < 3 {
                obj.metadata.labels.insert("app".to_string(), "nginx".to_string());
            }
            store.create(obj).await.unwrap();
        }

        // Filter to 3, limit 10 → should return 3
        let res = store
            .list(
                &key,
                ListOptions {
                    label_selector: Some(LabelSelector {
                        requirements: vec![LabelRequirement::Equals {
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
        let (store, _dir) = temp_store();
        let key = test_key();

        store
            .create(test_obj(key.clone(), "foo", json!({})))
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

    #[tokio::test]
    async fn list_with_multiple_label_requirements() {
        let (store, _dir) = temp_store();
        let key = test_key();

        let mut labels = HashMap::new();
        labels.insert("app".to_string(), "nginx".to_string());
        labels.insert("env".to_string(), "prod".to_string());
        let mut obj = test_obj(key.clone(), "matching", json!({}));
        obj.metadata.labels = labels;
        store.create(obj).await.unwrap();

        let mut labels2 = HashMap::new();
        labels2.insert("app".to_string(), "nginx".to_string());
        labels2.insert("env".to_string(), "staging".to_string());
        let mut obj = test_obj(key.clone(), "wrong-env", json!({}));
        obj.metadata.labels = labels2;
        store.create(obj).await.unwrap();

        let res = store
            .list(
                &key,
                ListOptions {
                    label_selector: Some(LabelSelector {
                        requirements: vec![
                            LabelRequirement::Equals {
                                key: "app".to_string(),
                                value: "nginx".to_string(),
                            },
                            LabelRequirement::Equals {
                                key: "env".to_string(),
                                value: "prod".to_string(),
                            },
                        ],
                    }),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert_eq!(res.items.len(), 1);
        assert_eq!(res.items[0].metadata.name, "matching");
    }

    // --- exists tests ---

    #[tokio::test]
    async fn exists_returns_true_when_objects_present() {
        let (store, _dir) = temp_store();
        let key = test_key();

        store
            .create(test_obj(key.clone(), "exists-test", json!({"x": 1})))
            .await
            .unwrap();

        assert!(store.exists(&key).await.unwrap());
    }

    #[tokio::test]
    async fn exists_returns_false_when_no_objects() {
        let (store, _dir) = temp_store();
        let key = test_key();

        assert!(!store.exists(&key).await.unwrap());
    }

    #[tokio::test]
    async fn exists_returns_false_for_different_key() {
        let (store, _dir) = temp_store();
        let key = test_key();

        store
            .create(test_obj(key.clone(), "test", json!({})))
            .await
            .unwrap();

        let other_key = ResourceKey {
            group: "other.io".to_string(),
            version: "v1".to_string(),
            kind: "Other".to_string(),
        };
        assert!(!store.exists(&other_key).await.unwrap());
    }

    // --- update_status tests (rewritten using transaction) ---

    #[tokio::test]
    async fn update_status_success() {
        let (store, _dir) = temp_store();
        let key = test_key();
        let created = store
            .create(test_obj(key.clone(), "my-widget", json!({"color": "blue"})))
            .await
            .unwrap();
        assert!(created.status.is_none());
        let v1 = created.system.resource_version;

        let updated = store
            .transaction(&key, "my-widget", Box::new(|existing| {
                let mut updated = existing.clone();
                updated.status = Some(SpecData { value: json!({"phase": "Running"}) });
                // Caller bumps resource_version
                updated.system.resource_version = existing.system.resource_version + 1;
                TransactionOp::Apply(updated)
            }))
            .unwrap();

        assert!(updated.status.is_some());
        assert_eq!(updated.status.unwrap().value, json!({"phase": "Running"}));
        assert_eq!(updated.system.resource_version, v1 + 1);
        assert_eq!(updated.spec.value, json!({"color": "blue"}));
    }

    #[tokio::test]
    async fn update_status_not_found() {
        let (store, _dir) = temp_store();
        let key = test_key();

        let err = store
            .transaction(&key, "nonexistent", Box::new(|_| TransactionOp::Delete))
            .unwrap_err();
        assert!(matches!(err, AppError::NotFound { .. }));
    }

    #[tokio::test]
    async fn update_status_replaces_existing_status() {
        let (store, _dir) = temp_store();
        let key = test_key();

        store
            .create(test_obj(key.clone(), "my-widget", json!({"color": "blue"})))
            .await
            .unwrap();

        store
            .transaction(&key, "my-widget", Box::new(|existing| {
                let mut updated = existing.clone();
                updated.status = Some(SpecData { value: json!({"phase": "Pending"}) });
                TransactionOp::Apply(updated)
            }))
            .unwrap();

        let updated = store
            .transaction(&key, "my-widget", Box::new(|existing| {
                let mut updated = existing.clone();
                updated.status = Some(SpecData { value: json!({"phase": "Running"}) });
                TransactionOp::Apply(updated)
            }))
            .unwrap();
        assert_eq!(updated.status.unwrap().value, json!({"phase": "Running"}));
    }

    #[tokio::test]
    async fn update_status_bumps_resource_version() {
        let (store, _dir) = temp_store();
        let key = test_key();

        let created = store
            .create(test_obj(key.clone(), "my-widget", json!({"x": 1})))
            .await
            .unwrap();
        let v1 = created.system.resource_version;

        let updated = store
            .transaction(&key, "my-widget", Box::new(|existing| {
                let mut updated = existing.clone();
                updated.status = Some(SpecData { value: json!({"phase": "Running"}) });
                // Caller bumps resource_version
                updated.system.resource_version = existing.system.resource_version + 1;
                TransactionOp::Apply(updated)
            }))
            .unwrap();
        assert_eq!(updated.system.resource_version, v1 + 1);
    }

    #[tokio::test]
    async fn update_status_preserves_spec() {
        let (store, _dir) = temp_store();
        let key = test_key();

        store
            .create(test_obj(key.clone(), "my-widget", json!({"color": "blue", "size": 10})))
            .await
            .unwrap();

        let updated = store
            .transaction(&key, "my-widget", Box::new(|existing| {
                let mut updated = existing.clone();
                updated.status = Some(SpecData { value: json!({"phase": "Running"}) });
                TransactionOp::Apply(updated)
            }))
            .unwrap();
        assert_eq!(updated.spec.value, json!({"color": "blue", "size": 10}));
    }

    // --- New transaction-based tests ---

    #[tokio::test]
    async fn transaction_apply_succeeds() {
        let (store, _dir) = temp_store();
        let key = test_key();
        let created = store
            .create(test_obj(key.clone(), "test", json!({"x": 1})))
            .await
            .unwrap();
        let v1 = created.system.resource_version;

        let result = store
            .transaction(&key, "test", Box::new(|existing| {
                let mut updated = existing.clone();
                updated.spec.value = json!({"x": 2});
                // Caller bumps resource_version
                updated.system.resource_version = existing.system.resource_version + 1;
                TransactionOp::Apply(updated)
            }))
            .unwrap();

        assert_eq!(result.system.resource_version, v1 + 1);
        assert_eq!(result.spec.value, json!({"x": 2}));
    }

    #[tokio::test]
    async fn transaction_abort_does_not_modify() {
        let (store, _dir) = temp_store();
        let key = test_key();
        let created = store
            .create(test_obj(key.clone(), "test", json!({"x": 1})))
            .await
            .unwrap();
        let v1 = created.system.resource_version;

        let err = store
            .transaction(
                &key,
                "test",
                Box::new(|_| TransactionOp::Abort(AppError::Internal(anyhow::anyhow!("aborted")))),
            )
            .unwrap_err();
        assert!(matches!(err, AppError::Internal { .. }));

        // Verify object is unmodified
        let retrieved = store.get(&key, "test").await.unwrap();
        assert_eq!(retrieved.system.resource_version, v1);
        assert_eq!(retrieved.spec.value, json!({"x": 1}));
    }

    #[tokio::test]
    async fn transaction_delete_removes_object() {
        let (store, _dir) = temp_store();
        let key = test_key();
        store
            .create(test_obj(key.clone(), "test", json!({"x": 1})))
            .await
            .unwrap();

        store
            .transaction(&key, "test", Box::new(|_| TransactionOp::Delete))
            .unwrap();

        let err = store.get(&key, "test").await.unwrap_err();
        assert!(matches!(err, AppError::NotFound { .. }));
    }
}
