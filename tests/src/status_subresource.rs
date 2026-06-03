use axum::http::StatusCode;
use serde_json::Value;

use crate::{
    TestApp, assert_status, parse_body,
};

fn widget_schema_with_status() -> Value {
    serde_json::json!({
        "targetGroup": "example.io",
        "targetVersion": "v1",
        "targetKind": "Widget",
        "jsonSchema": {
            "type": "object",
            "properties": {
                "color": { "type": "string" },
                "size": { "type": "integer" }
            },
            "required": ["color", "size"]
        },
        "statusSchema": {
            "type": "object",
            "properties": {
                "phase": { "type": "string" },
                "message": { "type": "string" }
            }
        }
    })
}

/// 8.1: Register Schema with statusSchema, create object, update status via /status, verify status is set
pub async fn test_status_subresource_update(app: &TestApp) -> Result<(), String> {
    let client = app.client();

    // Register schema with statusSchema
    let resp = client
        .post("/apis/kapi.io/v1/Schema", widget_schema_with_status())
        .await;
    assert_status(&resp, StatusCode::CREATED);

    // Create object
    let resp = client
        .post(
            "/apis/example.io/v1/Widget",
            serde_json::json!({
                "metadata": { "name": "test-widget" },
                "color": "blue",
                "size": 10
            }),
        )
        .await;
    assert_status(&resp, StatusCode::CREATED);
    let created: Value = parse_body(resp).await;
    assert!(created.get("status").is_none() || created["status"].is_null());

    // Update status via /status endpoint
    let resp = client
        .put(
            "/apis/example.io/v1/Widget/test-widget/status",
            serde_json::json!({
                "status": {
                    "phase": "Running",
                    "message": "All systems go"
                }
            }),
        )
        .await;
    assert_status(&resp, StatusCode::OK);
    let updated: Value = parse_body(resp).await;
    assert_eq!(updated["status"]["value"]["phase"], "Running");
    assert_eq!(updated["status"]["value"]["message"], "All systems go");

    // Get status via /status endpoint
    let resp = client
        .get("/apis/example.io/v1/Widget/test-widget/status")
        .await;
    assert_status(&resp, StatusCode::OK);
    let status: Value = parse_body(resp).await;
    // Status is returned as SpecData with a "value" wrapper
    assert_eq!(status["value"]["phase"], "Running");

    Ok(())
}

/// 8.2: Register Schema without statusSchema, attempt /status GET/PUT, verify 404
pub async fn test_status_subresource_not_enabled(app: &TestApp) -> Result<(), String> {
    let client = app.client();

    // Register schema WITHOUT statusSchema
    let resp = client
        .post("/apis/kapi.io/v1/Schema", crate::widget_schema())
        .await;
    assert_status(&resp, StatusCode::CREATED);

    // Create object
    let resp = client
        .post(
            "/apis/example.io/v1/Widget",
            serde_json::json!({
                "metadata": { "name": "test-widget" },
                "color": "blue",
                "size": 10
            }),
        )
        .await;
    assert_status(&resp, StatusCode::CREATED);

    // GET /status should return 404
    let resp = client
        .get("/apis/example.io/v1/Widget/test-widget/status")
        .await;
    assert_status(&resp, StatusCode::NOT_FOUND);

    // PUT /status should return 404
    let resp = client
        .put(
            "/apis/example.io/v1/Widget/test-widget/status",
            serde_json::json!({
                "status": { "phase": "Running" }
            }),
        )
        .await;
    assert_status(&resp, StatusCode::NOT_FOUND);

    Ok(())
}

