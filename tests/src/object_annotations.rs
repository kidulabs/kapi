use axum::http::StatusCode;
use serde_json::Value;

use crate::{TestApp, assert_status, parse_body, register_widget_schema, widget};

pub async fn test_create_object_with_annotations(app: &TestApp) -> Result<(), String> {
    let client = app.client();

    register_widget_schema(&client).await;

    let body = serde_json::json!({
        "metadata": {
            "name": "annotated-widget",
            "annotations": { "description": "my widget", "owner": "team-platform" }
        },
        "spec": {
            "color": "blue",
            "size": 10
        }
    });

    let resp = client.post("/apis/example.io/v1/Widget", body).await;
    assert_status(&resp, StatusCode::CREATED);
    let created: Value = parse_body(resp).await;
    assert_eq!(created["metadata"]["name"], "annotated-widget", "expected name 'annotated-widget'");
    assert_eq!(
        created["metadata"]["annotations"]["description"], "my widget",
        "expected annotation description=my widget"
    );
    assert_eq!(
        created["metadata"]["annotations"]["owner"], "team-platform",
        "expected annotation owner=team-platform"
    );

    // Verify annotations survive a GET
    let resp = client.get("/apis/example.io/v1/Widget/annotated-widget").await;
    assert_status(&resp, StatusCode::OK);
    let fetched: Value = parse_body(resp).await;
    assert_eq!(
        fetched["metadata"]["annotations"]["description"], "my widget",
        "GET: expected annotation description=my widget"
    );
    assert_eq!(
        fetched["metadata"]["annotations"]["owner"], "team-platform",
        "GET: expected annotation owner=team-platform"
    );

    Ok(())
}

pub async fn test_create_object_without_annotations(app: &TestApp) -> Result<(), String> {
    let client = app.client();

    register_widget_schema(&client).await;

    let resp =
        client.post("/apis/example.io/v1/Widget", widget("no-annotations-widget", "red", 5)).await;
    assert_status(&resp, StatusCode::CREATED);
    let created: Value = parse_body(resp).await;

    let annotations = &created["metadata"]["annotations"];
    assert!(annotations.is_object(), "expected annotations to be an object");
    assert!(
        annotations.as_object().map(|o| o.is_empty()).unwrap_or(false),
        "expected annotations to be empty, got: {annotations}"
    );

    Ok(())
}

pub async fn test_update_object_annotations(app: &TestApp) -> Result<(), String> {
    let client = app.client();

    register_widget_schema(&client).await;

    // Create with initial annotations
    let create_body = serde_json::json!({
        "metadata": {
            "name": "update-annotations",
            "annotations": { "description": "old widget" }
        },
        "spec": {
            "color": "blue",
            "size": 10
        }
    });

    let resp = client.post("/apis/example.io/v1/Widget", create_body).await;
    assert_status(&resp, StatusCode::CREATED);
    let created: Value = parse_body(resp).await;
    assert_eq!(
        created["metadata"]["annotations"]["description"], "old widget",
        "initial annotation description=old widget"
    );

    // Get the full StoredObject to use as update body
    let resp = client.get("/apis/example.io/v1/Widget/update-annotations").await;
    assert_status(&resp, StatusCode::OK);
    let mut obj: Value = parse_body(resp).await;

    // Modify annotations: change "description" and add "owner"
    obj["metadata"]["annotations"]["description"] = serde_json::json!("new widget");
    obj["metadata"]["annotations"]["owner"] = serde_json::json!("team");

    let resp = client.put("/apis/example.io/v1/Widget/update-annotations", obj).await;
    assert_status(&resp, StatusCode::OK);
    let updated: Value = parse_body(resp).await;
    let annotations = &updated["metadata"]["annotations"];
    assert_eq!(
        annotations["description"], "new widget",
        "expected modified annotation description=new widget"
    );
    assert_eq!(annotations["owner"], "team", "expected added annotation owner=team");

    // Verify via GET
    let resp = client.get("/apis/example.io/v1/Widget/update-annotations").await;
    assert_status(&resp, StatusCode::OK);
    let fetched: Value = parse_body(resp).await;
    let annotations = &fetched["metadata"]["annotations"];
    assert_eq!(
        annotations["description"], "new widget",
        "GET: expected modified annotation description=new widget"
    );
    assert_eq!(annotations["owner"], "team", "GET: expected added annotation owner=team");

    Ok(())
}

pub async fn test_create_schema_with_annotations(app: &TestApp) -> Result<(), String> {
    let client = app.client();

    let schema_body = serde_json::json!({
        "metadata": {
            "annotations": { "team": "platform", "docs": "https://example.com/docs" }
        },
        "targetGroup": "annotations-test.io",
        "targetVersion": "v1",
        "targetKind": "Gadget",
        "specSchema": {
            "type": "object",
            "properties": {
                "color": { "type": "string" }
            },
            "required": ["color"]
        }
    });

    let resp = client.post("/apis/kapi.io/v1/Schema", schema_body).await;
    assert_status(&resp, StatusCode::CREATED);
    let created: Value = parse_body(resp).await;
    assert_eq!(
        created["metadata"]["annotations"]["team"], "platform",
        "expected annotation team=platform on schema"
    );
    assert_eq!(
        created["metadata"]["annotations"]["docs"], "https://example.com/docs",
        "expected annotation docs on schema"
    );

    // GET the schema and verify annotations are persisted
    let resp = client.get("/apis/kapi.io/v1/Schema/Gadget.annotations-test.io").await;
    assert_status(&resp, StatusCode::OK);
    let fetched: Value = parse_body(resp).await;
    assert_eq!(
        fetched["metadata"]["annotations"]["team"], "platform",
        "GET: expected annotation team=platform on schema"
    );

    Ok(())
}

