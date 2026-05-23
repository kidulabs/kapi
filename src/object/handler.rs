//! HTTP handlers for object CRUD operations.
//!
//! Handlers are thin — they extract path params, deserialize body, call service, return response.
//! No business logic in handlers.

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::sse::{Event, Sse};
use futures_util::Stream;
use futures_util::StreamExt;
use serde::Deserialize;
use serde_json::Value;

use crate::error::AppError;
use crate::object::types::{ContinueToken, ListOptions, ObjectMeta, StoredObject};
use crate::routes::AppState;
use crate::store::ResourceKey;

/// Path parameters for /apis/{group}/{version}/{kind}
#[derive(Deserialize)]
pub struct ObjectPath {
    pub group: String,
    pub version: String,
    pub kind: String,
}

/// Path parameters for /apis/{group}/{version}/{kind}/{name}
#[derive(Deserialize)]
pub struct ObjectNamePath {
    pub group: String,
    pub version: String,
    pub kind: String,
    pub name: String,
}

/// Query parameters for list/watch endpoint
#[derive(Deserialize)]
pub struct ListQuery {
    pub watch: Option<bool>,
    pub limit: Option<usize>,
    #[serde(rename = "continue")]
    pub continue_token: Option<String>,
}

/// Extracts the schema name from a Schema registration body.
///
/// Reads `targetKind` and `targetGroup` from the JSON body and returns
/// `Some("{targetKind}.{targetGroup}")`. Returns `None` if either field
/// is missing or not a string.
///
/// The generated name is used as the storage key and cache key for the schema,
/// ensuring consistency between the stored name and the cache lookup key.
fn extract_schema_name(body: &Value) -> Option<String> {
    let target_kind = body.get("targetKind")?.as_str()?;
    let target_group = body.get("targetGroup")?.as_str()?;
    Some(format!("{}.{}", target_kind, target_group))
}

/// Creates a new object.
///
/// Extracts group, version, kind from path, deserializes body as JSON,
/// and calls ObjectService::create. Returns 201 Created with the StoredObject.
///
/// For Schema objects (`kind == "Schema"`), the name is generated from
/// `targetKind` and `targetGroup` in the body as `{targetKind}.{targetGroup}`.
/// For non-Schema objects, the name is extracted from `metadata.name`.
pub async fn create(
    State(state): State<AppState>,
    Path(path): Path<ObjectPath>,
    Json(mut body): Json<Value>,
) -> Result<(StatusCode, Json<StoredObject>), AppError> {
    // Branch on kind: Schema objects generate their name from payload fields,
    // while regular objects require a client-supplied metadata.name
    let meta = if path.kind == "Schema" {
        // Schema registration: generate name from targetKind.targetGroup
        let name = extract_schema_name(&body).ok_or_else(|| {
            AppError::InvalidSchema(
                "Schema registration requires targetKind and targetGroup fields".to_string(),
            )
        })?;
        ObjectMeta { name }
    } else {
        // Regular object: extract name from metadata.name
        let name = body
            .get("metadata")
            .and_then(|m| m.get("name"))
            .and_then(|n| n.as_str())
            .ok_or_else(|| {
                AppError::Internal(anyhow::anyhow!("missing metadata.name in request body"))
            })?
            .to_string();
        ObjectMeta { name }
    };

    // Remove metadata from body before passing to service
    // (metadata is a kapi-level concern, not part of the schema/object data)
    if let Some(obj) = body.as_object_mut() {
        obj.remove("metadata");
    }

    let key = ResourceKey {
        group: path.group,
        version: path.version,
        kind: path.kind,
    };

    let stored = state.object_service().create(key, meta, body).await?;
    Ok((StatusCode::CREATED, Json(stored)))
}

/// Gets an object by key and name.
///
/// Extracts path parameters and calls ObjectService::get.
/// Returns 200 OK with the StoredObject.
pub async fn get(
    State(state): State<AppState>,
    Path(path): Path<ObjectNamePath>,
) -> Result<Json<StoredObject>, AppError> {
    let key = ResourceKey {
        group: path.group,
        version: path.version,
        kind: path.kind,
    };

    let stored = state.object_service().get(key, path.name).await?;
    Ok(Json(stored))
}

/// Lists objects or starts a watch stream.
///
/// Checks for ?watch=true query parameter. If present, subscribes to event bus
/// and returns an SSE stream. Otherwise, calls ObjectService::list and returns JSON.
pub async fn list(
    State(state): State<AppState>,
    Path(path): Path<ObjectPath>,
    Query(query): Query<ListQuery>,
) -> Result<axum::response::Response, AppError> {
    let key = ResourceKey {
        group: path.group,
        version: path.version,
        kind: path.kind,
    };

    // Branch on watch parameter
    if query.watch == Some(true) {
        // Return SSE stream
        return Ok(watch(state, key).into_response());
    }

    // Regular list
    let opts = ListOptions {
        limit: query.limit,
        continue_token: query.continue_token.map(ContinueToken),
    };
    let response = state.object_service().list(key, opts).await?;
    Ok(Json(response).into_response())
}

/// Watch logic — subscribes to event bus and returns SSE stream.
///
/// Maps WatchEvent to axum SSE events with JSON data.
fn watch(
    state: AppState,
    key: ResourceKey,
) -> Sse<impl Stream<Item = Result<Event, std::convert::Infallible>>> {
    // Subscribe to watch events for this key via ObjectService
    let stream = state.object_service().subscribe(&key);

    // Map WatchEvent to SSE Event
    let sse_stream = stream.filter_map(|watch_event| async move {
        let json_data = serde_json::to_string(&watch_event).ok()?;
        Some(Ok(Event::default().event("message").data(json_data)))
    });

    Sse::new(sse_stream)
}

/// Updates an object.
///
/// Extracts path parameters, deserializes body as StoredObject,
/// validates URL key/name matches body, calls ObjectService::update.
/// Returns 200 OK with the updated StoredObject.
pub async fn update(
    State(state): State<AppState>,
    Path(path): Path<ObjectNamePath>,
    Json(mut body): Json<StoredObject>,
) -> Result<Json<StoredObject>, AppError> {
    // Validate URL key/name matches the object's key/name
    let url_key = ResourceKey {
        group: path.group.clone(),
        version: path.version.clone(),
        kind: path.kind.clone(),
    };

    if body.key != url_key {
        return Err(AppError::Internal(anyhow::anyhow!(
            "URL key does not match body key"
        )));
    }

    if body.metadata.name != path.name {
        return Err(AppError::Internal(anyhow::anyhow!(
            "URL name '{}' does not match body name '{}'",
            path.name,
            body.metadata.name
        )));
    }

    // Ensure the body object has the correct key and name from URL
    body.key = url_key;
    body.metadata.name = path.name;

    let updated = state.object_service().update(body).await?;
    Ok(Json(updated))
}

/// Deletes an object.
///
/// Extracts path parameters and calls ObjectService::delete.
/// Returns 200 OK with the deleted StoredObject.
pub async fn delete(
    State(state): State<AppState>,
    Path(path): Path<ObjectNamePath>,
) -> Result<Json<StoredObject>, AppError> {
    let key = ResourceKey {
        group: path.group,
        version: path.version,
        kind: path.kind,
    };

    let deleted = state.object_service().delete(key, path.name).await?;
    Ok(Json(deleted))
}