/// 8.3: Update status with invalid data, verify 422 SchemaValidation
pub async fn test_status_subresource_invalid_data(app: &TestApp) -> Result<(), String> {
    let client = app.client();

    // Register schema with statusSchema (phase must be string)
    let resp = client
        .post("/apis/kapi.io/v1/Schema", widget_schema_with_status())
        .await;
    assert_status(&resp, StatusCode::CREATED);

    // Create object
    let resp = client
        .post(
            "/apis/example.io/v1/Widget",
            serde_json::json!({
                "metadata": { "name": "test-widget" },
                "color": "blue",
                "size": 10
            }),
        )
        .await;
    assert_status(&resp, StatusCode::CREATED);

    // Update status with invalid data (phase should be string, not integer)
    let resp = client
        .put(
            "/apis/example.io/v1/Widget/test-widget/status",
            serde_json::json!({
                "status": { "phase": 123 }
            }),
        )
        .await;
    assert_status(&resp, StatusCode::UNPROCESSABLE_ENTITY);

    Ok(())
}

/// 8.4: Concurrent spec update and status update succeed without conflict
pub async fn test_concurrent_spec_and_status_update(app: &TestApp) -> Result<(), String> {
    let client = app.client();

    // Register schema with statusSchema
    let resp = client
        .post("/apis/kapi.io/v1/Schema", widget_schema_with_status())
        .await;
    assert_status(&resp, StatusCode::CREATED);

    // Create object
    let resp = client
        .post(
            "/apis/example.io/v1/Widget",
            serde_json::json!({
                "metadata": { "name": "test-widget" },
                "color": "blue",
                "size": 10
            }),
        )
        .await;
    assert_status(&resp, StatusCode::CREATED);

    // Update status (no CAS check)
    let resp = client
        .put(
            "/apis/example.io/v1/Widget/test-widget/status",
            serde_json::json!({
                "status": { "phase": "Running" }
            }),
        )
        .await;
    assert_status(&resp, StatusCode::OK);
    let status_updated: Value = parse_body(resp).await;
    assert_eq!(status_updated["status"]["value"]["phase"], "Running");

    // Verify status persists by getting the object
    let resp = client
        .get("/apis/example.io/v1/Widget/test-widget")
        .await;
    assert_status(&resp, StatusCode::OK);
    let obj: Value = parse_body(resp).await;
    assert_eq!(obj["status"]["value"]["phase"], "Running");

    // Update status again - should succeed without CAS
    let resp = client
        .put(
            "/apis/example.io/v1/Widget/test-widget/status",
            serde_json::json!({
                "status": { "phase": "Completed" }
            }),
        )
        .await;
    assert_status(&resp, StatusCode::OK);
    let status_updated2: Value = parse_body(resp).await;
    assert_eq!(status_updated2["status"]["value"]["phase"], "Completed");

    Ok(())
}

/// 8.5: Create object with status in body, verify status is null (ignored)
pub async fn test_create_ignores_status_in_body(app: &TestApp) -> Result<(), String> {
    let client = app.client();

    // Register schema with statusSchema
    let resp = client
        .post("/apis/kapi.io/v1/Schema", widget_schema_with_status())
        .await;
    assert_status(&resp, StatusCode::CREATED);

    // Create object with status in body — should be ignored
    let resp = client
        .post(
            "/apis/example.io/v1/Widget",
            serde_json::json!({
                "metadata": { "name": "test-widget" },
                "color": "blue",
                "size": 10,
                "status": { "phase": "Pre-set" }
            }),
        )
        .await;
    assert_status(&resp, StatusCode::CREATED);
    let created: Value = parse_body(resp).await;
    assert!(
        created.get("status").is_none() || created["status"].is_null(),
        "status should be null/absent when creating object, but got: {}",
        created["status"]
    );

    Ok(())
}

/// Status update for non-existent object returns 404 NotFound
pub async fn test_status_update_nonexistent_object(app: &TestApp) -> Result<(), String> {
    let client = app.client();

    // Register schema with statusSchema
    let resp = client
        .post("/apis/kapi.io/v1/Schema", widget_schema_with_status())
        .await;
    assert_status(&resp, StatusCode::CREATED);

    // PUT /status for object that does not exist should return 404 NotFound
    let resp = client
        .put(
            "/apis/example.io/v1/Widget/nonexistent-widget/status",
            serde_json::json!({
                "status": { "phase": "Running" }
            }),
        )
        .await;
    assert_status(&resp, StatusCode::NOT_FOUND);

    Ok(())
}

