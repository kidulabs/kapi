use axum::http::StatusCode;
use serde_json::Value;

use crate::{assert_status, parse_body, register_widget_schema, widget, TestApp};

pub async fn test_update_correct_rv(app: &TestApp) -> Result<(), String> {
    let client = app.client();

    register_widget_schema(&client).await;

    let resp = client
        .post("/apis/example.io/v1/Widget", widget("occ-correct", "red", 1))
        .await;
    assert_status(&resp, StatusCode::CREATED);
    let created: Value = parse_body(resp).await;
    let rv = created["metadata"]["resourceVersion"]
        .as_u64()
        .unwrap_or(0);
    let created_at = created["metadata"]["createdAt"]
        .as_str()
        .unwrap_or("")
        .to_string();
    let updated_at = created["metadata"]["updatedAt"]
        .as_str()
        .unwrap_or("")
        .to_string();
    assert!(rv > 0, "resourceVersion should be > 0");

    let update_body = serde_json::json!({
        "key": { "group": "example.io", "version": "v1", "kind": "Widget" },
        "metadata": { "name": "occ-correct", "resourceVersion": rv, "createdAt": created_at, "updatedAt": updated_at },
        "data": { "value": { "color": "blue", "size": 2 } }
    });

    let resp = client
        .put("/apis/example.io/v1/Widget/occ-correct", update_body)
        .await;
    assert_status(&resp, StatusCode::OK);
    let updated: Value = parse_body(resp).await;
    let new_rv = updated["metadata"]["resourceVersion"]
        .as_u64()
        .unwrap_or(0);
    assert!(
        new_rv > rv,
        "new resourceVersion should be greater than old"
    );

    Ok(())
}

pub async fn test_update_wrong_rv(app: &TestApp) -> Result<(), String> {
    let client = app.client();

    register_widget_schema(&client).await;

    let resp = client
        .post(
            "/apis/example.io/v1/Widget",
            widget("occ-wrong", "green", 3),
        )
        .await;
    assert_status(&resp, StatusCode::CREATED);
    let created: Value = parse_body(resp).await;
    let rv = created["metadata"]["resourceVersion"]
        .as_u64()
        .unwrap_or(0);
    let created_at = created["metadata"]["createdAt"]
        .as_str()
        .unwrap_or("")
        .to_string();
    let updated_at = created["metadata"]["updatedAt"]
        .as_str()
        .unwrap_or("")
        .to_string();
    assert!(rv > 0, "resourceVersion should be > 0");

    let wrong_rv = rv + 99;
    let update_body = serde_json::json!({
        "key": { "group": "example.io", "version": "v1", "kind": "Widget" },
        "metadata": { "name": "occ-wrong", "resourceVersion": wrong_rv, "createdAt": created_at, "updatedAt": updated_at },
        "data": { "value": { "color": "yellow", "size": 4 } }
    });

    let resp = client
        .put("/apis/example.io/v1/Widget/occ-wrong", update_body)
        .await;
    assert_status(&resp, StatusCode::CONFLICT);

    let body: Value = parse_body(resp).await;
    let error = body
        .get("error")
        .and_then(|e| e.as_str())
        .unwrap_or("");
    assert!(
        error.contains("conflict") || error.contains("Conflict"),
        "error message should mention conflict, got: {error}"
    );

    Ok(())
}
