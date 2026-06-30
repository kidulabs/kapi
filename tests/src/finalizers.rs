use axum::http::StatusCode;
use serde_json::Value;

use crate::{TestApp, assert_status, parse_body, register_widget_schema};

// ──────────────────────────────────────────────
// Task 2.2: DELETE integration tests
// ──────────────────────────────────────────────

/// DELETE object with empty finalizers → hard delete, object no longer exists (GET returns 404).
pub async fn test_delete_without_finalizers_hard_deletes(app: &TestApp) -> Result<(), String> {
    let client = app.client();
    register_widget_schema(&client).await;

    // Create object without finalizers
    let body = serde_json::json!({
        "metadata": { "name": "no-finalizers" },
        "spec": { "color": "blue", "size": 10 }
    });
    let resp = client.post("/apis/example.io/v1/namespaces/default/Widget", body).await;
    assert_status(&resp, StatusCode::CREATED);

    // DELETE → hard delete (empty finalizers)
    let resp = client.delete("/apis/example.io/v1/namespaces/default/Widget/no-finalizers").await;
    assert_status(&resp, StatusCode::OK);

    // GET → 404 (hard deleted)
    let resp = client.get("/apis/example.io/v1/namespaces/default/Widget/no-finalizers").await;
    assert_status(&resp, StatusCode::NOT_FOUND);

    Ok(())
}

/// DELETE object with finalizers → 200 with object, `deletionTimestamp` is set, object still exists (GET returns it).
pub async fn test_delete_with_finalizers_marks_for_deletion(app: &TestApp) -> Result<(), String> {
    let client = app.client();
    register_widget_schema(&client).await;

    // Create object with finalizers
    let body = serde_json::json!({
        "metadata": { "name": "with-finalizers", "finalizers": ["example.io/cleanup"] },
        "spec": { "color": "blue", "size": 10 }
    });
    let resp = client.post("/apis/example.io/v1/namespaces/default/Widget", body).await;
    assert_status(&resp, StatusCode::CREATED);

    // DELETE → marks for deletion (200, not hard delete)
    let resp = client.delete("/apis/example.io/v1/namespaces/default/Widget/with-finalizers").await;
    assert_status(&resp, StatusCode::OK);
    let deleted: Value = parse_body(resp).await;

    // Verify deletionTimestamp is set
    assert!(
        deleted["system"]["deletionTimestamp"].is_string(),
        "expected deletionTimestamp to be set"
    );
    assert_eq!(deleted["metadata"]["name"], "with-finalizers");

    // GET should still return the object (it's only marked for deletion)
    let resp = client.get("/apis/example.io/v1/namespaces/default/Widget/with-finalizers").await;
    assert_status(&resp, StatusCode::OK);
    let fetched: Value = parse_body(resp).await;
    assert!(
        fetched["system"]["deletionTimestamp"].is_string(),
        "GET should still return object with deletionTimestamp"
    );

    Ok(())
}

/// DELETE object that already has `deletionTimestamp` → 200, no state change, `deletionTimestamp` unchanged.
pub async fn test_delete_idempotent_on_already_deleting(app: &TestApp) -> Result<(), String> {
    let client = app.client();
    register_widget_schema(&client).await;

    // Create object with finalizers
    let body = serde_json::json!({
        "metadata": { "name": "idempotent-delete", "finalizers": ["example.io/cleanup"] },
        "spec": { "color": "blue", "size": 10 }
    });
    let resp = client.post("/apis/example.io/v1/namespaces/default/Widget", body).await;
    assert_status(&resp, StatusCode::CREATED);

    // First DELETE → marks for deletion
    let resp =
        client.delete("/apis/example.io/v1/namespaces/default/Widget/idempotent-delete").await;
    assert_status(&resp, StatusCode::OK);
    let first: Value = parse_body(resp).await;
    let ts = first["system"]["deletionTimestamp"].as_str().unwrap_or("").to_string();
    assert!(!ts.is_empty(), "deletionTimestamp should be set after first delete");
    let first_rv = first["system"]["resourceVersion"].as_u64().unwrap_or(0);

    // Second DELETE → idempotent, same state returned
    let resp =
        client.delete("/apis/example.io/v1/namespaces/default/Widget/idempotent-delete").await;
    assert_status(&resp, StatusCode::OK);
    let second: Value = parse_body(resp).await;
    let ts2 = second["system"]["deletionTimestamp"].as_str().unwrap_or("").to_string();
    let second_rv = second["system"]["resourceVersion"].as_u64().unwrap_or(0);

    // deletionTimestamp and resourceVersion should be unchanged
    assert_eq!(ts, ts2, "deletionTimestamp should not change on second delete");
    assert_eq!(first_rv, second_rv, "resourceVersion should not change on idempotent delete");

    Ok(())
}