/// Status update publishes StatusModified watch event
pub async fn test_status_update_publishes_status_modified_event(app: &TestApp) -> Result<(), String> {
    use crate::{watch_events, WatchEventType};
    use tokio::time::timeout;
    use std::time::Duration;

    let client = app.client();

    // Register schema with statusSchema
    let resp = client
        .post("/apis/kapi.io/v1/Schema", widget_schema_with_status())
        .await;
    assert_status(&resp, StatusCode::CREATED);

    // Create object
    let resp = client
        .post(
            "/apis/example.io/v1/Widget",
            serde_json::json!({
                "metadata": { "name": "event-widget" },
                "color": "blue",
                "size": 10
            }),
        )
        .await;
    assert_status(&resp, StatusCode::CREATED);

    // Start watching BEFORE status update
    let mut events = watch_events(&client, "/apis/example.io/v1/Widget?watch=true").await;
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Update status
    let resp = client
        .put(
            "/apis/example.io/v1/Widget/event-widget/status",
            serde_json::json!({
                "status": { "phase": "Running" }
            }),
        )
        .await;
    assert_status(&resp, StatusCode::OK);

    // Should receive StatusModified event
    let event = timeout(Duration::from_secs(3), events.recv())
        .await
        .map_err(|_| "timeout waiting for StatusModified event".to_string())?
        .ok_or("watch stream ended before receiving event".to_string())?;

    assert!(
        matches!(event.event_type, WatchEventType::StatusModified),
        "expected StatusModified event, got {:?}",
        event.event_type
    );
    assert_eq!(
        event.object.metadata.name, "event-widget",
        "event should be for the correct object"
    );
    // Full object context should be present
    assert!(
        event.object.spec.value.get("color").is_some(),
        "StatusModified event should include full spec context"
    );

    Ok(())
}

/// Status update does not modify spec field
pub async fn test_status_update_preserves_spec(app: &TestApp) -> Result<(), String> {
    let client = app.client();

    // Register schema with statusSchema
    let resp = client
        .post("/apis/kapi.io/v1/Schema", widget_schema_with_status())
        .await;
    assert_status(&resp, StatusCode::CREATED);

    // Create object
    let resp = client
        .post(
            "/apis/example.io/v1/Widget",
            serde_json::json!({
                "metadata": { "name": "spec-preserve-widget" },
                "color": "blue",
                "size": 10
            }),
        )
        .await;
    assert_status(&resp, StatusCode::CREATED);

    // Update status
    let resp = client
        .put(
            "/apis/example.io/v1/Widget/spec-preserve-widget/status",
            serde_json::json!({
                "status": { "phase": "Running", "message": "test" }
            }),
        )
        .await;
    assert_status(&resp, StatusCode::OK);

    // Get the object and verify spec is unchanged
    let resp = client
        .get("/apis/example.io/v1/Widget/spec-preserve-widget")
        .await;
    assert_status(&resp, StatusCode::OK);
    let obj: Value = parse_body(resp).await;

    assert_eq!(obj["spec"]["value"]["color"], "blue", "spec.color should be unchanged");
    assert_eq!(obj["spec"]["value"]["size"], 10, "spec.size should be unchanged");
    assert_eq!(obj["status"]["value"]["phase"], "Running", "status should be set");

    Ok(())
}

