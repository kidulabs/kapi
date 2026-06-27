//! HTTP handlers for object CRUD operations.
//!
//! Handlers extract parameters from HTTP requests, perform deserialization and structural
//! validation (required fields, type checks), and delegate to the appropriate service.
//! They never access the store, event bus, or schema registry directly. They do not perform
//! domain format validation (labels, annotations, finalizers) — that is the service layer's
//! responsibility.

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
    ContinueToken, FieldSelector, LabelSelector, ListOptions, ObjectMeta, StoredObject, WatchFilter,
};
use crate::routes::AppState;
use crate::schema::SCHEMA_KIND;
use crate::schema::schema_cache_key;
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
    #[serde(rename = "labelSelector")]
    pub label_selector: Option<String>,
}

/// Extracts the schema name from a Schema registration body.
///
/// Reads `targetKind`, `targetGroup`, and `targetVersion` from the JSON body
/// and returns `Some(schema_cache_key(target_kind, target_group, target_version))`.
/// Returns `None` if any of the three fields is missing or not a string.
///
/// The generated name is used as the storage key and cache key for the schema,
/// ensuring consistency between the stored name and the cache lookup key.
fn extract_schema_name(body: &Value) -> Option<String> {
    let target_kind = body.get("targetKind")?.as_str()?;
    let target_group = body.get("targetGroup")?.as_str()?;
    let target_version = body.get("targetVersion")?.as_str()?;
    Some(schema_cache_key(target_kind, target_group, target_version))
}