// ──────────────────────────────────────────────
// Task 3.4: UPDATE integration tests
// ──────────────────────────────────────────────

/// Create with finalizers, DELETE (marks for deletion), try UPDATE spec → 409 ObjectBeingDeleted.
pub async fn test_update_spec_on_deleting_object_rejected(app: &TestApp) -> Result<(), String> {
    let client = app.client();
    register_widget_schema(&client).await;

    // Create with finalizers
    let body = serde_json::json!({
        "metadata": { "name": "update-spec-rejected", "finalizers": ["example.io/cleanup"] },
        "spec": { "color": "blue", "size": 10 }
    });
    let resp = client.post("/apis/example.io/v1/namespaces/default/Widget", body).await;
    assert_status(&resp, StatusCode::CREATED);
    let created: Value = parse_body(resp).await;
    let rv = created["system"]["resourceVersion"].as_u64().unwrap();
    let created_at = created["system"]["createdAt"].as_str().unwrap().to_string();
    let updated_at = created["system"]["updatedAt"].as_str().unwrap().to_string();

    // DELETE → marks for deletion
    let resp =
        client.delete("/apis/example.io/v1/namespaces/default/Widget/update-spec-rejected").await;
    assert_status(&resp, StatusCode::OK);

    // Try to update spec → should be rejected (only finalizer changes allowed)
    let update_body = serde_json::json!({
        "key": { "group": "example.io", "version": "v1", "kind": "Widget" },
        "metadata": { "name": "update-spec-rejected", "finalizers": ["example.io/cleanup"] },
        "system": { "resourceVersion": rv, "createdAt": created_at, "updatedAt": updated_at },
        "spec": { "color": "red", "size": 20 }
    });
    let resp = client
        .put("/apis/example.io/v1/namespaces/default/Widget/update-spec-rejected", update_body)
        .await;
    assert_status(&resp, StatusCode::CONFLICT);
    let err: Value = parse_body(resp).await;
    assert_eq!(err["code"], "ObjectBeingDeleted");

    Ok(())
}

/// Create with finalizers, DELETE, try UPDATE labels → 409 ObjectBeingDeleted.
pub async fn test_update_labels_on_deleting_object_rejected(app: &TestApp) -> Result<(), String> {
    let client = app.client();
    register_widget_schema(&client).await;

    // Create with finalizers and labels
    let body = serde_json::json!({
        "metadata": {
            "name": "update-labels-rejected",
            "finalizers": ["example.io/cleanup"],
            "labels": { "env": "prod" }
        },
        "spec": { "color": "blue", "size": 10 }
    });
    let resp = client.post("/apis/example.io/v1/namespaces/default/Widget", body).await;
    assert_status(&resp, StatusCode::CREATED);
    let created: Value = parse_body(resp).await;
    let rv = created["system"]["resourceVersion"].as_u64().unwrap();
    let created_at = created["system"]["createdAt"].as_str().unwrap().to_string();
    let updated_at = created["system"]["updatedAt"].as_str().unwrap().to_string();

    // DELETE → marks for deletion
    let resp =
        client.delete("/apis/example.io/v1/namespaces/default/Widget/update-labels-rejected").await;
    assert_status(&resp, StatusCode::OK);

    // Try to update labels → should be rejected
    let update_body = serde_json::json!({
        "key": { "group": "example.io", "version": "v1", "kind": "Widget" },
        "metadata": {
            "name": "update-labels-rejected",
            "finalizers": ["example.io/cleanup"],
            "labels": { "env": "staging" }
        },
        "system": { "resourceVersion": rv, "createdAt": created_at, "updatedAt": updated_at },
        "spec": { "color": "blue", "size": 10 }
    });
    let resp = client
        .put("/apis/example.io/v1/namespaces/default/Widget/update-labels-rejected", update_body)
        .await;
    assert_status(&resp, StatusCode::CONFLICT);
    let err: Value = parse_body(resp).await;
    assert_eq!(err["code"], "ObjectBeingDeleted");

    Ok(())
}