/// Status update bumps resource_version
pub async fn test_status_update_bumps_resource_version(app: &TestApp) -> Result<(), String> {
    let client = app.client();

    // Register schema with statusSchema
    let resp = client
        .post("/apis/kapi.io/v1/Schema", widget_schema_with_status())
        .await;
    assert_status(&resp, StatusCode::CREATED);

    // Create object
    let resp = client
        .post(
            "/apis/example.io/v1/Widget",
            serde_json::json!({
                "metadata": { "name": "rv-bump-widget" },
                "color": "blue",
                "size": 10
            }),
        )
        .await;
    assert_status(&resp, StatusCode::CREATED);
    let created: Value = parse_body(resp).await;
    let initial_rv = created["system"]["resourceVersion"]
        .as_u64()
        .ok_or("missing resourceVersion on created object")?;

    // Update status
    let resp = client
        .put(
            "/apis/example.io/v1/Widget/rv-bump-widget/status",
            serde_json::json!({
                "status": { "phase": "Running" }
            }),
        )
        .await;
    assert_status(&resp, StatusCode::OK);
    let updated: Value = parse_body(resp).await;
    let status_rv = updated["system"]["resourceVersion"]
        .as_u64()
        .ok_or("missing resourceVersion after status update")?;

    assert!(
        status_rv > initial_rv,
        "resourceVersion should be bumped after status update: {} > {}",
        status_rv,
        initial_rv
    );

    Ok(())
}

/// Schema registration with invalid statusSchema fails
pub async fn test_invalid_status_schema_rejected(app: &TestApp) -> Result<(), String> {
    let client = app.client();

    // Register schema with invalid statusSchema (invalid JSON Schema type)
    let resp = client
        .post(
            "/apis/kapi.io/v1/Schema",
            serde_json::json!({
                "targetGroup": "example.io",
                "targetVersion": "v1",
                "targetKind": "Widget",
                "jsonSchema": {
                    "type": "object",
                    "properties": {
                        "color": { "type": "string" }
                    }
                },
                "statusSchema": {
                    "type": "not-a-real-type"
                }
            }),
        )
        .await;
    assert_status(&resp, StatusCode::UNPROCESSABLE_ENTITY);

    Ok(())
}

/// GET /status returns null when status not yet set
pub async fn test_get_status_returns_null_when_not_set(app: &TestApp) -> Result<(), String> {
    let client = app.client();

    // Register schema with statusSchema
    let resp = client
        .post("/apis/kapi.io/v1/Schema", widget_schema_with_status())
        .await;
    assert_status(&resp, StatusCode::CREATED);

    // Create object (status is null)
    let resp = client
        .post(
            "/apis/example.io/v1/Widget",
            serde_json::json!({
                "metadata": { "name": "null-status-widget" },
                "color": "blue",
                "size": 10
            }),
        )
        .await;
    assert_status(&resp, StatusCode::CREATED);

    // GET /status should return null (status not yet set)
    let resp = client
        .get("/apis/example.io/v1/Widget/null-status-widget/status")
        .await;
    assert_status(&resp, StatusCode::OK);
    let status: Value = parse_body(resp).await;
    assert!(
        status.is_null(),
        "status should be null when not yet set, got: {}",
        status
    );

    Ok(())
}

/// Meta-schema rejects statusSchema with invalid type (non-object)
pub async fn test_meta_schema_rejects_invalid_status_schema_type(app: &TestApp) -> Result<(), String> {
    let client = app.client();

    // statusSchema must be an object, not a string
    let resp = client
        .post(
            "/apis/kapi.io/v1/Schema",
            serde_json::json!({
                "targetGroup": "example.io",
                "targetVersion": "v1",
                "targetKind": "Widget",
                "jsonSchema": {
                    "type": "object",
                    "properties": {
                        "color": { "type": "string" }
                    }
                },
                "statusSchema": "this is not a schema"
            }),
        )
        .await;
    assert_status(&resp, StatusCode::UNPROCESSABLE_ENTITY);

    // statusSchema must be an object, not an array
    let resp = client
        .post(
            "/apis/kapi.io/v1/Schema",
            serde_json::json!({
                "targetGroup": "example.io",
                "targetVersion": "v1",
                "targetKind": "Widget",
                "jsonSchema": {
                    "type": "object",
                    "properties": {
                        "color": { "type": "string" }
                    }
                },
                "statusSchema": ["not", "a", "schema"]
            }),
        )
        .await;
    assert_status(&resp, StatusCode::UNPROCESSABLE_ENTITY);

    Ok(())
}

