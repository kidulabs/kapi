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
use futures_util::stream;
use serde::Deserialize;
use serde_json::Value;

use std::collections::HashMap;

use crate::error::AppError;
use crate::object::types::{
    ContinueToken, FieldSelector, ListOptions, ObjectMeta, StoredObject, WatchFilter,
};
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
    #[serde(rename = "fieldSelector")]
    pub field_selector: Option<String>,
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

/// Extracts labels from `metadata.labels` in the request body.
///
/// Returns an empty `HashMap` when `metadata.labels` is absent.
/// Returns an error when `metadata.labels` is present but not an object
/// with string values.
fn extract_labels(body: &Value) -> Result<HashMap<String, String>, AppError> {
    let labels_value = match body.get("metadata").and_then(|m| m.get("labels")) {
        Some(v) => v,
        None => return Ok(HashMap::new()),
    };

    let labels_obj = labels_value
        .as_object()
        .ok_or_else(|| AppError::InvalidLabel("metadata.labels must be an object".to_string()))?;

    let mut labels = HashMap::with_capacity(labels_obj.len());
    for (key, value) in labels_obj {
        let str_value = value.as_str().ok_or_else(|| {
            AppError::InvalidLabel(format!("label value for key '{}' must be a string", key))
        })?;
        labels.insert(key.clone(), str_value.to_string());
    }
    Ok(labels)
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
    // Extract labels from metadata.labels (shared across both paths)
    let labels = extract_labels(&body)?;

    // Branch on kind: Schema objects generate their name from payload fields,
    // while regular objects require a client-supplied metadata.name
    let meta = if path.kind == "Schema" {
        // Schema registration: generate name from targetKind.targetGroup
        let name = extract_schema_name(&body).ok_or_else(|| {
            AppError::InvalidSchema(
                "Schema registration requires targetKind and targetGroup fields".to_string(),
            )
        })?;
        ObjectMeta { name, labels }
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
        ObjectMeta { name, labels }
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

    // Parse fieldSelector if present
    let filter = match &query.field_selector {
        Some(raw) => Some(parse_field_selector(raw)?),
        None => None,
    };

    // Branch on watch parameter
    if query.watch == Some(true) {
        // Return SSE stream with optional fieldSelector filter
        let filter = filter.unwrap_or(WatchFilter::All);
        return Ok(watch(state, key, filter).into_response());
    }

    // fieldSelector on non-watch request returns 400
    if filter.is_some() {
        return Err(AppError::InvalidFieldSelector(
            "fieldSelector is only valid with watch=true".to_string(),
        ));
    }

    // Regular list
    let opts = ListOptions {
        limit: query.limit,
        continue_token: query.continue_token.map(ContinueToken),
    };
    let response = state.object_service().list(key, opts).await?;
    Ok(Json(response).into_response())
}

/// Parses a `fieldSelector` query parameter value into a `WatchFilter`.
///
/// Supports standard syntax: `metadata.name=<value>`.
/// Returns `InvalidFieldSelector` for unsupported fields or malformed input.
pub fn parse_field_selector(raw: &str) -> Result<WatchFilter, AppError> {
    let (field, value) = raw.split_once('=').ok_or_else(|| {
        AppError::InvalidFieldSelector(format!(
            "invalid field selector format: expected 'field=value', got '{raw}'"
        ))
    })?;
    match field {
        "metadata.name" => Ok(WatchFilter::FieldSelector(FieldSelector::NameEquals(
            value.to_string(),
        ))),
        _ => Err(AppError::InvalidFieldSelector(format!(
            "unsupported field '{field}': only 'metadata.name' is supported"
        ))),
    }
}

/// Watch logic — subscribes to event bus and returns SSE stream.
///
/// Maps WatchEvent to axum SSE events with JSON data.
fn watch(
    state: AppState,
    key: ResourceKey,
    filter: WatchFilter,
) -> Sse<impl Stream<Item = Result<Event, std::convert::Infallible>>> {
    tracing::trace!(
        group = %key.group,
        version = %key.version,
        kind = %key.kind,
        "sse watch stream started"
    );

    let stream = state.object_service().subscribe(&key, filter);

    let sse_stream = stream.filter_map(|watch_event| async move {
        let json_data = serde_json::to_string(&watch_event).ok()?;
        Some(Ok(Event::default().event("message").data(json_data)))
    });

    let sse_stream = stream::once(async move {
        tracing::trace!("sse watch stream ended");
        sse_stream
    })
    .flatten();

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_field_selector_valid_metadata_name() {
        let result = parse_field_selector("metadata.name=my-widget");
        assert!(result.is_ok());
        let filter = result.unwrap();
        assert!(matches!(
            filter,
            WatchFilter::FieldSelector(FieldSelector::NameEquals(name)) if name == "my-widget"
        ));
    }

    #[test]
    fn parse_field_selector_unsupported_field() {
        let result = parse_field_selector("metadata.namespace=default");
        assert!(result.is_err());
        assert!(
            matches!(result, Err(AppError::InvalidFieldSelector(msg)) if msg.contains("metadata.namespace"))
        );
    }

    #[test]
    fn parse_field_selector_malformed_input() {
        let result = parse_field_selector("invalid-format");
        assert!(result.is_err());
        assert!(
            matches!(result, Err(AppError::InvalidFieldSelector(msg)) if msg.contains("expected 'field=value'"))
        );
    }

    #[test]
    fn parse_field_selector_empty_value() {
        let result = parse_field_selector("metadata.name=");
        assert!(result.is_ok());
        let filter = result.unwrap();
        assert!(matches!(
            filter,
            WatchFilter::FieldSelector(FieldSelector::NameEquals(name)) if name.is_empty()
        ));
    }
}
