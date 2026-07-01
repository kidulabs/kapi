//! Integration tests for Namespace as a first-class resource.
//!
//! Covers:
//! - Namespace CRUD operations (create, get, list, update, delete)
//! - "default" namespace bootstrap
//! - "default" namespace deletion rejection (403)
//! - Namespace existence validation on object creation (404)
//! - Namespace deletion blocking (409 for non-empty namespaces)
//! - Namespace-scoped watch (WatchFilter::Namespace)

use axum::http::StatusCode;
use serde_json::Value;

use crate::{
    DEFAULT_NS, TestApp, assert_status, parse_body, register_namespace, register_widget_schema,
    widget, widget_collection_url, widget_item_url,
};

const NAMESPACE_API: &str = "/apis/kapi.io/v1/Namespace";
const NAMESPACE_ITEM_API: &str = "/apis/kapi.io/v1/Namespace";

// ──────────────────────────────────────────────
// Task 8.1: Namespace CRUD operations
// ──────────────────────────────────────────────

/// Create a namespace via the API.
pub async fn test_create_namespace(app: &TestApp) -> Result<(), String> {
    let client = app.client();
    let body = serde_json::json!({
        "metadata": { "name": "production" },
        "spec": { "annotations": {} }
    });
    let resp = client.post(NAMESPACE_API, body).await;
    assert_status(&resp, StatusCode::CREATED);
    let created: Value = parse_body(resp).await;
    assert_eq!(created["metadata"]["name"].as_str(), Some("production"));
    // Namespace is cluster-scoped — metadata.namespace must be None
    assert!(
        created["metadata"]["namespace"].is_null(),
        "namespace objects must be cluster-scoped, got: {:?}",
        created["metadata"]["namespace"]
    );
    Ok(())
}

/// Get a namespace via the API.
pub async fn test_get_namespace(app: &TestApp) -> Result<(), String> {
    let client = app.client();
    register_namespace(&client, "staging").await;

    let resp = client.get(&format!("{NAMESPACE_ITEM_API}/staging")).await;
    assert_status(&resp, StatusCode::OK);
    let body: Value = parse_body(resp).await;
    assert_eq!(body["metadata"]["name"].as_str(), Some("staging"));
    Ok(())
}

/// Get a non-existent namespace returns 404.
pub async fn test_get_nonexistent_namespace(app: &TestApp) -> Result<(), String> {
    let client = app.client();
    let resp = client.get(&format!("{NAMESPACE_ITEM_API}/nonexistent")).await;
    assert_status(&resp, StatusCode::NOT_FOUND);
    Ok(())
}

/// List namespaces via the API.
pub async fn test_list_namespaces(app: &TestApp) -> Result<(), String> {
    let client = app.client();
    // "default" is bootstrapped; add a few more
    register_namespace(&client, "alpha").await;
    register_namespace(&client, "beta").await;

    let resp = client.get(NAMESPACE_API).await;
    assert_status(&resp, StatusCode::OK);
    let body: Value = parse_body(resp).await;
    let items = body["items"].as_array().unwrap();
    assert_eq!(items.len(), 3, "should list default + alpha + beta");

    let names: Vec<&str> =
        items.iter().map(|i| i["metadata"]["name"].as_str().unwrap_or("")).collect();
    assert!(names.contains(&"default"));
    assert!(names.contains(&"alpha"));
    assert!(names.contains(&"beta"));
    Ok(())
}