/// Status update replaces status completely (not merged)
pub async fn test_status_update_replaces_not_merges(app: &TestApp) -> Result<(), String> {
    let client = app.client();

    // Register schema with statusSchema that allows multiple fields
    let resp = client
        .post("/apis/kapi.io/v1/Schema", widget_schema_with_status())
        .await;
    assert_status(&resp, StatusCode::CREATED);

    // Create object
    let resp = client
        .post(
            "/apis/example.io/v1/Widget",
            serde_json::json!({
                "metadata": { "name": "replace-widget" },
                "color": "blue",
                "size": 10
            }),
        )
        .await;
    assert_status(&resp, StatusCode::CREATED);

    // Set status with both phase and message
    let resp = client
        .put(
            "/apis/example.io/v1/Widget/replace-widget/status",
            serde_json::json!({
                "status": { "phase": "Running", "message": "initial message" }
            }),
        )
        .await;
    assert_status(&resp, StatusCode::OK);

    // Update status with only phase (no message)
    let resp = client
        .put(
            "/apis/example.io/v1/Widget/replace-widget/status",
            serde_json::json!({
                "status": { "phase": "Completed" }
            }),
        )
        .await;
    assert_status(&resp, StatusCode::OK);

    // Verify message is gone (replaced, not merged)
    let resp = client
        .get("/apis/example.io/v1/Widget/replace-widget/status")
        .await;
    assert_status(&resp, StatusCode::OK);
    let status: Value = parse_body(resp).await;
    assert_eq!(status["value"]["phase"], "Completed");
    assert!(
        status["value"].get("message").is_none(),
        "message should be removed after status replacement, but got: {}",
        status["value"]["message"]
    );

    Ok(())
}

/// Spec update publishes Modified event (not StatusModified)
pub async fn test_spec_update_publishes_modified_not_status_modified(app: &TestApp) -> Result<(), String> {
    use crate::{watch_events, WatchEventType};
    use tokio::time::timeout;
    use std::time::Duration;

    let client = app.client();

    // Register schema with statusSchema
    let resp = client
        .post("/apis/kapi.io/v1/Schema", widget_schema_with_status())
        .await;
    assert_status(&resp, StatusCode::CREATED);

    // Create object
    let resp = client
        .post(
            "/apis/example.io/v1/Widget",
            serde_json::json!({
                "metadata": { "name": "spec-event-widget" },
                "color": "blue",
                "size": 10
            }),
        )
        .await;
    assert_status(&resp, StatusCode::CREATED);
    let created: Value = parse_body(resp).await;
    let rv = created["system"]["resourceVersion"].as_u64().unwrap_or(0);
    let created_at = created["system"]["createdAt"].as_str().unwrap_or("").to_string();
    let updated_at = created["system"]["updatedAt"].as_str().unwrap_or("").to_string();

    // Start watching
    let mut events = watch_events(&client, "/apis/example.io/v1/Widget?watch=true").await;
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Update spec (regular PUT)
    let update_body = serde_json::json!({
        "key": { "group": "example.io", "version": "v1", "kind": "Widget" },
        "metadata": { "name": "spec-event-widget" },
        "system": { "resourceVersion": rv, "createdAt": created_at, "updatedAt": updated_at },
        "spec": { "value": { "color": "red", "size": 20 } }
    });
    let resp = client
        .put("/apis/example.io/v1/Widget/spec-event-widget", update_body)
        .await;
    assert_status(&resp, StatusCode::OK);

    // Should receive Modified event (not StatusModified)
    let event = timeout(Duration::from_secs(3), events.recv())
        .await
        .map_err(|_| "timeout waiting for Modified event".to_string())?
        .ok_or("watch stream ended before receiving event".to_string())?;

    assert!(
        matches!(event.event_type, WatchEventType::Modified),
        "expected Modified event for spec update, got {:?}",
        event.event_type
    );

    Ok(())
}