/// Create with finalizers, DELETE, UPDATE to remove one finalizer → 200, finalizers updated, deletionTimestamp still set.
pub async fn test_update_finalizers_on_deleting_object_allowed(
    app: &TestApp,
) -> Result<(), String> {
    let client = app.client();
    register_widget_schema(&client).await;

    // Create with multiple finalizers
    let body = serde_json::json!({
        "metadata": {
            "name": "update-finalizers-allowed",
            "finalizers": ["example.io/cleanup", "kapi.io/finalizer"]
        },
        "spec": { "color": "blue", "size": 10 }
    });
    let resp = client.post("/apis/example.io/v1/namespaces/default/Widget", body).await;
    assert_status(&resp, StatusCode::CREATED);
    let created: Value = parse_body(resp).await;
    let rv = created["system"]["resourceVersion"].as_u64().unwrap();
    let created_at = created["system"]["createdAt"].as_str().unwrap().to_string();
    let updated_at = created["system"]["updatedAt"].as_str().unwrap().to_string();

    // DELETE → marks for deletion
    let resp = client
        .delete("/apis/example.io/v1/namespaces/default/Widget/update-finalizers-allowed")
        .await;
    assert_status(&resp, StatusCode::OK);
    let deleted: Value = parse_body(resp).await;
    let ts = deleted["system"]["deletionTimestamp"].as_str().unwrap_or("").to_string();
    assert!(!ts.is_empty(), "deletionTimestamp should be set after delete");

    // Update to remove one finalizer → should succeed
    let update_body = serde_json::json!({
        "key": { "group": "example.io", "version": "v1", "kind": "Widget" },
        "metadata": {
            "name": "update-finalizers-allowed",
            "finalizers": ["example.io/cleanup"]
        },
        "system": { "resourceVersion": rv, "createdAt": created_at, "updatedAt": updated_at },
        "spec": { "color": "blue", "size": 10 }
    });
    let resp = client
        .put("/apis/example.io/v1/namespaces/default/Widget/update-finalizers-allowed", update_body)
        .await;
    assert_status(&resp, StatusCode::OK);
    let updated: Value = parse_body(resp).await;
    let finalizers = updated["metadata"]["finalizers"].as_array().unwrap();
    assert_eq!(finalizers.len(), 1, "expected 1 finalizer after removal");
    assert_eq!(finalizers[0], "example.io/cleanup");
    // deletionTimestamp should still be set
    assert!(
        updated["system"]["deletionTimestamp"].is_string(),
        "deletionTimestamp should still be set after finalizer update"
    );

    Ok(())
}

/// Create with finalizers, DELETE, UPDATE to empty finalizers → 200, object no longer exists (GET returns 404).
pub async fn test_update_finalizers_to_empty_triggers_hard_delete(
    app: &TestApp,
) -> Result<(), String> {
    let client = app.client();
    register_widget_schema(&client).await;

    // Create with finalizers
    let body = serde_json::json!({
        "metadata": { "name": "finalizers-to-empty", "finalizers": ["example.io/cleanup"] },
        "spec": { "color": "blue", "size": 10 }
    });
    let resp = client.post("/apis/example.io/v1/namespaces/default/Widget", body).await;
    assert_status(&resp, StatusCode::CREATED);
    let created: Value = parse_body(resp).await;
    let rv = created["system"]["resourceVersion"].as_u64().unwrap();
    let created_at = created["system"]["createdAt"].as_str().unwrap().to_string();
    let updated_at = created["system"]["updatedAt"].as_str().unwrap().to_string();

    // DELETE → marks for deletion
    let resp =
        client.delete("/apis/example.io/v1/namespaces/default/Widget/finalizers-to-empty").await;
    assert_status(&resp, StatusCode::OK);

    // Update to empty finalizers → should trigger hard delete
    let update_body = serde_json::json!({
        "key": { "group": "example.io", "version": "v1", "kind": "Widget" },
        "metadata": { "name": "finalizers-to-empty", "finalizers": [] },
        "system": { "resourceVersion": rv, "createdAt": created_at, "updatedAt": updated_at },
        "spec": { "color": "blue", "size": 10 }
    });
    let resp = client
        .put("/apis/example.io/v1/namespaces/default/Widget/finalizers-to-empty", update_body)
        .await;
    assert_status(&resp, StatusCode::OK);

    // GET → should be 404 (hard deleted)
    let resp =
        client.get("/apis/example.io/v1/namespaces/default/Widget/finalizers-to-empty").await;
    assert_status(&resp, StatusCode::NOT_FOUND);

    Ok(())
}

