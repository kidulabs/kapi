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
    ContinueToken, FieldSelector, LabelRequirement, LabelSelector, ListOptions, ListResponse,
    ObjectMeta, StoredObject, SystemMetadata,
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
        let store = Self { conn: Arc::new(Mutex::new(conn)) };
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
                namespace          TEXT    NOT NULL DEFAULT '',
                spec               TEXT    NOT NULL,
                status             TEXT,
                annotations        TEXT,
                finalizers         TEXT    NOT NULL DEFAULT '[]',
                resource_version   INTEGER NOT NULL,
                generation         INTEGER NOT NULL DEFAULT 1,
                created_at         TEXT    NOT NULL,
                updated_at         TEXT    NOT NULL,
                deletion_timestamp TEXT,
                PRIMARY KEY (resource_group, api_version, resource_kind, namespace, name)
            )",
            [],
        )
        .map_err(|e| AppError::Internal(e.into()))?;

        // Index is implicit via PK; keep for composite lookups
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_objects_gvknn ON \
             objects(resource_group, api_version, resource_kind, namespace, name)",
            [],
        )
        .map_err(|e| AppError::Internal(e.into()))?;

        // Enable foreign key support (required for ON DELETE CASCADE)
        conn.execute("PRAGMA foreign_keys = ON", []).map_err(|e| AppError::Internal(e.into()))?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS labels (
                resource_group  TEXT NOT NULL,
                api_version     TEXT NOT NULL,
                resource_kind   TEXT NOT NULL,
                name            TEXT NOT NULL,
                namespace       TEXT NOT NULL DEFAULT '',
                label_key       TEXT NOT NULL,
                label_value     TEXT NOT NULL,
                PRIMARY KEY (resource_group, api_version, resource_kind, namespace, name, label_key),
                FOREIGN KEY (resource_group, api_version, resource_kind, namespace, name)
                    REFERENCES objects(resource_group, api_version, resource_kind, namespace, name)
                    ON DELETE CASCADE
            )",
            [],
        )
        .map_err(|e| AppError::Internal(e.into()))?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_labels_gvknn ON \
             labels(resource_group, api_version, resource_kind, namespace, name)",
            [],
        )
        .map_err(|e| AppError::Internal(e.into()))?;
        Ok(())
    }

    /// Converts raw column values from a query row into a `StoredObject`.
    /// Labels are set to empty — callers must populate them via `query_labels()`.
    /// Annotations are deserialized from JSON; NULL maps to empty HashMap.
    /// Finalizers are deserialized from JSON array string.
    #[allow(clippy::too_many_arguments)]
    fn deserialize_row(
        group: String,
        version: String,
        kind: String,
        name: String,
        namespace: Option<String>,
        spec: String,
        status: Option<String>,
        annotations: Option<String>,
        finalizers: Option<String>,
        resource_version: i64,
        generation: i64,
        created_at: String,
        updated_at: String,
        deletion_timestamp: Option<String>,
    ) -> Result<StoredObject, AppError> {
        let spec_value: Value =
            serde_json::from_str(&spec).map_err(|e| AppError::Internal(e.into()))?;
        let status_value: Option<Value> = status
            .map(|s| serde_json::from_str(&s).map_err(|e| AppError::Internal(e.into())))
            .transpose()?;
        let annotations_value: HashMap<String, String> = annotations
            .map(|a| serde_json::from_str(&a).map_err(|e| AppError::Internal(e.into())))
            .transpose()?
            .unwrap_or_default();
        let finalizers_value: Vec<String> = match finalizers {
            Some(f) => serde_json::from_str(&f).map_err(|e| AppError::Internal(e.into()))?,
            None => Vec::new(),
        };
        let created_at =
            DateTime::parse_from_rfc3339(&created_at).map_err(|e| AppError::Internal(e.into()))?;
        let updated_at =
            DateTime::parse_from_rfc3339(&updated_at).map_err(|e| AppError::Internal(e.into()))?;
        let deletion_timestamp_value: Option<DateTime<Utc>> = match deletion_timestamp {
            Some(ts) => {
                let parsed =
                    DateTime::parse_from_rfc3339(&ts).map_err(|e| AppError::Internal(e.into()))?;
                Some(parsed.with_timezone(&Utc))
            }
            None => None,
        };
        // Convert empty string back to None (cluster-scoped)
        let namespace = namespace.filter(|s| !s.is_empty());

        Ok(StoredObject {
            key: ResourceKey { group, version, kind },
            metadata: ObjectMeta {
                name,
                namespace,
                labels: HashMap::new(),
                annotations: annotations_value,
                finalizers: finalizers_value,
            },
            system: SystemMetadata {
                resource_version: resource_version as u64,
                generation: generation as u64,
                created_at: created_at.with_timezone(&Utc),
                updated_at: updated_at.with_timezone(&Utc),
                deletion_timestamp: deletion_timestamp_value,
            },
            spec: spec_value,
            status: status_value,
        })
    }

    /// Queries labels from the `labels` table for a single object.
    /// `namespace` is the Rust representation (None = cluster-scoped).
    /// Internally, cluster-scoped maps to `""` in SQL.
    fn query_labels(
        conn: &Connection,
        group: &str,
        version: &str,
        kind: &str,
        namespace: Option<&str>,
        name: &str,
    ) -> Result<HashMap<String, String>, AppError> {
        let ns_sql = namespace.unwrap_or("");
        let sql = "SELECT label_key, label_value FROM labels \
             WHERE resource_group = ?1 AND api_version = ?2 \
             AND resource_kind = ?3 AND namespace = ?4 AND name = ?5"
            .to_string();
        let params_vec: Vec<Box<dyn rusqlite::types::ToSql>> = vec![
            Box::new(group.to_string()),
            Box::new(version.to_string()),
            Box::new(kind.to_string()),
            Box::new(ns_sql.to_string()),
            Box::new(name.to_string()),
        ];

        let mut stmt = conn.prepare(&sql).map_err(|e| AppError::Internal(e.into()))?;
        let params_refs: Vec<&dyn rusqlite::types::ToSql> =
            params_vec.iter().map(|b| b.as_ref()).collect();
        let rows = stmt
            .query_map(params_refs.as_slice(), |row| {
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

    /// Batch-fetches labels for multiple objects identified by (group, version, kind, namespace, name).
    ///
    /// When `namespace` is `Some`, all objects share the same namespace and the map is keyed by `name`.
    /// When `namespace` is `None` (cross-namespace), the map is keyed by `"ns\x00name"` to disambiguate
    /// objects with the same name in different namespaces.
    fn batch_query_labels(
        conn: &Connection,
        group: &str,
        version: &str,
        kind: &str,
        namespace: Option<&str>,
        names: &[String],
    ) -> Result<HashMap<String, HashMap<String, String>>, AppError> {
        if names.is_empty() {
            return Ok(HashMap::new());
        }

        let placeholders: Vec<String> = (1..=names.len()).map(|i| format!("?{}", i)).collect();
        let next_idx = names.len() + 1;

        let (sql, extra_params): (String, Vec<Box<dyn rusqlite::types::ToSql>>) =
            if let Some(ns) = namespace {
                let sql = format!(
                    "SELECT name, label_key, label_value FROM labels \
                     WHERE resource_group = ?{next_idx} AND api_version = ?{n2} \
                     AND resource_kind = ?{n3} AND namespace = ?{n4} AND name IN ({})",
                    placeholders.join(", "),
                    next_idx = next_idx,
                    n2 = next_idx + 1,
                    n3 = next_idx + 2,
                    n4 = next_idx + 3,
                );
                let extra: Vec<Box<dyn rusqlite::types::ToSql>> = vec![
                    Box::new(group.to_string()),
                    Box::new(version.to_string()),
                    Box::new(kind.to_string()),
                    Box::new(ns.to_string()),
                ];
                (sql, extra)
            } else {
                // Cross-namespace: include namespace in SELECT to build compound key
                let sql = format!(
                    "SELECT namespace, name, label_key, label_value FROM labels \
                     WHERE resource_group = ?{next_idx} AND api_version = ?{n2} \
                     AND resource_kind = ?{n3} AND name IN ({})",
                    placeholders.join(", "),
                    next_idx = next_idx,
                    n2 = next_idx + 1,
                    n3 = next_idx + 2,
                );
                let extra: Vec<Box<dyn rusqlite::types::ToSql>> = vec![
                    Box::new(group.to_string()),
                    Box::new(version.to_string()),
                    Box::new(kind.to_string()),
                ];
                (sql, extra)
            };

        let mut stmt = conn.prepare(&sql).map_err(|e| AppError::Internal(e.into()))?;

        let mut params_vec: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        for name in names {
            params_vec.push(Box::new(name.clone()));
        }
        params_vec.extend(extra_params);

        let params_refs: Vec<&dyn rusqlite::types::ToSql> =
            params_vec.iter().map(|b| b.as_ref()).collect();

        if let Some(_ns) = namespace {
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
        } else {
            // Cross-namespace: key by "ns\x00name"
            let rows = stmt
                .query_map(params_refs.as_slice(), |row| {
                    let ns: Option<String> = row.get(0)?;
                    let name: String = row.get(1)?;
                    let key: String = row.get(2)?;
                    let value: String = row.get(3)?;
                    Ok((ns, name, key, value))
                })
                .map_err(|e| AppError::Internal(e.into()))?;

            let mut result: HashMap<String, HashMap<String, String>> = HashMap::new();
            for row in rows {
                let (ns, name, key, value) = row.map_err(|e| AppError::Internal(e.into()))?;
                let compound_key = Self::compound_label_key(ns.as_deref(), &name);
                result.entry(compound_key).or_default().insert(key, value);
            }
            Ok(result)
        }
    }

    /// Builds a compound key for cross-namespace label lookups: `"ns\x00name"`.
    /// For cluster-scoped objects (namespace=None), the key is `"\x00name"`.
    fn compound_label_key(namespace: Option<&str>, name: &str) -> String {
        format!("{}\x00{}", namespace.unwrap_or_default(), name)
    }

    /// Fetch an object while holding the connection lock.
    fn fetch_object_locked(
        conn: &Connection,
        key: &ResourceKey,
        namespace: Option<&str>,
        name: &str,
    ) -> Result<StoredObject, AppError> {
        let ns_sql = namespace.unwrap_or("");
        let sql = "SELECT resource_group, api_version, resource_kind, name, namespace, \
             spec, status, annotations, finalizers, resource_version, generation, \
             created_at, updated_at, deletion_timestamp \
             FROM objects WHERE resource_group = ?1 AND api_version = ?2 \
             AND resource_kind = ?3 AND namespace = ?4 AND name = ?5"
            .to_string();
        let params_vec: Vec<Box<dyn rusqlite::types::ToSql>> = vec![
            Box::new(key.group.clone()),
            Box::new(key.version.clone()),
            Box::new(key.kind.clone()),
            Box::new(ns_sql.to_string()),
            Box::new(name.to_string()),
        ];

        let mut stmt = conn.prepare(&sql).map_err(|e| AppError::Internal(e.into()))?;
        let params_refs: Vec<&dyn rusqlite::types::ToSql> =
            params_vec.iter().map(|b| b.as_ref()).collect();
        let mut obj = stmt
            .query_row(params_refs.as_slice(), row_to_object)
            .optional()
            .map_err(|e| AppError::Internal(e.into()))?;

        match obj {
            Some(ref mut obj) => {
                obj.metadata.labels =
                    Self::query_labels(conn, &key.group, &key.version, &key.kind, namespace, name)?;
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
    fn persist_object_locked(conn: &Connection, object: &StoredObject) -> Result<(), AppError> {
        let spec_json =
            serde_json::to_string(&object.spec).map_err(|e| AppError::Internal(e.into()))?;
        let status_json = object
            .status
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|e| AppError::Internal(e.into()))?;
        let annotations_json = if object.metadata.annotations.is_empty() {
            None
        } else {
            Some(
                serde_json::to_string(&object.metadata.annotations)
                    .map_err(|e| AppError::Internal(e.into()))?,
            )
        };
        let finalizers_json = serde_json::to_string(&object.metadata.finalizers)
            .map_err(|e| AppError::Internal(e.into()))?;
        let created_at = object.system.created_at.to_rfc3339();
        let updated_at = object.system.updated_at.to_rfc3339();
        let deletion_timestamp =
            object.system.deletion_timestamp.as_ref().map(|dt| dt.to_rfc3339());

        let tx = conn.unchecked_transaction().map_err(|e| AppError::Internal(e.into()))?;

        tx.execute(
            "INSERT OR REPLACE INTO objects \
             (resource_group, api_version, resource_kind, name, namespace, spec, status, \
              annotations, finalizers, resource_version, generation, created_at, \
              updated_at, deletion_timestamp) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                object.key.group,
                object.key.version,
                object.key.kind,
                object.metadata.name,
                object.metadata.namespace.as_deref().unwrap_or_default(),
                spec_json,
                status_json,
                annotations_json,
                finalizers_json,
                object.system.resource_version as i64,
                object.system.generation as i64,
                created_at,
                updated_at,
                deletion_timestamp,
            ],
        )
        .map_err(|e| AppError::Internal(e.into()))?;

        // Full label replacement: delete all existing, insert new
        let ns_sql = object.metadata.namespace.as_deref().unwrap_or("");
        tx.execute(
            "DELETE FROM labels WHERE resource_group = ?1 AND api_version = ?2 \
             AND resource_kind = ?3 AND namespace = ?4 AND name = ?5",
            params![
                object.key.group,
                object.key.version,
                object.key.kind,
                ns_sql,
                object.metadata.name,
            ],
        )
        .map_err(|e| AppError::Internal(e.into()))?;

        if !object.metadata.labels.is_empty() {
            let mut stmt = tx
                .prepare(
                    "INSERT INTO labels (resource_group, api_version, resource_kind, \
                     name, namespace, label_key, label_value) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                )
                .map_err(|e| AppError::Internal(e.into()))?;
            for (label_key, label_value) in &object.metadata.labels {
                stmt.execute(params![
                    object.key.group,
                    object.key.version,
                    object.key.kind,
                    object.metadata.name,
                    object.metadata.namespace.as_deref().unwrap_or_default(),
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
    /// Labels are removed via ON DELETE CASCADE (namespace is NOT NULL in SQL,
    /// so FK enforcement always works).
    fn delete_object_locked(
        conn: &Connection,
        key: &ResourceKey,
        namespace: Option<&str>,
        name: &str,
    ) -> Result<(), AppError> {
        let ns_sql = namespace.unwrap_or("");
        let rows = conn
            .execute(
                "DELETE FROM objects WHERE resource_group = ?1 AND api_version = ?2 \
                 AND resource_kind = ?3 AND namespace = ?4 AND name = ?5",
                params![key.group, key.version, key.kind, ns_sql, name],
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
    let namespace: Option<String> = row.get("namespace")?;
    let spec: String = row.get("spec")?;
    let status: Option<String> = row.get("status")?;
    let annotations: Option<String> = row.get("annotations")?;
    let finalizers: Option<String> = row.get("finalizers")?;
    let resource_version: i64 = row.get("resource_version")?;
    let generation: i64 = row.get("generation")?;
    let created_at: String = row.get("created_at")?;
    let updated_at: String = row.get("updated_at")?;
    let deletion_timestamp: Option<String> = row.get("deletion_timestamp")?;
    SQLiteStore::deserialize_row(
        group,
        version,
        kind,
        name,
        namespace,
        spec,
        status,
        annotations,
        finalizers,
        resource_version,
        generation,
        created_at,
        updated_at,
        deletion_timestamp,
    )
    .map_err(|e| rusqlite::Error::ToSqlConversionFailure(e.into()))
}

/// Builds dynamic SQL WHERE clauses for `SQLiteStore::list()`.
///
/// Encapsulates `where_clauses`, `params`, and `param_idx` tracking
/// to eliminate the manual index management bug class and make query
/// generation testable without a database.
struct ListQueryBuilder {
    where_clauses: Vec<String>,
    params: Vec<Box<dyn rusqlite::types::ToSql>>,
    param_idx: usize,
}

impl ListQueryBuilder {
    /// Initializes with the base WHERE clause for resource key matching.
    ///
    /// Sets up `resource_group = ?1 AND api_version = ?2 AND resource_kind = ?3`
    /// with the corresponding params, and starts `param_idx` at 4 for subsequent
    /// parameter placeholders.
    fn new(key: &ResourceKey) -> Self {
        Self {
            where_clauses: vec![
                "resource_group = ?1 AND api_version = ?2 AND resource_kind = ?3".to_string(),
            ],
            params: vec![
                Box::new(key.group.clone()),
                Box::new(key.version.clone()),
                Box::new(key.kind.clone()),
            ],
            param_idx: 4,
        }
    }

    /// Adds a namespace filter clause.
    ///
    /// When `namespace` is `Some`, adds `namespace = ?N`.
    /// When `None`, adds `namespace IS NULL`.
    fn add_namespace_filter(&mut self, namespace: Option<&str>) {
        match namespace {
            Some(ns) => {
                self.where_clauses.push(format!("namespace = ?{}", self.param_idx));
                self.params.push(Box::new(ns.to_string()));
                self.param_idx += 1;
            }
            None => {
                self.where_clauses.push("namespace IS NULL".to_string());
                // No param needed for IS NULL
            }
        }
    }

    /// Adds a `name > ?N` clause for cursor-based pagination within a namespace.
    fn add_continue_token(&mut self, skip: &str) {
        self.where_clauses.push(format!("name > ?{}", self.param_idx));
        self.params.push(Box::new(skip.to_string()));
        self.param_idx += 1;
    }

    /// Adds a `(namespace > ?N OR (namespace = ?M AND name > ?P))` clause
    /// for cross-namespace cursor-based pagination. Works with NOT NULL namespace
    /// (cluster-scoped = empty string).
    fn add_cross_namespace_continue_token(&mut self, skip_ns: Option<&str>, skip_name: &str) {
        let ns_idx = self.param_idx;
        let ns2_idx = self.param_idx + 1;
        let name_idx = self.param_idx + 2;
        self.where_clauses.push(format!(
            "(namespace > ?{ns_idx} OR (namespace = ?{ns2_idx} AND name > ?{name_idx}))"
        ));
        let ns_val = skip_ns.unwrap_or("");
        self.params.push(Box::new(ns_val.to_string()));
        self.params.push(Box::new(ns_val.to_string()));
        self.params.push(Box::new(skip_name.to_string()));
        self.param_idx += 3;
    }

    /// Adds field selector clauses.
    ///
    /// Currently supports:
    /// - `FieldSelector::NameEquals` — adds `name = ?N`
    fn add_field_selector(&mut self, selector: &FieldSelector) {
        match selector {
            FieldSelector::NameEquals(name) => {
                self.where_clauses.push(format!("name = ?{}", self.param_idx));
                self.params.push(Box::new(name.clone()));
                self.param_idx += 1;
            }
        }
    }

    /// Adds label selector clauses using EXISTS subqueries.
    ///
    /// Each `LabelRequirement` variant maps to a different EXISTS pattern:
    /// - `Equals` — EXISTS with key = value match
    /// - `NotEquals` — NOT EXISTS (key) OR EXISTS (key with different value)
    /// - `Exists` — EXISTS with key match
    /// - `NotExists` — NOT EXISTS with key match
    fn add_label_selector(&mut self, selector: &LabelSelector) {
        for req in &selector.requirements {
            match req {
                LabelRequirement::Equals { key: k, value: v } => {
                    let clause = format!(
                        "EXISTS (SELECT 1 FROM labels l WHERE l.resource_group = o.resource_group \
                         AND l.api_version = o.api_version AND l.resource_kind = o.resource_kind \
                         AND l.namespace = o.namespace AND l.name = o.name \
                         AND l.label_key = ?{} AND l.label_value = ?{})",
                        self.param_idx,
                        self.param_idx + 1
                    );
                    self.where_clauses.push(clause);
                    self.params.push(Box::new(k.clone()));
                    self.params.push(Box::new(v.clone()));
                    self.param_idx += 2;
                }
                LabelRequirement::NotEquals { key: k, value: v } => {
                    let clause = format!(
                        "(NOT EXISTS (SELECT 1 FROM labels l WHERE l.resource_group = o.resource_group \
                         AND l.api_version = o.api_version AND l.resource_kind = o.resource_kind \
                         AND l.namespace = o.namespace AND l.name = o.name \
                         AND l.label_key = ?{}) \
                         OR EXISTS (SELECT 1 FROM labels l WHERE l.resource_group = o.resource_group \
                         AND l.api_version = o.api_version AND l.resource_kind = o.resource_kind \
                         AND l.namespace = o.namespace AND l.name = o.name \
                         AND l.label_key = ?{} AND l.label_value != ?{}))",
                        self.param_idx,
                        self.param_idx + 1,
                        self.param_idx + 2
                    );
                    self.where_clauses.push(clause);
                    self.params.push(Box::new(k.clone()));
                    self.params.push(Box::new(k.clone()));
                    self.params.push(Box::new(v.clone()));
                    self.param_idx += 3;
                }
                LabelRequirement::Exists { key: k } => {
                    let clause = format!(
                        "EXISTS (SELECT 1 FROM labels l WHERE l.resource_group = o.resource_group \
                         AND l.api_version = o.api_version AND l.resource_kind = o.resource_kind \
                         AND l.namespace = o.namespace AND l.name = o.name \
                         AND l.label_key = ?{})",
                        self.param_idx
                    );
                    self.where_clauses.push(clause);
                    self.params.push(Box::new(k.clone()));
                    self.param_idx += 1;
                }
                LabelRequirement::NotExists { key: k } => {
                    let clause = format!(
                        "NOT EXISTS (SELECT 1 FROM labels l WHERE l.resource_group = o.resource_group \
                         AND l.api_version = o.api_version AND l.resource_kind = o.resource_kind \
                         AND l.namespace = o.namespace AND l.name = o.name \
                         AND l.label_key = ?{})",
                        self.param_idx
                    );
                    self.where_clauses.push(clause);
                    self.params.push(Box::new(k.clone()));
                    self.param_idx += 1;
                }
            }
        }
    }

    /// Consumes the builder and returns the joined WHERE SQL, all parameters,
    /// and the next parameter index (for use with the LIMIT parameter).
    fn build(self) -> (String, Vec<Box<dyn rusqlite::types::ToSql>>, usize) {
        (self.where_clauses.join(" AND "), self.params, self.param_idx)
    }
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
                serde_json::to_string(&object.spec).map_err(|e| AppError::Internal(e.into()))?;
            let status_json = object
                .status
                .as_ref()
                .map(serde_json::to_string)
                .transpose()
                .map_err(|e| AppError::Internal(e.into()))?;
            let annotations_json = if object.metadata.annotations.is_empty() {
                None
            } else {
                Some(
                    serde_json::to_string(&object.metadata.annotations)
                        .map_err(|e| AppError::Internal(e.into()))?,
                )
            };
            let finalizers_json =
                serde_json::to_string(&object.metadata.finalizers)
                    .map_err(|e| AppError::Internal(e.into()))?;
            let created_at = object.system.created_at.to_rfc3339();
            let updated_at = object.system.updated_at.to_rfc3339();
            let deletion_timestamp = object
                .system
                .deletion_timestamp
                .as_ref()
                .map(|dt| dt.to_rfc3339());

            let c = conn.lock().unwrap();

            // Use immediate transaction for atomicity of object + labels
            let tx = c.unchecked_transaction().map_err(|e| AppError::Internal(e.into()))?;

            let result = tx.execute(
                "INSERT INTO objects (resource_group, api_version, resource_kind, name, namespace, spec, status, annotations, finalizers, resource_version, generation, created_at, updated_at, deletion_timestamp)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
                params![
                    object.key.group, object.key.version, object.key.kind, object.metadata.name,
                    object.metadata.namespace.as_deref().unwrap_or_default(),
                    spec_json, status_json, annotations_json, finalizers_json,
                    object.system.resource_version as i64, object.system.generation as i64,
                    created_at, updated_at, deletion_timestamp
                ],
            );

            match result {
                Ok(_) => {
                    // Insert labels if non-empty
                    if !object.metadata.labels.is_empty() {
                        let mut stmt = tx.prepare(
                            "INSERT INTO labels (resource_group, api_version, resource_kind, name, namespace, label_key, label_value)
                             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)"
                        ).map_err(|e| AppError::Internal(e.into()))?;
                        for (label_key, label_value) in &object.metadata.labels {
                            stmt.execute(params![
                                object.key.group, object.key.version, object.key.kind, object.metadata.name,
                                object.metadata.namespace.as_deref().unwrap_or_default(), label_key, label_value
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
        namespace: Option<&str>,
        name: &str,
        op: Box<dyn FnOnce(&StoredObject) -> TransactionOp + Send>,
    ) -> Result<StoredObject, AppError> {
        // Acquire exclusive lock on the connection.
        // The lock is held for the entire transaction (read → callback → write).
        let conn = self.conn.lock().unwrap();

        // Read existing object (blocking SQLite call, but we hold the lock)
        let existing = Self::fetch_object_locked(&conn, key, namespace, name)?;

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
                Self::delete_object_locked(&conn, key, namespace, name)?;
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
    async fn get(
        &self,
        key: &ResourceKey,
        namespace: Option<&str>,
        name: &str,
    ) -> Result<StoredObject, AppError> {
        let key = key.clone();
        let namespace = namespace.map(|s| s.to_string());
        let name = name.to_string();
        let conn = Arc::clone(&self.conn);

        tokio::task::spawn_blocking(move || {
            let c = conn.lock().unwrap();
            let ns_sql = namespace.as_deref().unwrap_or("");
            let mut stmt = c
                .prepare(
                    "SELECT resource_group, api_version, resource_kind, name, namespace, \
                     spec, status, annotations, finalizers, resource_version, generation, \
                     created_at, updated_at, deletion_timestamp \
                     FROM objects WHERE resource_group = ?1 AND api_version = ?2 \
                     AND resource_kind = ?3 AND namespace = ?4 AND name = ?5",
                )
                .map_err(|e| AppError::Internal(e.into()))?;

            let mut obj = stmt
                .query_row(params![key.group, key.version, key.kind, ns_sql, name], row_to_object)
                .optional()
                .map_err(|e| AppError::Internal(e.into()))?;

            match obj {
                Some(ref mut obj) => {
                    obj.metadata.labels = SQLiteStore::query_labels(
                        &c,
                        &key.group,
                        &key.version,
                        &key.kind,
                        namespace.as_deref(),
                        &name,
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
    ///
    /// When `namespace` is `Some`, only objects in that namespace are returned (sorted by name).
    /// When `namespace` is `None`, objects across all namespaces are returned (sorted by namespace, name).
    async fn list(
        &self,
        key: &ResourceKey,
        namespace: Option<&str>,
        opts: ListOptions,
    ) -> Result<ListResponse, AppError> {
        let key = key.clone();
        let namespace = namespace.map(|s| s.to_string());
        let conn = Arc::clone(&self.conn);

        tokio::task::spawn_blocking(move || {
            let skip_past = opts.continue_token.as_ref().map(decode_continue_token).transpose()?;

            let limit = opts.limit.unwrap_or(usize::MAX);
            // Fetch one extra to detect if more pages exist
            let query_limit = limit.saturating_add(1);

            let c = conn.lock().unwrap();

            // Build dynamic SQL with filter WHERE clauses using ListQueryBuilder
            let mut builder = ListQueryBuilder::new(&key);

            // Add namespace filter when namespace is Some
            if let Some(ref ns) = namespace {
                builder.add_namespace_filter(Some(ns));
            }

            // Add continue token (different handling for namespace-scoped vs cross-namespace)
            if let Some(ref skip) = skip_past {
                if namespace.is_some() {
                    // Namespace-scoped: skip by name only
                    let skip_name = &skip.1;
                    builder.add_continue_token(skip_name);
                } else {
                    // Cross-namespace: skip by (namespace, name)
                    let skip_ns = skip.0.as_deref();
                    let skip_name = &skip.1;
                    builder.add_cross_namespace_continue_token(skip_ns, skip_name);
                }
            }

            if let Some(ref selector) = opts.field_selector {
                builder.add_field_selector(selector);
            }

            if let Some(ref selector) = opts.label_selector {
                builder.add_label_selector(selector);
            }

            let (where_sql, mut params_vec, limit_param_idx) = builder.build();

            // Add limit parameter
            params_vec.push(Box::new(query_limit as i64));

            let order_clause = if namespace.is_some() {
                "ORDER BY o.name ASC".to_string()
            } else {
                // Cross-namespace: order by namespace then name.
                // Cluster-scoped objects have namespace='' which sorts before any real namespace.
                "ORDER BY o.namespace, o.name ASC".to_string()
            };

            let sql = format!(
                "SELECT o.resource_group, o.api_version, o.resource_kind, o.name, o.namespace, \
                 o.spec, o.status, o.annotations, o.finalizers, o.resource_version, o.generation, \
                 o.created_at, o.updated_at, o.deletion_timestamp \
                 FROM objects o \
                 WHERE {where_sql} \
                 {order_clause} \
                 LIMIT ?{limit_param_idx}"
            );

            let mut stmt = c.prepare(&sql).map_err(|e| AppError::Internal(e.into()))?;

            let params_refs: Vec<&dyn rusqlite::types::ToSql> =
                params_vec.iter().map(|b| b.as_ref()).collect();

            let rows = stmt
                .query_map(params_refs.as_slice(), row_to_object)
                .map_err(|e| AppError::Internal(e.into()))?;

            let items: Vec<StoredObject> =
                rows.collect::<Result<Vec<_>, _>>().map_err(|e| AppError::Internal(e.into()))?;

            let has_more = items.len() > limit;
            let mut items: Vec<StoredObject> =
                if has_more { items[..limit].to_vec() } else { items };

            // Batch-fetch labels for all returned objects
            let names: Vec<String> = items.iter().map(|o| o.metadata.name.clone()).collect();
            let labels_map = SQLiteStore::batch_query_labels(
                &c,
                &key.group,
                &key.version,
                &key.kind,
                namespace.as_deref(),
                &names,
            )?;

            if namespace.is_some() {
                // Namespace-scoped: map keyed by name
                for item in &mut items {
                    if let Some(labels) = labels_map.get(&item.metadata.name) {
                        item.metadata.labels = labels.clone();
                    }
                }
            } else {
                // Cross-namespace: map keyed by compound key "ns\x00name"
                for item in &mut items {
                    let compound_key = SQLiteStore::compound_label_key(
                        item.metadata.namespace.as_deref(),
                        &item.metadata.name,
                    );
                    if let Some(labels) = labels_map.get(&compound_key) {
                        item.metadata.labels = labels.clone();
                    }
                }
            }

            let continue_token = if has_more {
                items.last().map(|last| {
                    encode_continue_token(last.metadata.namespace.as_deref(), &last.metadata.name)
                })
            } else {
                None
            };

            Ok(ListResponse { items, continue_token })
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

/// Decodes a base64-encoded continue token back to (namespace, name).
fn decode_continue_token(token: &ContinueToken) -> Result<(Option<String>, String), AppError> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(&token.0)
        .map_err(|_| AppError::Internal(anyhow::anyhow!("invalid continue token")))?;
    let json: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|_| AppError::Internal(anyhow::anyhow!("invalid continue token")))?;
    let namespace = json.get("namespace").and_then(|v| v.as_str()).map(|s| s.to_string());
    let name = json.get("name").and_then(|v| v.as_str()).ok_or_else(|| {
        AppError::Internal(anyhow::anyhow!("invalid continue token: missing name"))
    })?;
    Ok((namespace, name.to_string()))
}

/// Encodes (namespace, name) into a base64 continue token.
fn encode_continue_token(namespace: Option<&str>, name: &str) -> ContinueToken {
    let json = serde_json::json!({
        "namespace": namespace,
        "name": name
    });
    let encoded = base64::engine::general_purpose::STANDARD.encode(json.to_string());
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
                namespace: None,
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

        let created = store.create(test_obj(key.clone(), "my-widget", data.clone())).await.unwrap();
        assert_eq!(created.metadata.name, "my-widget");
        assert_eq!(created.spec, data);
        assert_eq!(created.key, key);
        assert_eq!(created.system.resource_version, 1);

        let retrieved = store.get(&key, None, "my-widget").await.unwrap();
        assert_eq!(retrieved.metadata.name, created.metadata.name);
        assert_eq!(retrieved.spec, created.spec);
        assert_eq!(retrieved.system.resource_version, created.system.resource_version);
    }

    #[tokio::test]
    async fn create_duplicate_conflict() {
        let (store, _dir) = temp_store();
        let key = test_key();

        store.create(test_obj(key.clone(), "my-widget", json!({"x": 1}))).await.unwrap();

        let err =
            store.create(test_obj(key.clone(), "my-widget", json!({"x": 2}))).await.unwrap_err();
        assert!(matches!(err, AppError::AlreadyExists { .. }));
    }

    #[tokio::test]
    async fn get_missing_not_found() {
        let (store, _dir) = temp_store();
        let key = test_key();

        let err = store.get(&key, None, "nonexistent").await.unwrap_err();
        assert!(matches!(err, AppError::NotFound { .. }));
    }

    #[tokio::test]
    async fn list_sorted_by_name() {
        let (store, _dir) = temp_store();
        let key = test_key();

        store.create(test_obj(key.clone(), "c", json!({}))).await.unwrap();
        store.create(test_obj(key.clone(), "a", json!({}))).await.unwrap();
        store.create(test_obj(key.clone(), "b", json!({}))).await.unwrap();

        let res = store
            .list(
                &key,
                None,
                ListOptions { limit: None, continue_token: None, ..Default::default() },
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
            store.create(test_obj(key.clone(), &format!("item-{i}"), json!({}))).await.unwrap();
        }

        let res = store
            .list(
                &key,
                None,
                ListOptions { limit: Some(2), continue_token: None, ..Default::default() },
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
            store.create(test_obj(key.clone(), &format!("item-{i}"), json!({}))).await.unwrap();
        }

        let first = store
            .list(
                &key,
                None,
                ListOptions { limit: Some(2), continue_token: None, ..Default::default() },
            )
            .await
            .unwrap();
        let token = first.continue_token.unwrap();

        let second = store
            .list(
                &key,
                None,
                ListOptions { limit: Some(2), continue_token: Some(token), ..Default::default() },
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
        let created =
            store.create(test_obj(key.clone(), "my-widget", json!({"x": 1}))).await.unwrap();
        let v1 = created.system.resource_version;

        let updated = store
            .transaction(
                &key,
                None,
                "my-widget",
                Box::new(|existing| {
                    let mut updated = existing.clone();
                    updated.spec = json!({"x": 2});
                    // Caller bumps resource_version
                    updated.system.resource_version = existing.system.resource_version + 1;
                    TransactionOp::Apply(updated)
                }),
            )
            .unwrap();

        assert_eq!(updated.system.resource_version, v1 + 1);
        assert_eq!(updated.spec, json!({"x": 2}));
    }

    #[tokio::test]
    async fn update_missing_not_found() {
        let (store, _dir) = temp_store();
        let key = test_key();

        let err = store
            .transaction(&key, None, "nonexistent", Box::new(|_| TransactionOp::Delete))
            .unwrap_err();
        assert!(matches!(err, AppError::NotFound { .. }));
    }

    #[tokio::test]
    async fn delete_returns_object_and_get_not_found() {
        let (store, _dir) = temp_store();
        let key = test_key();
        let created =
            store.create(test_obj(key.clone(), "my-widget", json!({"x": 1}))).await.unwrap();

        let deleted = store
            .transaction(&key, None, "my-widget", Box::new(|_| TransactionOp::Delete))
            .unwrap();
        assert_eq!(deleted.metadata.name, created.metadata.name);

        let err = store.get(&key, None, "my-widget").await.unwrap_err();
        assert!(matches!(err, AppError::NotFound { .. }));
    }

    #[tokio::test]
    async fn delete_missing_not_found() {
        let (store, _dir) = temp_store();
        let key = test_key();

        let err = store
            .transaction(&key, None, "nonexistent", Box::new(|_| TransactionOp::Delete))
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
                None,
                ListOptions { limit: None, continue_token: None, ..Default::default() },
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
            let retrieved = store.get(&key, None, "persistent").await.unwrap();
            assert_eq!(retrieved.spec, json!({"data": "hello"}));
        }
    }

    // --- Filtering tests ---

    #[tokio::test]
    async fn list_with_field_selector() {
        let (store, _dir) = temp_store();
        let key = test_key();

        store.create(test_obj(key.clone(), "foo", json!({}))).await.unwrap();
        store.create(test_obj(key.clone(), "bar", json!({}))).await.unwrap();
        store.create(test_obj(key.clone(), "baz", json!({}))).await.unwrap();

        let res = store
            .list(
                &key,
                None,
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

        store.create(test_obj(key.clone(), "web-3", json!({}))).await.unwrap();

        let res = store
            .list(
                &key,
                None,
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

        store.create(test_obj(key.clone(), "no-env-app", json!({}))).await.unwrap();

        let res = store
            .list(
                &key,
                None,
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

        store.create(test_obj(key.clone(), "cpu-node", json!({}))).await.unwrap();

        let res = store
            .list(
                &key,
                None,
                ListOptions {
                    label_selector: Some(LabelSelector {
                        requirements: vec![LabelRequirement::Exists { key: "gpu".to_string() }],
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

        store.create(test_obj(key.clone(), "stable-app", json!({}))).await.unwrap();

        let res = store
            .list(
                &key,
                None,
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

        store.create(test_obj(key.clone(), "target-nolabel", json!({}))).await.unwrap();

        let res = store
            .list(
                &key,
                None,
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
                None,
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

        store.create(test_obj(key.clone(), "foo", json!({}))).await.unwrap();

        let res = store
            .list(
                &key,
                None,
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
                None,
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

        store.create(test_obj(key.clone(), "exists-test", json!({"x": 1}))).await.unwrap();

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

        store.create(test_obj(key.clone(), "test", json!({}))).await.unwrap();

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
            .transaction(
                &key,
                None,
                "my-widget",
                Box::new(|existing| {
                    let mut updated = existing.clone();
                    updated.status = Some(json!({"phase": "Running"}));
                    // Caller bumps resource_version
                    updated.system.resource_version = existing.system.resource_version + 1;
                    TransactionOp::Apply(updated)
                }),
            )
            .unwrap();

        assert!(updated.status.is_some());
        assert_eq!(updated.status.unwrap(), json!({"phase": "Running"}));
        assert_eq!(updated.system.resource_version, v1 + 1);
        assert_eq!(updated.spec, json!({"color": "blue"}));
    }

    #[tokio::test]
    async fn update_status_not_found() {
        let (store, _dir) = temp_store();
        let key = test_key();

        let err = store
            .transaction(&key, None, "nonexistent", Box::new(|_| TransactionOp::Delete))
            .unwrap_err();
        assert!(matches!(err, AppError::NotFound { .. }));
    }

    #[tokio::test]
    async fn update_status_replaces_existing_status() {
        let (store, _dir) = temp_store();
        let key = test_key();

        store.create(test_obj(key.clone(), "my-widget", json!({"color": "blue"}))).await.unwrap();

        store
            .transaction(
                &key,
                None,
                "my-widget",
                Box::new(|existing| {
                    let mut updated = existing.clone();
                    updated.status = Some(json!({"phase": "Pending"}));
                    TransactionOp::Apply(updated)
                }),
            )
            .unwrap();

        let updated = store
            .transaction(
                &key,
                None,
                "my-widget",
                Box::new(|existing| {
                    let mut updated = existing.clone();
                    updated.status = Some(json!({"phase": "Running"}));
                    TransactionOp::Apply(updated)
                }),
            )
            .unwrap();
        assert_eq!(updated.status.unwrap(), json!({"phase": "Running"}));
    }

    #[tokio::test]
    async fn update_status_bumps_resource_version() {
        let (store, _dir) = temp_store();
        let key = test_key();

        let created =
            store.create(test_obj(key.clone(), "my-widget", json!({"x": 1}))).await.unwrap();
        let v1 = created.system.resource_version;

        let updated = store
            .transaction(
                &key,
                None,
                "my-widget",
                Box::new(|existing| {
                    let mut updated = existing.clone();
                    updated.status = Some(json!({"phase": "Running"}));
                    // Caller bumps resource_version
                    updated.system.resource_version = existing.system.resource_version + 1;
                    TransactionOp::Apply(updated)
                }),
            )
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
            .transaction(
                &key,
                None,
                "my-widget",
                Box::new(|existing| {
                    let mut updated = existing.clone();
                    updated.status = Some(json!({"phase": "Running"}));
                    TransactionOp::Apply(updated)
                }),
            )
            .unwrap();
        assert_eq!(updated.spec, json!({"color": "blue", "size": 10}));
    }

    // --- New transaction-based tests ---

    #[tokio::test]
    async fn transaction_apply_succeeds() {
        let (store, _dir) = temp_store();
        let key = test_key();
        let created = store.create(test_obj(key.clone(), "test", json!({"x": 1}))).await.unwrap();
        let v1 = created.system.resource_version;

        let result = store
            .transaction(
                &key,
                None,
                "test",
                Box::new(|existing| {
                    let mut updated = existing.clone();
                    updated.spec = json!({"x": 2});
                    // Caller bumps resource_version
                    updated.system.resource_version = existing.system.resource_version + 1;
                    TransactionOp::Apply(updated)
                }),
            )
            .unwrap();

        assert_eq!(result.system.resource_version, v1 + 1);
        assert_eq!(result.spec, json!({"x": 2}));
    }

    #[tokio::test]
    async fn transaction_abort_does_not_modify() {
        let (store, _dir) = temp_store();
        let key = test_key();
        let created = store.create(test_obj(key.clone(), "test", json!({"x": 1}))).await.unwrap();
        let v1 = created.system.resource_version;

        let err = store
            .transaction(
                &key,
                None,
                "test",
                Box::new(|_| TransactionOp::Abort(AppError::Internal(anyhow::anyhow!("aborted")))),
            )
            .unwrap_err();
        assert!(matches!(err, AppError::Internal { .. }));

        // Verify object is unmodified
        let retrieved = store.get(&key, None, "test").await.unwrap();
        assert_eq!(retrieved.system.resource_version, v1);
        assert_eq!(retrieved.spec, json!({"x": 1}));
    }

    #[tokio::test]
    async fn transaction_delete_removes_object() {
        let (store, _dir) = temp_store();
        let key = test_key();
        store.create(test_obj(key.clone(), "test", json!({"x": 1}))).await.unwrap();

        store.transaction(&key, None, "test", Box::new(|_| TransactionOp::Delete)).unwrap();

        let err = store.get(&key, None, "test").await.unwrap_err();
        assert!(matches!(err, AppError::NotFound { .. }));
    }

    // --- Namespace tests ---

    fn test_obj_in_ns(
        key: ResourceKey,
        name: &str,
        namespace: &str,
        spec: serde_json::Value,
    ) -> StoredObject {
        let mut obj = test_obj(key, name, spec);
        obj.metadata.namespace = Some(namespace.to_string());
        obj
    }

    #[tokio::test]
    async fn same_name_different_namespaces_coexist_sqlite() {
        let (store, _dir) = temp_store();
        let key = test_key();

        store.create(test_obj_in_ns(key.clone(), "shared", "ns1", json!({"x": 1}))).await.unwrap();
        store.create(test_obj_in_ns(key.clone(), "shared", "ns2", json!({"x": 2}))).await.unwrap();

        let ns1 = store.get(&key, Some("ns1"), "shared").await.unwrap();
        assert_eq!(ns1.spec, json!({"x": 1}));

        let ns2 = store.get(&key, Some("ns2"), "shared").await.unwrap();
        assert_eq!(ns2.spec, json!({"x": 2}));
    }

    #[tokio::test]
    async fn namespace_list_with_none_returns_all() {
        let (store, _dir) = temp_store();
        let key = test_key();

        store.create(test_obj_in_ns(key.clone(), "a", "ns1", json!({}))).await.unwrap();
        store.create(test_obj_in_ns(key.clone(), "b", "ns2", json!({}))).await.unwrap();
        store.create(test_obj(key.clone(), "cluster-scoped", json!({}))).await.unwrap();

        let res = store
            .list(
                &key,
                None,
                ListOptions { limit: None, continue_token: None, ..Default::default() },
            )
            .await
            .unwrap();
        // all 3 objects should be returned
        assert_eq!(res.items.len(), 3);
    }

    #[tokio::test]
    async fn namespace_list_with_some_returns_only_that_namespace() {
        let (store, _dir) = temp_store();
        let key = test_key();

        store.create(test_obj_in_ns(key.clone(), "a", "ns1", json!({}))).await.unwrap();
        store.create(test_obj_in_ns(key.clone(), "b", "ns2", json!({}))).await.unwrap();
        store.create(test_obj_in_ns(key.clone(), "c", "ns1", json!({}))).await.unwrap();
        store.create(test_obj(key.clone(), "cluster-scoped", json!({}))).await.unwrap();

        let res = store
            .list(
                &key,
                Some("ns1"),
                ListOptions { limit: None, continue_token: None, ..Default::default() },
            )
            .await
            .unwrap();
        assert_eq!(res.items.len(), 2);
        assert_eq!(res.items[0].metadata.name, "a");
        assert_eq!(res.items[1].metadata.name, "c");
    }

    #[tokio::test]
    async fn namespace_cross_namespace_pagination_with_continue_token() {
        let (store, _dir) = temp_store();
        let key = test_key();

        // Create objects across namespaces: ns1/a, ns1/b, ns2/c, ns2/d
        store.create(test_obj_in_ns(key.clone(), "a", "ns1", json!({}))).await.unwrap();
        store.create(test_obj_in_ns(key.clone(), "b", "ns1", json!({}))).await.unwrap();
        store.create(test_obj_in_ns(key.clone(), "c", "ns2", json!({}))).await.unwrap();
        store.create(test_obj_in_ns(key.clone(), "d", "ns2", json!({}))).await.unwrap();

        // First page: namespace=None, limit=2 → sorted by (namespace, name)
        let page1 = store
            .list(
                &key,
                None,
                ListOptions { limit: Some(2), continue_token: None, ..Default::default() },
            )
            .await
            .unwrap();
        assert_eq!(page1.items.len(), 2);
        assert_eq!(page1.items[0].metadata.namespace.as_deref(), Some("ns1"));
        assert_eq!(page1.items[0].metadata.name, "a");
        assert_eq!(page1.items[1].metadata.namespace.as_deref(), Some("ns1"));
        assert_eq!(page1.items[1].metadata.name, "b");
        assert!(page1.continue_token.is_some());

        // Second page: resume with continue_token
        let token = page1.continue_token.unwrap();
        let page2 = store
            .list(
                &key,
                None,
                ListOptions { limit: Some(2), continue_token: Some(token), ..Default::default() },
            )
            .await
            .unwrap();
        assert_eq!(page2.items.len(), 2);
        assert_eq!(page2.items[0].metadata.namespace.as_deref(), Some("ns2"));
        assert_eq!(page2.items[0].metadata.name, "c");
        assert_eq!(page2.items[1].metadata.namespace.as_deref(), Some("ns2"));
        assert_eq!(page2.items[1].metadata.name, "d");
        assert!(page2.continue_token.is_none());
    }

    #[tokio::test]
    async fn namespace_scoped_list_with_continue_token() {
        let (store, _dir) = temp_store();
        let key = test_key();

        store.create(test_obj_in_ns(key.clone(), "a", "ns1", json!({}))).await.unwrap();
        store.create(test_obj_in_ns(key.clone(), "b", "ns1", json!({}))).await.unwrap();
        store.create(test_obj_in_ns(key.clone(), "c", "ns1", json!({}))).await.unwrap();

        let page1 = store
            .list(
                &key,
                Some("ns1"),
                ListOptions { limit: Some(2), continue_token: None, ..Default::default() },
            )
            .await
            .unwrap();
        assert_eq!(page1.items.len(), 2);
        assert_eq!(page1.items[0].metadata.name, "a");
        assert_eq!(page1.items[1].metadata.name, "b");
        assert!(page1.continue_token.is_some());

        let token = page1.continue_token.unwrap();
        let page2 = store
            .list(
                &key,
                Some("ns1"),
                ListOptions { limit: Some(2), continue_token: Some(token), ..Default::default() },
            )
            .await
            .unwrap();
        assert_eq!(page2.items.len(), 1);
        assert_eq!(page2.items[0].metadata.name, "c");
        assert!(page2.continue_token.is_none());
    }

    #[tokio::test]
    async fn namespace_labels_with_labels() {
        let (store, _dir) = temp_store();
        let key = test_key();

        let mut labels = HashMap::new();
        labels.insert("app".to_string(), "nginx".to_string());
        let mut obj = test_obj_in_ns(key.clone(), "web", "ns1", json!({}));
        obj.metadata.labels = labels;
        store.create(obj).await.unwrap();

        // Retrieve and verify labels are set
        let retrieved = store.get(&key, Some("ns1"), "web").await.unwrap();
        assert_eq!(retrieved.metadata.labels.get("app").unwrap(), "nginx");
        assert_eq!(retrieved.metadata.namespace.as_deref(), Some("ns1"));
    }
}