/// Update a namespace's labels via the API.
pub async fn test_update_namespace(app: &TestApp) -> Result<(), String> {
    let client = app.client();
    register_namespace(&client, "updatable").await;

    // Get the namespace to fetch resource_version
    let resp = client.get(&format!("{NAMESPACE_ITEM_API}/updatable")).await;
    let ns: Value = parse_body(resp).await;
    let rv = ns["system"]["resourceVersion"].as_u64().unwrap_or(0);
    let created_at = ns["system"]["createdAt"].as_str().unwrap_or("").to_string();
    let updated_at = ns["system"]["updatedAt"].as_str().unwrap_or("").to_string();

    // Update with new labels
    let body = serde_json::json!({
        "key": { "group": "kapi.io", "version": "v1", "kind": "Namespace" },
        "metadata": {
            "name": "updatable",
            "labels": { "env": "prod", "team": "platform" }
        },
        "system": {
            "resourceVersion": rv,
            "createdAt": created_at,
            "updatedAt": updated_at
        },
        "spec": {}
    });
    let resp = client.put(&format!("{NAMESPACE_ITEM_API}/updatable"), body).await;
    assert_status(&resp, StatusCode::OK);
    let updated: Value = parse_body(resp).await;
    assert_eq!(updated["metadata"]["labels"]["env"].as_str(), Some("prod"));
    assert_eq!(updated["metadata"]["labels"]["team"].as_str(), Some("platform"));
    Ok(())
}

// ──────────────────────────────────────────────
// Task 8.2: "default" namespace bootstrap
// ──────────────────────────────────────────────

/// "default" namespace exists after server startup.
pub async fn test_default_namespace_exists(app: &TestApp) -> Result<(), String> {
    let client = app.client();
    let resp = client.get(&format!("{NAMESPACE_ITEM_API}/default")).await;
    assert_status(&resp, StatusCode::OK);
    let body: Value = parse_body(resp).await;
    assert_eq!(body["metadata"]["name"].as_str(), Some("default"));
    Ok(())
}

/// Listing namespaces includes "default" (after fresh start).
pub async fn test_default_namespace_in_list(app: &TestApp) -> Result<(), String> {
    let client = app.client();
    let resp = client.get(NAMESPACE_API).await;
    assert_status(&resp, StatusCode::OK);
    let body: Value = parse_body(resp).await;
    let items = body["items"].as_array().unwrap();
    let names: Vec<&str> =
        items.iter().map(|i| i["metadata"]["name"].as_str().unwrap_or("")).collect();
    assert!(names.contains(&"default"), "default namespace should be in the list, got: {names:?}");
    Ok(())
}

// ──────────────────────────────────────────────
// Task 8.3: "default" namespace deletion rejection
// ──────────────────────────────────────────────

/// DELETE on the "default" namespace returns 403 Forbidden.
pub async fn test_delete_default_namespace_rejected(app: &TestApp) -> Result<(), String> {
    let client = app.client();
    let resp = client.delete(&format!("{NAMESPACE_ITEM_API}/default")).await;
    assert_status(&resp, StatusCode::FORBIDDEN);
    let body: Value = parse_body(resp).await;
    let error_msg = body["error"].as_str().unwrap_or("");
    assert!(
        error_msg.contains("protected") || error_msg.contains("default"),
        "expected protected/default error, got: {body}"
    );
    Ok(())
}

/// After rejected delete, "default" namespace still exists.
pub async fn test_default_namespace_persists_after_delete_attempt(
    app: &TestApp,
) -> Result<(), String> {
    let client = app.client();
    // Attempt to delete
    let _ = client.delete(&format!("{NAMESPACE_ITEM_API}/default")).await;
    // Verify it still exists
    let resp = client.get(&format!("{NAMESPACE_ITEM_API}/default")).await;
    assert_status(&resp, StatusCode::OK);
    Ok(())
}

// ──────────────────────────────────────────────
// Task 8.4: Namespace existence validation
// ──────────────────────────────────────────────

/// Creating an object in a non-existent namespace returns 404.
pub async fn test_create_in_nonexistent_namespace_404(app: &TestApp) -> Result<(), String> {
    let client = app.client();
    register_widget_schema(&client).await;

    let resp =
        client.post(&widget_collection_url("nonexistent"), widget("my-widget", "red", 1)).await;
    assert_status(&resp, StatusCode::NOT_FOUND);
    let body: Value = parse_body(resp).await;
    let error_msg = body["error"].as_str().unwrap_or("");
    assert!(
        error_msg.contains("namespace") || error_msg.contains("nonexistent"),
        "expected namespace-related error, got: {body}"
    );
    Ok(())
}