/// Create with finalizers, DELETE, try to add a new finalizer → 409 ObjectBeingDeleted.
pub async fn test_update_adds_finalizer_on_deleting_object_rejected(
    app: &TestApp,
) -> Result<(), String> {
    let client = app.client();
    register_widget_schema(&client).await;

    // Create with a finalizer
    let body = serde_json::json!({
        "metadata": { "name": "add-finalizer-rejected", "finalizers": ["example.io/cleanup"] },
        "spec": { "color": "blue", "size": 10 }
    });
    let resp = client.post("/apis/example.io/v1/namespaces/default/Widget", body).await;
    assert_status(&resp, StatusCode::CREATED);
    let created: Value = parse_body(resp).await;
    let rv = created["system"]["resourceVersion"].as_u64().unwrap();
    let created_at = created["system"]["createdAt"].as_str().unwrap().to_string();
    let updated_at = created["system"]["updatedAt"].as_str().unwrap().to_string();

    // DELETE → marks for deletion
    let resp =
        client.delete("/apis/example.io/v1/namespaces/default/Widget/add-finalizer-rejected").await;
    assert_status(&resp, StatusCode::OK);

    // Try to add a new finalizer → should be rejected
    let update_body = serde_json::json!({
        "key": { "group": "example.io", "version": "v1", "kind": "Widget" },
        "metadata": {
            "name": "add-finalizer-rejected",
            "finalizers": ["example.io/cleanup", "kapi.io/finalizer"]
        },
        "system": { "resourceVersion": rv, "createdAt": created_at, "updatedAt": updated_at },
        "spec": { "color": "blue", "size": 10 }
    });
    let resp = client
        .put("/apis/example.io/v1/namespaces/default/Widget/add-finalizer-rejected", update_body)
        .await;
    assert_status(&resp, StatusCode::CONFLICT);
    let err: Value = parse_body(resp).await;
    assert_eq!(err["code"], "ObjectBeingDeleted");

    Ok(())
}

// ──────────────────────────────────────────────
// Task 5.2: CREATE integration tests
// ──────────────────────────────────────────────

/// Create object with valid finalizers → success, response contains finalizers.
pub async fn test_create_with_valid_finalizers(app: &TestApp) -> Result<(), String> {
    let client = app.client();
    register_widget_schema(&client).await;

    let body = serde_json::json!({
        "metadata": {
            "name": "valid-finalizers",
            "finalizers": ["example.io/cleanup", "kapi.io/finalizer"]
        },
        "spec": { "color": "blue", "size": 10 }
    });
    let resp = client.post("/apis/example.io/v1/namespaces/default/Widget", body).await;
    assert_status(&resp, StatusCode::CREATED);
    let created: Value = parse_body(resp).await;
    let finalizers = created["metadata"]["finalizers"].as_array().unwrap();
    assert_eq!(finalizers.len(), 2);
    assert_eq!(finalizers[0], "example.io/cleanup");
    assert_eq!(finalizers[1], "kapi.io/finalizer");

    Ok(())
}

/// Create with invalid finalizer name (contains spaces) → 400 InvalidFinalizer.
pub async fn test_create_with_invalid_finalizer_name(app: &TestApp) -> Result<(), String> {
    let client = app.client();
    register_widget_schema(&client).await;

    let body = serde_json::json!({
        "metadata": {
            "name": "invalid-finalizer-name",
            "finalizers": ["invalid name with spaces"]
        },
        "spec": { "color": "blue", "size": 10 }
    });
    let resp = client.post("/apis/example.io/v1/namespaces/default/Widget", body).await;
    assert_status(&resp, StatusCode::BAD_REQUEST);
    let err: Value = parse_body(resp).await;
    assert_eq!(err["code"], "InvalidFinalizer");

    Ok(())
}

/// Create with 21 finalizers (exceeds max 20) → 400 InvalidFinalizer.
pub async fn test_create_with_too_many_finalizers(app: &TestApp) -> Result<(), String> {
    let client = app.client();
    register_widget_schema(&client).await;

    let too_many: Vec<String> = (0..21).map(|i| format!("finalizer-{}", i)).collect();
    let body = serde_json::json!({
        "metadata": {
            "name": "too-many-finalizers",
            "finalizers": too_many
        },
        "spec": { "color": "blue", "size": 10 }
    });
    let resp = client.post("/apis/example.io/v1/namespaces/default/Widget", body).await;
    assert_status(&resp, StatusCode::BAD_REQUEST);
    let err: Value = parse_body(resp).await;
    assert_eq!(err["code"], "InvalidFinalizer");

    Ok(())
}

