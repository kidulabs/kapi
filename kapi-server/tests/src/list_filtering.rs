use axum::http::StatusCode;
use serde_json::Value;

use crate::{
    TestApp, assert_status, parse_body, register_widget_schema, widget, widget_with_labels,
};

pub async fn test_list_with_field_selector(app: &TestApp) -> Result<(), String> {
    let client = app.client();
    register_widget_schema(&client).await;

    // Create multiple widgets
    for name in ["foo", "bar", "baz"] {
        let resp = client
            .post("/apis/example.io/v1/namespaces/default/Widget", widget(name, "blue", 10))
            .await;
        assert_status(&resp, StatusCode::CREATED);
    }

    // List with fieldSelector=metadata.name=foo
    let resp = client
        .get("/apis/example.io/v1/namespaces/default/Widget?fieldSelector=metadata.name=foo")
        .await;
    assert_status(&resp, StatusCode::OK);
    let body: Value = parse_body(resp).await;
    let items = body["items"].as_array().unwrap();
    assert_eq!(items.len(), 1, "expected 1 item, got {}", items.len());
    assert_eq!(items[0]["metadata"]["name"], "foo");

    Ok(())
}

pub async fn test_list_with_label_selector(app: &TestApp) -> Result<(), String> {
    let client = app.client();
    register_widget_schema(&client).await;

    // Create widgets with different labels
    let resp = client
        .post(
            "/apis/example.io/v1/namespaces/default/Widget",
            widget_with_labels("web-1", "blue", 10, serde_json::json!({"app": "nginx"})),
        )
        .await;
    assert_status(&resp, StatusCode::CREATED);

    let resp = client
        .post(
            "/apis/example.io/v1/namespaces/default/Widget",
            widget_with_labels("web-2", "red", 20, serde_json::json!({"app": "apache"})),
        )
        .await;
    assert_status(&resp, StatusCode::CREATED);

    let resp = client
        .post(
            "/apis/example.io/v1/namespaces/default/Widget",
            widget_with_labels("web-3", "green", 30, serde_json::json!({})),
        )
        .await;
    assert_status(&resp, StatusCode::CREATED);

    // List with labelSelector=app=nginx
    let resp =
        client.get("/apis/example.io/v1/namespaces/default/Widget?labelSelector=app=nginx").await;
    assert_status(&resp, StatusCode::OK);
    let body: Value = parse_body(resp).await;
    let items = body["items"].as_array().unwrap();
    assert_eq!(items.len(), 1, "expected 1 item, got {}", items.len());
    assert_eq!(items[0]["metadata"]["name"], "web-1");

    Ok(())
}

pub async fn test_list_with_both_selectors(app: &TestApp) -> Result<(), String> {
    let client = app.client();
    register_widget_schema(&client).await;

    // Create widgets
    let resp = client
        .post(
            "/apis/example.io/v1/namespaces/default/Widget",
            widget_with_labels("target", "blue", 10, serde_json::json!({"app": "nginx"})),
        )
        .await;
    assert_status(&resp, StatusCode::CREATED);

    let resp = client
        .post(
            "/apis/example.io/v1/namespaces/default/Widget",
            widget_with_labels("other", "red", 20, serde_json::json!({"app": "nginx"})),
        )
        .await;
    assert_status(&resp, StatusCode::CREATED);

    let resp = client
        .post(
            "/apis/example.io/v1/namespaces/default/Widget",
            widget("target-nolabel", "green", 30),
        )
        .await;
    assert_status(&resp, StatusCode::CREATED);

    // List with both selectors
    let resp = client
        .get(
            "/apis/example.io/v1/namespaces/default/Widget?fieldSelector=metadata.name=target&labelSelector=app=nginx",
        )
        .await;
    assert_status(&resp, StatusCode::OK);
    let body: Value = parse_body(resp).await;
    let items = body["items"].as_array().unwrap();
    assert_eq!(items.len(), 1, "expected 1 item, got {}", items.len());
    assert_eq!(items[0]["metadata"]["name"], "target");

    Ok(())
}