/// Creating an object in "default" (which always exists) succeeds.
pub async fn test_create_in_default_namespace_succeeds(app: &TestApp) -> Result<(), String> {
    let client = app.client();
    register_widget_schema(&client).await;

    let resp =
        client.post(&widget_collection_url(DEFAULT_NS), widget("default-widget", "blue", 1)).await;
    assert_status(&resp, StatusCode::CREATED);
    Ok(())
}

/// After creating a namespace, objects can be created in it.
pub async fn test_create_after_namespace_creation_succeeds(app: &TestApp) -> Result<(), String> {
    let client = app.client();
    register_widget_schema(&client).await;
    register_namespace(&client, "fresh").await;

    let resp = client.post(&widget_collection_url("fresh"), widget("widget-1", "green", 2)).await;
    assert_status(&resp, StatusCode::CREATED);
    Ok(())
}

// ──────────────────────────────────────────────
// Task 8.5: Namespace deletion blocking
// ──────────────────────────────────────────────

/// Deleting a non-empty namespace returns 409 Conflict.
pub async fn test_delete_non_empty_namespace_409(app: &TestApp) -> Result<(), String> {
    let client = app.client();
    register_widget_schema(&client).await;
    register_namespace(&client, "populated").await;

    // Create an object in the namespace
    let resp =
        client.post(&widget_collection_url("populated"), widget("widget-1", "blue", 1)).await;
    assert_status(&resp, StatusCode::CREATED);

    // Try to delete the namespace
    let resp = client.delete(&format!("{NAMESPACE_ITEM_API}/populated")).await;
    assert_status(&resp, StatusCode::CONFLICT);
    let body: Value = parse_body(resp).await;
    let error_msg = body["error"].as_str().unwrap_or("");
    assert!(
        error_msg.contains("not empty") || error_msg.contains("objects"),
        "expected not-empty error, got: {body}"
    );
    Ok(())
}

/// Error response includes the object count.
pub async fn test_delete_non_empty_namespace_error_includes_count(
    app: &TestApp,
) -> Result<(), String> {
    let client = app.client();
    register_widget_schema(&client).await;
    register_namespace(&client, "counted").await;

    // Create 2 objects
    for i in 0..2 {
        let resp = client
            .post(&widget_collection_url("counted"), widget(&format!("widget-{i}"), "blue", i))
            .await;
        assert_status(&resp, StatusCode::CREATED);
    }

    let resp = client.delete(&format!("{NAMESPACE_ITEM_API}/counted")).await;
    assert_status(&resp, StatusCode::CONFLICT);
    let body: Value = parse_body(resp).await;
    let details = body["details"].clone();
    let count = details["objectCount"].as_u64().unwrap_or(0);
    assert_eq!(count, 2, "expected objectCount=2 in error details, got: {body}");
    Ok(())
}

/// After deleting all objects, the empty namespace can be deleted.
pub async fn test_delete_namespace_after_clearing_objects(app: &TestApp) -> Result<(), String> {
    let client = app.client();
    register_widget_schema(&client).await;
    register_namespace(&client, "to-delete").await;

    // Create an object
    let resp =
        client.post(&widget_collection_url("to-delete"), widget("widget-1", "blue", 1)).await;
    assert_status(&resp, StatusCode::CREATED);

    // Delete the object first
    let resp = client.delete(&widget_item_url("to-delete", "widget-1")).await;
    assert_status(&resp, StatusCode::OK);

    // Now the namespace can be deleted
    let resp = client.delete(&format!("{NAMESPACE_ITEM_API}/to-delete")).await;
    assert_status(&resp, StatusCode::OK);
    Ok(())
}

/// Deleting an empty namespace succeeds.
pub async fn test_delete_empty_namespace(app: &TestApp) -> Result<(), String> {
    let client = app.client();
    register_namespace(&client, "empty").await;
    let resp = client.delete(&format!("{NAMESPACE_ITEM_API}/empty")).await;
    assert_status(&resp, StatusCode::OK);
    Ok(())
}
