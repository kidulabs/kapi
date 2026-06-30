use axum::http::StatusCode;
use serde_json::Value;

use crate::{TestApp, assert_status, parse_body, register_widget_schema, widget};

pub async fn test_delete_schema_no_objects(app: &TestApp) -> Result<(), String> {
    let client = app.client();

    register_widget_schema(&client).await;

    let resp = client.delete("/apis/kapi.io/v1/Schema/Widget.example.io.v1").await;
    assert_status(&resp, StatusCode::OK);

    Ok(())
}

pub async fn test_delete_schema_with_objects(app: &TestApp) -> Result<(), String> {
    let client = app.client();

    register_widget_schema(&client).await;

    for i in 0..2 {
        let name = format!("del-schema-obj-{i}");
        let resp = client
            .post("/apis/example.io/v1/namespaces/default/Widget", widget(&name, "red", i as i64))
            .await;
        assert_status(&resp, StatusCode::CREATED);
    }

    let resp = client.delete("/apis/kapi.io/v1/Schema/Widget.example.io.v1").await;
    assert_status(&resp, StatusCode::CONFLICT);

    let body: Value = parse_body(resp).await;
    let details = body.get("details").and_then(|d| d.as_object());
    assert!(details.is_some(), "error response should include details object");
    if let Some(d) = details {
        let kind = d.get("kind").and_then(|v| v.as_str()).unwrap_or("");
        assert_eq!(kind, "Widget", "expected kind 'Widget' in error details");
    }

    Ok(())
}