pub async fn test_invalid_annotation_key_empty(app: &TestApp) -> Result<(), String> {
    let client = app.client();

    register_widget_schema(&client).await;

    let body = serde_json::json!({
        "metadata": {
            "name": "bad-annotation",
            "annotations": { "": "value" }
        },
        "spec": {
            "color": "blue",
            "size": 10
        }
    });

    let resp = client.post("/apis/example.io/v1/Widget", body).await;
    assert_status(&resp, StatusCode::BAD_REQUEST);
    let err: Value = parse_body(resp).await;
    assert_eq!(err["code"], "InvalidAnnotation", "expected InvalidAnnotation error code");

    Ok(())
}

pub async fn test_invalid_annotation_key_too_long(app: &TestApp) -> Result<(), String> {
    let client = app.client();

    register_widget_schema(&client).await;

    let long_key = "k".repeat(257);
    let body = serde_json::json!({
        "metadata": {
            "name": "long-key-annotation",
            "annotations": { long_key: "value" }
        },
        "spec": {
            "color": "blue",
            "size": 10
        }
    });

    let resp = client.post("/apis/example.io/v1/Widget", body).await;
    assert_status(&resp, StatusCode::BAD_REQUEST);
    let err: Value = parse_body(resp).await;
    assert_eq!(err["code"], "InvalidAnnotation", "expected InvalidAnnotation error code");

    Ok(())
}

pub async fn test_invalid_annotation_value_non_string(app: &TestApp) -> Result<(), String> {
    let client = app.client();

    register_widget_schema(&client).await;

    let body = serde_json::json!({
        "metadata": {
            "name": "bad-value-type",
            "annotations": { "key": 123 }
        },
        "spec": {
            "color": "blue",
            "size": 10
        }
    });

    let resp = client.post("/apis/example.io/v1/Widget", body).await;
    assert_status(&resp, StatusCode::BAD_REQUEST);
    let err: Value = parse_body(resp).await;
    assert_eq!(err["code"], "InvalidAnnotation", "expected InvalidAnnotation error code");

    Ok(())
}

pub async fn test_invalid_annotations_format(app: &TestApp) -> Result<(), String> {
    let client = app.client();

    register_widget_schema(&client).await;

    let body = serde_json::json!({
        "metadata": {
            "name": "bad-format",
            "annotations": "not-an-object"
        },
        "spec": {
            "color": "blue",
            "size": 10
        }
    });

    let resp = client.post("/apis/example.io/v1/Widget", body).await;
    assert_status(&resp, StatusCode::BAD_REQUEST);
    let err: Value = parse_body(resp).await;
    assert_eq!(err["code"], "InvalidAnnotation", "expected InvalidAnnotation error code");

    Ok(())
}

pub async fn test_annotation_size_limit(app: &TestApp) -> Result<(), String> {
    let client = app.client();

    register_widget_schema(&client).await;

    let large_value = "x".repeat(256 * 1024); // > 256KB
    let body = serde_json::json!({
        "metadata": {
            "name": "too-large",
            "annotations": { "key": large_value }
        },
        "spec": {
            "color": "blue",
            "size": 10
        }
    });

    let resp = client.post("/apis/example.io/v1/Widget", body).await;
    assert_status(&resp, StatusCode::BAD_REQUEST);
    let err: Value = parse_body(resp).await;
    assert_eq!(
        err["code"], "InvalidAnnotation",
        "expected InvalidAnnotation error code for size limit"
    );

    Ok(())
}

pub async fn test_annotation_size_limit_on_update(app: &TestApp) -> Result<(), String> {
    let client = app.client();

    register_widget_schema(&client).await;

    // Create with valid annotations
    let create_body = serde_json::json!({
        "metadata": {
            "name": "size-limit-update",
            "annotations": { "description": "small" }
        },
        "spec": {
            "color": "blue",
            "size": 10
        }
    });

    let resp = client.post("/apis/example.io/v1/Widget", create_body).await;
    assert_status(&resp, StatusCode::CREATED);

    // Get the full StoredObject to use as update body
    let resp = client.get("/apis/example.io/v1/Widget/size-limit-update").await;
    assert_status(&resp, StatusCode::OK);
    let mut obj: Value = parse_body(resp).await;

    // Modify annotations to exceed 256KB
    let large_value = "x".repeat(256 * 1024); // > 256KB
    obj["metadata"]["annotations"]["large"] = serde_json::json!(large_value);

    let resp = client.put("/apis/example.io/v1/Widget/size-limit-update", obj).await;
    assert_status(&resp, StatusCode::BAD_REQUEST);
    let err: Value = parse_body(resp).await;
    assert_eq!(
        err["code"], "InvalidAnnotation",
        "expected InvalidAnnotation error code for size limit on update"
    );

    Ok(())
}
