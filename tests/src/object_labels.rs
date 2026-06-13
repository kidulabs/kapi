use axum::http::StatusCode;
use serde_json::Value;

use crate::{TestApp, assert_status, parse_body, register_widget_schema, widget};

pub async fn test_create_object_with_labels(app: &TestApp) -> Result<(), String> {
    let client = app.client();

    register_widget_schema(&client).await;

    let body = serde_json::json!({
        "metadata": {
            "name": "labeled-widget",
            "labels": { "app": "nginx", "env": "prod" }
        },
        "spec": {
            "color": "blue",
            "size": 10
        }
    });

    let resp = client.post("/apis/example.io/v1/Widget", body).await;
    assert_status(&resp, StatusCode::CREATED);
    let created: Value = parse_body(resp).await;
    assert_eq!(
        created["metadata"]["name"], "labeled-widget",
        "expected name 'labeled-widget'"
    );
    assert_eq!(
        created["metadata"]["labels"]["app"], "nginx",
        "expected label app=nginx"
    );
    assert_eq!(
        created["metadata"]["labels"]["env"], "prod",
        "expected label env=prod"
    );

    // Verify labels survive a GET
    let resp = client
        .get("/apis/example.io/v1/Widget/labeled-widget")
        .await;
    assert_status(&resp, StatusCode::OK);
    let fetched: Value = parse_body(resp).await;
    assert_eq!(
        fetched["metadata"]["labels"]["app"], "nginx",
        "GET: expected label app=nginx"
    );
    assert_eq!(
        fetched["metadata"]["labels"]["env"], "prod",
        "GET: expected label env=prod"
    );

    Ok(())
}

pub async fn test_create_object_without_labels(app: &TestApp) -> Result<(), String> {
    let client = app.client();

    register_widget_schema(&client).await;

    let resp = client
        .post(
            "/apis/example.io/v1/Widget",
            widget("no-labels-widget", "red", 5),
        )
        .await;
    assert_status(&resp, StatusCode::CREATED);
    let created: Value = parse_body(resp).await;

    let labels = &created["metadata"]["labels"];
    assert!(labels.is_object(), "expected labels to be an object");
    assert!(
        labels.as_object().map(|o| o.is_empty()).unwrap_or(false),
        "expected labels to be empty, got: {labels}"
    );

    Ok(())
}

pub async fn test_update_object_labels(app: &TestApp) -> Result<(), String> {
    let client = app.client();

    register_widget_schema(&client).await;

    // Create with initial labels
    let create_body = serde_json::json!({
        "metadata": {
            "name": "update-labels",
            "labels": { "app": "nginx" }
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
        created["metadata"]["labels"]["app"], "nginx",
        "initial label app=nginx"
    );

    // Get the full StoredObject to use as update body
    let resp = client.get("/apis/example.io/v1/Widget/update-labels").await;
    assert_status(&resp, StatusCode::OK);
    let mut obj: Value = parse_body(resp).await;

    // Modify labels: change "app" and add "env"
    obj["metadata"]["labels"]["app"] = serde_json::json!("nginx2");
    obj["metadata"]["labels"]["env"] = serde_json::json!("prod");

    let resp = client
        .put("/apis/example.io/v1/Widget/update-labels", obj)
        .await;
    assert_status(&resp, StatusCode::OK);
    let updated: Value = parse_body(resp).await;
    let labels = &updated["metadata"]["labels"];
    assert_eq!(
        labels["app"], "nginx2",
        "expected modified label app=nginx2"
    );
    assert_eq!(labels["env"], "prod", "expected added label env=prod");

    // Verify via GET
    let resp = client.get("/apis/example.io/v1/Widget/update-labels").await;
    assert_status(&resp, StatusCode::OK);
    let fetched: Value = parse_body(resp).await;
    let labels = &fetched["metadata"]["labels"];
    assert_eq!(
        labels["app"], "nginx2",
        "GET: expected modified label app=nginx2"
    );
    assert_eq!(labels["env"], "prod", "GET: expected added label env=prod");

    Ok(())
}

pub async fn test_create_schema_with_labels(app: &TestApp) -> Result<(), String> {
    let client = app.client();

    let schema_body = serde_json::json!({
        "metadata": {
            "labels": { "team": "platform" }
        },
        "targetGroup": "labels-test.io",
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
        created["metadata"]["labels"]["team"], "platform",
        "expected label team=platform on schema"
    );

    // GET the schema and verify labels are persisted
    let resp = client
        .get("/apis/kapi.io/v1/Schema/Gadget.labels-test.io")
        .await;
    assert_status(&resp, StatusCode::OK);
    let fetched: Value = parse_body(resp).await;
    assert_eq!(
        fetched["metadata"]["labels"]["team"], "platform",
        "GET: expected label team=platform on schema"
    );

    Ok(())
}

pub async fn test_invalid_label_key_format(app: &TestApp) -> Result<(), String> {
    let client = app.client();

    let body = serde_json::json!({
        "metadata": {
            "name": "bad-key",
            "labels": { "invalid key!": "value" }
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
        err["code"], "InvalidLabel",
        "expected InvalidLabel error code"
    );

    Ok(())
}

pub async fn test_invalid_label_value_format(app: &TestApp) -> Result<(), String> {
    let client = app.client();

    let body = serde_json::json!({
        "metadata": {
            "name": "bad-value",
            "labels": { "key": "invalid value!" }
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
        err["code"], "InvalidLabel",
        "expected InvalidLabel error code"
    );

    Ok(())
}

pub async fn test_label_key_exceeds_length(app: &TestApp) -> Result<(), String> {
    let client = app.client();

    let long_key = "k".repeat(257);
    let body = serde_json::json!({
        "metadata": {
            "name": "long-key-label",
            "labels": { long_key: "value" }
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
        err["code"], "InvalidLabel",
        "expected InvalidLabel error code"
    );

    Ok(())
}

pub async fn test_label_value_exceeds_length(app: &TestApp) -> Result<(), String> {
    let client = app.client();

    let long_value = "v".repeat(257);
    let body = serde_json::json!({
        "metadata": {
            "name": "long-value-label",
            "labels": { "key": long_value }
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
        err["code"], "InvalidLabel",
        "expected InvalidLabel error code"
    );

    Ok(())
}