pub async fn test_list_filter_with_pagination(app: &TestApp) -> Result<(), String> {
    let client = app.client();
    register_widget_schema(&client).await;

    // Create 10 widgets, only 2 have the label
    for i in 0..10 {
        let labels =
            if i < 2 { serde_json::json!({"app": "nginx"}) } else { serde_json::json!({}) };
        let resp = client
            .post(
                "/apis/example.io/v1/namespaces/default/Widget",
                widget_with_labels(&format!("obj-{i:02}"), "blue", 10, labels),
            )
            .await;
        assert_status(&resp, StatusCode::CREATED);
    }

    // Filter to 2, limit 10 → should return 2
    let resp = client
        .get("/apis/example.io/v1/namespaces/default/Widget?labelSelector=app=nginx&limit=10")
        .await;
    assert_status(&resp, StatusCode::OK);
    let body: Value = parse_body(resp).await;
    let items = body["items"].as_array().unwrap();
    assert_eq!(items.len(), 2, "expected 2 items, got {}", items.len());
    assert!(body["continueToken"].is_null(), "expected no continue token");

    Ok(())
}

pub async fn test_list_filter_no_matches(app: &TestApp) -> Result<(), String> {
    let client = app.client();
    register_widget_schema(&client).await;

    let resp = client
        .post("/apis/example.io/v1/namespaces/default/Widget", widget("existing", "blue", 10))
        .await;
    assert_status(&resp, StatusCode::CREATED);

    // Filter that matches nothing
    let resp = client
        .get(
            "/apis/example.io/v1/namespaces/default/Widget?fieldSelector=metadata.name=nonexistent",
        )
        .await;
    assert_status(&resp, StatusCode::OK);
    let body: Value = parse_body(resp).await;
    let items = body["items"].as_array().unwrap();
    assert!(items.is_empty(), "expected empty result");

    Ok(())
}

pub async fn test_watch_with_both_selectors_matching(app: &TestApp) -> Result<(), String> {
    use crate::watch_events;
    use kapi_server::object::types::WatchEventType;

    let client = app.client();
    register_widget_schema(&client).await;

    // Start watch with both selectors.
    // NOTE: watch_events spawns a task that calls client.get() to establish the
    // subscription. A brief yield ensures the runtime polls the spawned task
    // and the subscription is registered before we publish events.
    let mut rx = watch_events(
        &client,
        "/apis/example.io/v1/namespaces/default/Widget?watch=true&fieldSelector=metadata.name=watch-target&labelSelector=app=nginx",
    )
    .await;
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Create object matching both selectors
    let resp = client
        .post(
            "/apis/example.io/v1/namespaces/default/Widget",
            widget_with_labels("watch-target", "blue", 10, serde_json::json!({"app": "nginx"})),
        )
        .await;
    assert_status(&resp, StatusCode::CREATED);

    // Should receive the event
    let event = tokio::time::timeout(std::time::Duration::from_secs(2), rx.recv())
        .await
        .map_err(|_| "timeout waiting for watch event")?
        .ok_or("channel closed")?;

    assert!(matches!(event.event_type, WatchEventType::Added), "expected Added event type");
    assert_eq!(event.object.metadata.name, "watch-target");

    Ok(())
}

pub async fn test_watch_with_both_selectors_not_matching(app: &TestApp) -> Result<(), String> {
    use crate::watch_events;

    let client = app.client();
    register_widget_schema(&client).await;

    // Start watch with both selectors
    let mut rx = watch_events(
        &client,
        "/apis/example.io/v1/namespaces/default/Widget?watch=true&fieldSelector=metadata.name=watch-target&labelSelector=app=nginx",
    )
    .await;

    // Create object matching only field selector (wrong label)
    let resp = client
        .post(
            "/apis/example.io/v1/namespaces/default/Widget",
            widget_with_labels("watch-target", "blue", 10, serde_json::json!({"app": "apache"})),
        )
        .await;
    assert_status(&resp, StatusCode::CREATED);

    // Should NOT receive the event within timeout
    let result = tokio::time::timeout(std::time::Duration::from_millis(500), rx.recv()).await;
    if result.is_ok() {
        return Err("should not have received event for object matching only one selector".into());
    }

    Ok(())
}

pub async fn test_list_invalid_field_selector(app: &TestApp) -> Result<(), String> {
    let client = app.client();
    register_widget_schema(&client).await;

    // Invalid field selector (unsupported field)
    let resp =
        client.get("/apis/example.io/v1/namespaces/default/Widget?fieldSelector=metadata.namespace=default").await;
    assert_status(&resp, StatusCode::BAD_REQUEST);

    Ok(())
}

pub async fn test_list_invalid_label_selector(app: &TestApp) -> Result<(), String> {
    let client = app.client();
    register_widget_schema(&client).await;

    // Invalid label selector (empty value in equality)
    let resp = client.get("/apis/example.io/v1/namespaces/default/Widget?labelSelector=app=").await;
    assert_status(&resp, StatusCode::BAD_REQUEST);

    Ok(())
}