// ──────────────────────────────────────────────
// Task 9.1-9.3: Edge cases
// ──────────────────────────────────────────────

/// Create with finalizers, DELETE (marks for deletion), try CREATE same name → 409 AlreadyExists.
pub async fn test_create_same_name_after_delete_with_finalizers(
    app: &TestApp,
) -> Result<(), String> {
    let client = app.client();
    register_widget_schema(&client).await;

    // Create with finalizers
    let body = serde_json::json!({
        "metadata": {
            "name": "same-name-after-delete",
            "finalizers": ["example.io/cleanup"]
        },
        "spec": { "color": "blue", "size": 10 }
    });
    let resp = client.post("/apis/example.io/v1/namespaces/default/Widget", body).await;
    assert_status(&resp, StatusCode::CREATED);

    // DELETE → marks for deletion (not hard delete because finalizers are present)
    let resp =
        client.delete("/apis/example.io/v1/namespaces/default/Widget/same-name-after-delete").await;
    assert_status(&resp, StatusCode::OK);

    // Try to create with same name → should fail with AlreadyExists
    let body = serde_json::json!({
        "metadata": {
            "name": "same-name-after-delete",
            "finalizers": ["example.io/cleanup"]
        },
        "spec": { "color": "red", "size": 20 }
    });
    let resp = client.post("/apis/example.io/v1/namespaces/default/Widget", body).await;
    assert_status(&resp, StatusCode::CONFLICT);
    let err: Value = parse_body(resp).await;
    assert_eq!(err["code"], "AlreadyExists");

    Ok(())
}

// ──────────────────────────────────────────────
// Task 10.1-10.2: Backward compatibility
// ──────────────────────────────────────────────

/// Verify that objects created without finalizers field deserialize correctly
/// (finalizers defaults to empty vec).
pub async fn test_backward_compat_deserialize_without_finalizers(
    app: &TestApp,
) -> Result<(), String> {
    let client = app.client();
    register_widget_schema(&client).await;

    // 1) HTTP API integration: create object without finalizers, verify response
    let body = serde_json::json!({
        "metadata": { "name": "no-finalizers-field" },
        "spec": { "color": "blue", "size": 10 }
    });
    let resp = client.post("/apis/example.io/v1/namespaces/default/Widget", body).await;
    assert_status(&resp, StatusCode::CREATED);
    let created: Value = parse_body(resp).await;

    // The response should include finalizers as an empty array
    assert!(
        created["metadata"]["finalizers"].is_array(),
        "finalizers should be present as an array"
    );
    assert!(
        created["metadata"]["finalizers"].as_array().unwrap().is_empty(),
        "finalizers should be empty when not provided"
    );

    // 2) Direct deserialization test: StoredObject JSON without finalizers field
    let json = serde_json::json!({
        "key": { "group": "example.io", "version": "v1", "kind": "Widget" },
        "metadata": { "name": "direct-test", "labels": {}, "annotations": {} },
        "system": {
            "resourceVersion": 1,
            "createdAt": "2024-01-01T00:00:00Z",
            "updatedAt": "2024-01-01T00:00:00Z"
        },
        "spec": { "color": "blue", "size": 10 }
    });
    let obj: kapi::object::types::StoredObject =
        serde_json::from_value(json).map_err(|e| e.to_string())?;
    assert!(
        obj.metadata.finalizers.is_empty(),
        "finalizers should default to empty vec when missing from JSON"
    );

    Ok(())
}

/// Verify that objects without deletionTimestamp deserialize correctly (defaults to None).
pub async fn test_backward_compat_deserialize_without_deletion_timestamp(
    _app: &TestApp,
) -> Result<(), String> {
    // Direct deserialization test: StoredObject JSON without deletionTimestamp
    let json = serde_json::json!({
        "key": { "group": "example.io", "version": "v1", "kind": "Widget" },
        "metadata": { "name": "direct-test", "labels": {}, "annotations": {}, "finalizers": [] },
        "system": {
            "resourceVersion": 1,
            "createdAt": "2024-01-01T00:00:00Z",
            "updatedAt": "2024-01-01T00:00:00Z"
        },
        "spec": { "color": "blue", "size": 10 }
    });
    let obj: kapi::object::types::StoredObject =
        serde_json::from_value(json).map_err(|e| e.to_string())?;
    assert!(
        obj.system.deletion_timestamp.is_none(),
        "deletionTimestamp should default to None when missing from JSON"
    );

    Ok(())
}