/// Extracts annotations from `metadata.annotations` in the request body.
///
/// Returns an empty `HashMap` when `metadata.annotations` is absent.
/// Returns an error when `metadata.annotations` is present but not an object
/// with string values.
///
/// Size validation happens in the service layer (`validate_annotations`), not here.
fn extract_annotations(body: &Value) -> Result<HashMap<String, String>, AppError> {
    let annotations_value = match body.get("metadata").and_then(|m| m.get("annotations")) {
        Some(v) => v,
        None => return Ok(HashMap::new()),
    };

    let annotations_obj = annotations_value.as_object().ok_or_else(|| {
        AppError::InvalidAnnotation("metadata.annotations must be an object".to_string())
    })?;

    let mut annotations = HashMap::with_capacity(annotations_obj.len());
    for (key, value) in annotations_obj {
        let str_value = value.as_str().ok_or_else(|| {
            AppError::InvalidAnnotation(format!(
                "annotation value for key '{}' must be a string",
                key
            ))
        })?;
        annotations.insert(key.clone(), str_value.to_string());
    }
    Ok(annotations)
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

/// Extracts finalizers from `metadata.finalizers` in the request body.
///
/// Returns an empty `Vec` when `metadata.finalizers` is absent.
/// Returns an error when `metadata.finalizers` is present but not an array of strings.
fn extract_finalizers(body: &Value) -> Result<Vec<String>, AppError> {
    let finalizers_value = match body.get("metadata").and_then(|m| m.get("finalizers")) {
        Some(v) => v,
        None => return Ok(Vec::new()),
    };

    let finalizers_arr = finalizers_value.as_array().ok_or_else(|| {
        AppError::InvalidFinalizer("metadata.finalizers must be an array".to_string())
    })?;

    let mut finalizers = Vec::with_capacity(finalizers_arr.len());
    for value in finalizers_arr {
        let str_value = value
            .as_str()
            .ok_or_else(|| AppError::InvalidFinalizer("finalizer must be a string".to_string()))?;
        finalizers.push(str_value.to_string());
    }
    Ok(finalizers)
}

/// Creates a new object.
///
/// Extracts group, version, kind from path, deserializes body as JSON,
/// and calls ObjectService::create. Returns 201 Created with the StoredObject.
///
/// For Schema objects (`kind == SCHEMA_KIND`), the name is generated from
/// `targetKind`, `targetGroup`, and `targetVersion` in the body as
/// `{targetKind}.{targetGroup}.{targetVersion}`, and the full body is passed
/// as the spec data.
///
/// For non-Schema objects:
/// - The name is extracted from `metadata.name`.
/// - The `spec` field is extracted from the body and validated (required, must be
///   a non-empty JSON object).
/// - Only `metadata` and `spec` are allowed as top-level fields; any other fields
///   result in a 400 Bad Request.
/// - The extracted `spec` value is passed to the service as the data to store.
pub async fn create(
    State(state): State<AppState>,
    Path(path): Path<ObjectPath>,
    Json(body): Json<Value>,
) -> Result<(StatusCode, Json<StoredObject>), AppError> {
    // Extract labels, annotations, and finalizers from metadata (shared across both paths)
    let labels = extract_labels(&body)?;
    let annotations = extract_annotations(&body)?;
    let finalizers = extract_finalizers(&body)?;

    // Branch on kind: Schema objects generate their name from payload fields,
    // while regular objects require a client-supplied metadata.name and a spec field
    let (meta, data) = if path.kind == SCHEMA_KIND {
        // Schema registration: generate name from targetKind.targetGroup.targetVersion
        let name = extract_schema_name(&body).ok_or_else(|| {
            AppError::InvalidSchema(
                "Schema registration requires targetKind, targetGroup, and targetVersion fields"
                    .to_string(),
            )
        })?;
        // Strip metadata from body before passing as spec data
        let mut data = body;
        if let Some(obj) = data.as_object_mut() {
            obj.remove("metadata");
        }
        (ObjectMeta { name, labels, annotations, finalizers }, data)
    } else {
        // Validate: only "metadata" and "spec" allowed as top-level fields
        if let Some(obj) = body.as_object() {
            let unknown: Vec<&String> =
                obj.keys().filter(|k| *k != "metadata" && *k != "spec").collect();
            if !unknown.is_empty() {
                let fields = unknown.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ");
                return Err(AppError::InvalidRequestBody(format!("unknown field(s): {fields}")));
            }
        }

        // Regular object: extract name from metadata.name
        let name = body
            .get("metadata")
            .and_then(|m| m.get("name"))
            .and_then(|n| n.as_str())
            .ok_or_else(|| AppError::InvalidRequestBody("'metadata.name' is required".to_string()))?
            .to_string();

        // Extract and validate spec
        let spec = body
            .get("spec")
            .ok_or_else(|| AppError::InvalidRequestBody("'spec' field is required".to_string()))?;

        if !spec.is_object() {
            return Err(AppError::InvalidRequestBody("'spec' must be a JSON object".to_string()));
        }

        if spec.as_object().is_some_and(|o| o.is_empty()) {
            return Err(AppError::InvalidRequestBody("'spec' must not be empty".to_string()));
        }

        (ObjectMeta { name, labels, annotations, finalizers }, spec.clone())
    };

    let key = ResourceKey { group: path.group, version: path.version, kind: path.kind };

    let stored = if key.kind == SCHEMA_KIND {
        state.schema_service().create(key, meta, data).await?
    } else {
        state.object_service().create(key, meta, data).await?
    };
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
    let key = ResourceKey { group: path.group, version: path.version, kind: path.kind };

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
    let key = ResourceKey { group: path.group, version: path.version, kind: path.kind };

    // Parse fieldSelector if present
    let field_filter = match &query.field_selector {
        Some(raw) => Some(FieldSelector::parse(raw)?),
        None => None,
    };

    // Parse labelSelector if present
    let label_filter = match &query.label_selector {
        Some(raw) => Some(LabelSelector::parse(raw)?),
        None => None,
    };

    // Branch on watch parameter
    if query.watch == Some(true) {
        // Combine field and label selectors with WatchFilter::And when both present
        let filter = match (field_filter, label_filter) {
            (Some(f), Some(l)) => WatchFilter::And(Box::new(f), Box::new(l)),
            (Some(f), None) => f,
            (None, Some(l)) => l,
            (None, None) => WatchFilter::All,
        };
        return Ok(watch(state, key, filter).into_response());
    }

    // Regular list with optional selectors
    let opts = ListOptions {
        limit: query.limit,
        continue_token: query.continue_token.map(ContinueToken),
        field_selector: field_filter.map(|f| match f {
            WatchFilter::FieldSelector(fs) => fs,
            _ => unreachable!("field_filter is always FieldSelector"),
        }),
        label_selector: label_filter.map(|l| match l {
            WatchFilter::LabelSelector(ls) => ls,
            _ => unreachable!("label_filter is always LabelSelector"),
        }),
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
        return Err(AppError::Internal(anyhow::anyhow!("URL key does not match body key")));
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

    let updated = if body.key.kind == SCHEMA_KIND {
        state.schema_service().update(body).await?
    } else {
        state.object_service().update(body).await?
    };
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
    let key = ResourceKey { group: path.group, version: path.version, kind: path.kind };

    let deleted = if key.kind == SCHEMA_KIND {
        state.schema_service().delete(key.clone(), path.name.clone()).await?
    } else {
        state.object_service().delete(key, path.name).await?
    };
    Ok(Json(deleted))
}

/// Gets the status subresource of an object.
///
/// Extracts path parameters, calls `object_service.get_status()`,
/// and returns the status value as JSON. Returns 404 if status subresource
/// is not enabled for this kind.
pub async fn get_status(
    State(state): State<AppState>,
    Path(path): Path<ObjectNamePath>,
) -> Result<Json<Option<Value>>, AppError> {
    let key = ResourceKey { group: path.group, version: path.version, kind: path.kind };

    let status = state.object_service().get_status(key, path.name).await?;
    Ok(Json(status))
}

/// Updates the status subresource of an object.
///
/// Extracts path parameters, deserializes body, extracts the `status` field,
/// calls `object_service.update_status()`, and returns the full `StoredObject`.
/// Returns 404 if status subresource is not enabled for this kind.
pub async fn update_status(
    State(state): State<AppState>,
    Path(path): Path<ObjectNamePath>,
    Json(body): Json<Value>,
) -> Result<Json<StoredObject>, AppError> {
    let key = ResourceKey { group: path.group, version: path.version, kind: path.kind };

    // Extract status field from body
    let status = body.get("status").cloned().unwrap_or(Value::Object(serde_json::Map::new()));

    let updated = state.object_service().update_status(key, path.name, status).await?;
    Ok(Json(updated))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::Router;
    use axum::body::Body;
    use axum::http::{Method, Request, StatusCode};
    use http_body_util::BodyExt;
    use serde_json::json;
    use std::sync::Arc;
    use tower::ServiceExt;

    use crate::event::EventBus;
    use crate::event::EventPublisher;
    use crate::object::schema_service::SchemaService;
    use crate::object::service::ObjectService;
    use crate::object::types::LabelRequirement;
    use crate::routes::build_router;
    use crate::schema::SchemaValidator;
    use crate::schema::meta_schema::compile_meta_schema;
    use crate::store::ObjectStore;
    use crate::store::memory::InMemoryStore;

    /// Builds a test router with an in-memory store for handler-level tests.
    fn test_router() -> Router {
        let store: Arc<dyn ObjectStore> = Arc::new(InMemoryStore::new());
        let event_bus: Arc<dyn EventPublisher> = Arc::new(EventBus::default());
        let meta_validator: Arc<dyn SchemaValidator> =
            Arc::new(compile_meta_schema().expect("meta-schema should compile"));

        let schema_service =
            Arc::new(SchemaService::new(store.clone(), event_bus.clone(), meta_validator.clone()));

        let object_service = Arc::new(ObjectService::new(
            store.clone(),
            event_bus.clone(),
            crate::schema::SchemaRegistry::new(store, meta_validator),
        ));

        let state = AppState::new(object_service, schema_service);
        build_router(state)
    }

    /// Sends a request via a cloned router (consuming the clone) and returns response.
    async fn send_request(
        router: &Router,
        method: Method,
        uri: &str,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        let req = match body {
            Some(b) => Request::builder()
                .method(method)
                .uri(uri)
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&b).unwrap()))
                .unwrap(),
            None => Request::builder().method(method).uri(uri).body(Body::empty()).unwrap(),
        };
        let resp = router.clone().oneshot(req).await.unwrap();
        let status = resp.status();
        let body_bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let body: Value = serde_json::from_slice(&body_bytes).unwrap_or(json!({}));
        (status, body)
    }

    /// Registers a Widget schema on the given router (cloned internally).
    async fn register_schema(router: &Router) {
        let schema_body = json!({
            "targetGroup": "example.io",
            "targetVersion": "v1",
            "targetKind": "Widget",
            "specSchema": {
                "type": "object",
                "properties": {
                    "color": { "type": "string" },
                    "size": { "type": "integer" }
                },
                "required": ["color", "size"]
            }
        });
        let req = Request::builder()
            .method(Method::POST)
            .uri("/apis/kapi.io/v1/Schema")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&schema_body).unwrap()))
            .unwrap();
        let resp = router.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED, "schema registration should succeed");
    }

    // --- create handler: fail-fast validation tests ---

    #[tokio::test]
    async fn create_rejects_invalid_label_key_before_service() {
        let router = test_router();
        register_schema(&router).await;

        // Create with invalid label key containing '!'
        let body = json!({
            "metadata": {
                "name": "test-widget",
                "labels": { "invalid key!": "value" }
            },
            "spec": { "color": "red", "size": 1 }
        });
        let (status, resp_body) =
            send_request(&router, Method::POST, "/apis/example.io/v1/Widget", Some(body)).await;
        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "expected 400 for invalid label key, got {status}: {resp_body}"
        );
        // Should be an InvalidLabel error, not a service error
        let error_msg = resp_body["error"].as_str().unwrap_or("");
        assert!(
            error_msg.contains("label"),
            "expected a label-related error message, got: {error_msg}"
        );
    }

    #[tokio::test]
    async fn create_rejects_annotations_exceeding_size_limit_before_service() {
        let router = test_router();
        register_schema(&router).await;

        // Create with annotations exceeding 256KB
        let large_value = "x".repeat(256 * 1024); // > 256KB
        let body = json!({
            "metadata": {
                "name": "test-widget",
                "annotations": { "key": large_value }
            },
            "spec": { "color": "red", "size": 1 }
        });
        let (status, resp_body) =
            send_request(&router, Method::POST, "/apis/example.io/v1/Widget", Some(body)).await;
        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "expected 400 for oversized annotations, got {status}: {resp_body}"
        );
        let error_msg = resp_body["error"].as_str().unwrap_or("");
        assert!(
            error_msg.contains("annotation"),
            "expected an annotation-related error message, got: {error_msg}"
        );
    }

    // --- update handler: fail-fast validation tests ---

    #[tokio::test]
    async fn update_rejects_invalid_label_key_before_service() {
        let router = test_router();
        register_schema(&router).await;

        // First create a valid object
        let create_body = json!({
            "metadata": { "name": "update-test" },
            "spec": { "color": "red", "size": 1 }
        });
        let (status, create_resp) =
            send_request(&router, Method::POST, "/apis/example.io/v1/Widget", Some(create_body))
                .await;
        assert_eq!(status, StatusCode::CREATED, "create should succeed");
        let rv = create_resp["system"]["resourceVersion"].as_u64().unwrap_or(0);
        let created_at = create_resp["system"]["createdAt"].as_str().unwrap_or("").to_string();
        let updated_at = create_resp["system"]["updatedAt"].as_str().unwrap_or("").to_string();

        // Now update with invalid labels
        let update_body = json!({
            "key": { "group": "example.io", "version": "v1", "kind": "Widget" },
            "metadata": {
                "name": "update-test",
                "labels": { "invalid key!": "value" }
            },
            "system": {
                "resourceVersion": rv,
                "createdAt": created_at,
                "updatedAt": updated_at
            },
            "spec": { "color": "blue", "size": 2 }
        });
        let (status, resp_body) = send_request(
            &router,
            Method::PUT,
            "/apis/example.io/v1/Widget/update-test",
            Some(update_body),
        )
        .await;
        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "expected 400 for invalid label in update, got {status}: {resp_body}"
        );
        let error_msg = resp_body["error"].as_str().unwrap_or("");
        assert!(
            error_msg.contains("label"),
            "expected a label-related error message, got: {error_msg}"
        );
    }

    #[test]
    fn parse_field_selector_valid_metadata_name() {
        let result = FieldSelector::parse("metadata.name=my-widget");
        assert!(result.is_ok());
        let filter = result.unwrap();
        assert!(matches!(
            filter,
            WatchFilter::FieldSelector(FieldSelector::NameEquals(name)) if name == "my-widget"
        ));
    }

    #[test]
    fn parse_field_selector_unsupported_field() {
        let result = FieldSelector::parse("metadata.namespace=default");
        assert!(result.is_err());
        assert!(
            matches!(result, Err(AppError::InvalidFieldSelector(msg)) if msg.contains("metadata.namespace"))
        );
    }

    #[test]
    fn parse_field_selector_malformed_input() {
        let result = FieldSelector::parse("invalid-format");
        assert!(result.is_err());
        assert!(
            matches!(result, Err(AppError::InvalidFieldSelector(msg)) if msg.contains("expected 'field=value'"))
        );
    }

    #[test]
    fn parse_field_selector_empty_value() {
        let result = FieldSelector::parse("metadata.name=");
        assert!(result.is_ok());
        let filter = result.unwrap();
        assert!(matches!(
            filter,
            WatchFilter::FieldSelector(FieldSelector::NameEquals(name)) if name.is_empty()
        ));
    }

    // parse_label_selector tests

    #[test]
    fn parse_label_selector_equality() {
        let result = LabelSelector::parse("app=nginx");
        assert!(result.is_ok());
        let filter = result.unwrap();
        if let WatchFilter::LabelSelector(selector) = filter {
            assert_eq!(selector.requirements.len(), 1);
            if let LabelRequirement::Equals { key, value } = &selector.requirements[0] {
                assert_eq!(key, "app");
                assert_eq!(value, "nginx");
            } else {
                panic!("expected Equals requirement");
            }
        } else {
            panic!("expected LabelSelector filter");
        }
    }

    #[test]
    fn parse_label_selector_inequality() {
        let result = LabelSelector::parse("env!=prod");
        assert!(result.is_ok());
        let filter = result.unwrap();
        if let WatchFilter::LabelSelector(selector) = filter {
            assert_eq!(selector.requirements.len(), 1);
            if let LabelRequirement::NotEquals { key, value } = &selector.requirements[0] {
                assert_eq!(key, "env");
                assert_eq!(value, "prod");
            } else {
                panic!("expected NotEquals requirement");
            }
        } else {
            panic!("expected LabelSelector filter");
        }
    }

    #[test]
    fn parse_label_selector_existence() {
        let result = LabelSelector::parse("gpu");
        assert!(result.is_ok());
        let filter = result.unwrap();
        if let WatchFilter::LabelSelector(selector) = filter {
            assert_eq!(selector.requirements.len(), 1);
            if let LabelRequirement::Exists { key } = &selector.requirements[0] {
                assert_eq!(key, "gpu");
            } else {
                panic!("expected Exists requirement");
            }
        } else {
            panic!("expected LabelSelector filter");
        }
    }

    #[test]
    fn parse_label_selector_non_existence() {
        let result = LabelSelector::parse("!experimental");
        assert!(result.is_ok());
        let filter = result.unwrap();
        if let WatchFilter::LabelSelector(selector) = filter {
            assert_eq!(selector.requirements.len(), 1);
            if let LabelRequirement::NotExists { key } = &selector.requirements[0] {
                assert_eq!(key, "experimental");
            } else {
                panic!("expected NotExists requirement");
            }
        } else {
            panic!("expected LabelSelector filter");
        }
    }

    #[test]
    fn parse_label_selector_and_combinator() {
        let result = LabelSelector::parse("app=nginx,env=prod");
        assert!(result.is_ok());
        let filter = result.unwrap();
        if let WatchFilter::LabelSelector(selector) = filter {
            assert_eq!(selector.requirements.len(), 2);
        } else {
            panic!("expected LabelSelector filter");
        }
    }

    #[test]
    fn parse_label_selector_mixed_operators() {
        let result = LabelSelector::parse("app=nginx,!experimental,gpu");
        assert!(result.is_ok());
        let filter = result.unwrap();
        if let WatchFilter::LabelSelector(selector) = filter {
            assert_eq!(selector.requirements.len(), 3);
            assert!(matches!(&selector.requirements[0], LabelRequirement::Equals { .. }));
            assert!(matches!(&selector.requirements[1], LabelRequirement::NotExists { .. }));
            assert!(matches!(&selector.requirements[2], LabelRequirement::Exists { .. }));
        } else {
            panic!("expected LabelSelector filter");
        }
    }

    #[test]
    fn parse_label_selector_empty_string() {
        let result = LabelSelector::parse("");
        assert!(result.is_ok());
        let filter = result.unwrap();
        if let WatchFilter::LabelSelector(selector) = filter {
            assert!(selector.requirements.is_empty());
        } else {
            panic!("expected LabelSelector filter");
        }
    }

    #[test]
    fn parse_label_selector_with_whitespace() {
        let result = LabelSelector::parse("app=nginx, env=prod");
        assert!(result.is_ok());
        let filter = result.unwrap();
        if let WatchFilter::LabelSelector(selector) = filter {
            assert_eq!(selector.requirements.len(), 2);
        } else {
            panic!("expected LabelSelector filter");
        }
    }

    #[test]
    fn parse_label_selector_empty_value_error() {
        let result = LabelSelector::parse("app=");
        assert!(result.is_err());
        assert!(
            matches!(result, Err(AppError::InvalidLabelSelector(msg)) if msg.contains("empty value"))
        );
    }

    #[test]
    fn parse_label_selector_invalid_key_with_space() {
        let result = LabelSelector::parse("invalid key!=value");
        assert!(result.is_err());
        assert!(
            matches!(result, Err(AppError::InvalidLabelSelector(msg)) if msg.contains("whitespace"))
        );
    }

    #[test]
    fn parse_label_selector_empty_segment_error() {
        let result = LabelSelector::parse("app=nginx,,env=prod");
        assert!(result.is_err());
        assert!(
            matches!(result, Err(AppError::InvalidLabelSelector(msg)) if msg.contains("empty segment"))
        );
    }

    // Watch filter combination tests

    #[test]
    fn watch_filter_combination_both_present_creates_and() {
        let field = FieldSelector::parse("metadata.name=foo").unwrap();
        let label = LabelSelector::parse("app=nginx").unwrap();
        let combined = match (Some(field), Some(label)) {
            (Some(f), Some(l)) => WatchFilter::And(Box::new(f), Box::new(l)),
            (Some(f), None) => f,
            (None, Some(l)) => l,
            (None, None) => WatchFilter::All,
        };
        assert!(matches!(combined, WatchFilter::And(_, _)));
    }

    #[test]
    fn watch_filter_combination_field_only() {
        let field = FieldSelector::parse("metadata.name=foo").unwrap();
        let combined = match (Some(field), None::<WatchFilter>) {
            (Some(f), Some(l)) => WatchFilter::And(Box::new(f), Box::new(l)),
            (Some(f), None) => f,
            (None, Some(l)) => l,
            (None, None) => WatchFilter::All,
        };
        assert!(matches!(combined, WatchFilter::FieldSelector(_)));
    }

    #[test]
    fn watch_filter_combination_label_only() {
        let label = LabelSelector::parse("app=nginx").unwrap();
        let combined = match (None::<WatchFilter>, Some(label)) {
            (Some(f), Some(l)) => WatchFilter::And(Box::new(f), Box::new(l)),
            (Some(f), None) => f,
            (None, Some(l)) => l,
            (None, None) => WatchFilter::All,
        };
        assert!(matches!(combined, WatchFilter::LabelSelector(_)));
    }

    #[test]
    fn watch_filter_combination_neither() {
        let combined = match (None::<WatchFilter>, None::<WatchFilter>) {
            (Some(f), Some(l)) => WatchFilter::And(Box::new(f), Box::new(l)),
            (Some(f), None) => f,
            (None, Some(l)) => l,
            (None, None) => WatchFilter::All,
        };
        assert!(matches!(combined, WatchFilter::All));
    }
}
