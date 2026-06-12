use std::time::Duration;

use axum::http::StatusCode;
use serde_json::Value;
use tokio::time::timeout;

use crate::{
    TestApp, WatchEventType, assert_status, parse_body, register_widget_schema, watch_events,
    widget, widget_schema, widget_with_labels,
};

pub async fn test_watch_schema_added(app: &TestApp) -> Result<(), String> {
    let client = app.client();
    let mut events = watch_events(&client, "/apis/kapi.io/v1/Schema?watch=true").await;

    tokio::time::sleep(Duration::from_millis(100)).await;

    let resp = client
        .post("/apis/kapi.io/v1/Schema", widget_schema())
        .await;
    assert_status(&resp, StatusCode::CREATED);

    let event = timeout(Duration::from_secs(3), events.recv())
        .await
        .map_err(|_| "timeout waiting for Added event on Schema watch".to_string())?
        .ok_or("watch stream ended before receiving event".to_string())?;

    assert!(
        matches!(event.event_type, WatchEventType::Added),
        "expected Added event, got {:?}",
        event.event_type
    );
    assert_eq!(
        event.object.metadata.name, "Widget.example.io",
        "expected schema name 'Widget.example.io'"
    );

    Ok(())
}

pub async fn test_watch_object_events(app: &TestApp) -> Result<(), String> {
    let client = app.client();
    register_widget_schema(&client).await;

    let mut events = watch_events(&client, "/apis/example.io/v1/Widget?watch=true").await;

    tokio::time::sleep(Duration::from_millis(100)).await;

    let resp = client
        .post(
            "/apis/example.io/v1/Widget",
            widget("watch-test", "purple", 7),
        )
        .await;
    assert_status(&resp, StatusCode::CREATED);
    let created: Value = parse_body(resp).await;
    let rv = created["system"]["resourceVersion"].as_u64().unwrap_or(0);
    let created_at = created["system"]["createdAt"]
        .as_str()
        .unwrap_or("")
        .to_string();
    let updated_at = created["system"]["updatedAt"]
        .as_str()
        .unwrap_or("")
        .to_string();
    assert!(rv > 0, "resourceVersion should be > 0");

    let added = timeout(Duration::from_secs(3), events.recv())
        .await
        .map_err(|_| "timeout waiting for Added event".to_string())?
        .ok_or("watch stream ended before Added event".to_string())?;
    assert!(
        matches!(added.event_type, WatchEventType::Added),
        "expected Added event, got {:?}",
        added.event_type
    );

    let update_body = serde_json::json!({
        "key": { "group": "example.io", "version": "v1", "kind": "Widget" },
        "metadata": { "name": "watch-test" },
        "system": { "resourceVersion": rv, "createdAt": created_at, "updatedAt": updated_at },
        "spec": { "value": { "color": "orange", "size": 99 } }
    });
    let resp = client
        .put("/apis/example.io/v1/Widget/watch-test", update_body)
        .await;
    assert_status(&resp, StatusCode::OK);

    let modified = timeout(Duration::from_secs(3), events.recv())
        .await
        .map_err(|_| "timeout waiting for Modified event".to_string())?
        .ok_or("watch stream ended before Modified event".to_string())?;
    assert!(
        matches!(modified.event_type, WatchEventType::Modified),
        "expected Modified event, got {:?}",
        modified.event_type
    );

    let resp = client.delete("/apis/example.io/v1/Widget/watch-test").await;
    assert_status(&resp, StatusCode::OK);

    let deleted = timeout(Duration::from_secs(3), events.recv())
        .await
        .map_err(|_| "timeout waiting for Deleted event".to_string())?
        .ok_or("watch stream ended before Deleted event".to_string())?;
    assert!(
        matches!(deleted.event_type, WatchEventType::Deleted),
        "expected Deleted event, got {:?}",
        deleted.event_type
    );

    Ok(())
}

pub async fn test_watch_by_name_matching_events(app: &TestApp) -> Result<(), String> {
    let client = app.client();
    register_widget_schema(&client).await;

    // Watch only events for "my-target-widget"
    let mut events = watch_events(
        &client,
        "/apis/example.io/v1/Widget?watch=true&fieldSelector=metadata.name=my-target-widget",
    )
    .await;

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Create a non-target object — should NOT arrive on this watch
    let resp = client
        .post(
            "/apis/example.io/v1/Widget",
            widget("other-widget", "blue", 1),
        )
        .await;
    assert_status(&resp, StatusCode::CREATED);

    // Create the target object — should arrive
    let resp = client
        .post(
            "/apis/example.io/v1/Widget",
            widget("my-target-widget", "red", 2),
        )
        .await;
    assert_status(&resp, StatusCode::CREATED);

    // We should receive exactly one event — only for the target name
    let event = timeout(Duration::from_secs(3), events.recv())
        .await
        .map_err(|_| "timeout waiting for matching event".to_string())?
        .ok_or("watch stream ended before event".to_string())?;

    assert_eq!(
        event.object.metadata.name, "my-target-widget",
        "expected only events for target widget"
    );

    Ok(())
}

pub async fn test_watch_by_name_non_matching_filtered(app: &TestApp) -> Result<(), String> {
    let client = app.client();
    register_widget_schema(&client).await;

    // Watch only events for "target"
    let mut events = watch_events(
        &client,
        "/apis/example.io/v1/Widget?watch=true&fieldSelector=metadata.name=target",
    )
    .await;

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Create a non-target object — should be filtered out
    let resp = client
        .post("/apis/example.io/v1/Widget", widget("other", "blue", 1))
        .await;
    assert_status(&resp, StatusCode::CREATED);

    // Verify no event arrived for the non-target object
    let result = timeout(Duration::from_millis(500), events.recv()).await;
    assert!(
        result.is_err(),
        "should not receive event for non-target object"
    );

    Ok(())
}

pub async fn test_watch_invalid_field_selector(app: &TestApp) -> Result<(), String> {
    let client = app.client();
    register_widget_schema(&client).await;

    // Unsupported field
    let resp = client
        .get("/apis/example.io/v1/Widget?watch=true&fieldSelector=metadata.namespace=default")
        .await;
    assert_eq!(
        resp.status(),
        StatusCode::BAD_REQUEST,
        "expected 400 for unsupported field"
    );

    // Malformed field selector
    let resp = client
        .get("/apis/example.io/v1/Widget?watch=true&fieldSelector=invalid-format")
        .await;
    assert_eq!(
        resp.status(),
        StatusCode::BAD_REQUEST,
        "expected 400 for malformed field selector"
    );

    Ok(())
}

pub async fn test_field_selector_on_non_watch_returns_400(app: &TestApp) -> Result<(), String> {
    let client = app.client();
    register_widget_schema(&client).await;

    // fieldSelector without watch=true
    let resp = client
        .get("/apis/example.io/v1/Widget?fieldSelector=metadata.name=my-widget")
        .await;
    assert_eq!(
        resp.status(),
        StatusCode::BAD_REQUEST,
        "expected 400 for fieldSelector on non-watch request"
    );

    Ok(())
}

pub async fn test_watch_by_name_and_watch_all_simultaneously(app: &TestApp) -> Result<(), String> {
    let client = app.client();
    register_widget_schema(&client).await;

    // Two simultaneous watches: one filtered by name, one watching all
    let mut named_events = watch_events(
        &client,
        "/apis/example.io/v1/Widget?watch=true&fieldSelector=metadata.name=named-one",
    )
    .await;
    let mut all_events = watch_events(&client, "/apis/example.io/v1/Widget?watch=true").await;

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Create the named object — both watchers should receive it
    let resp = client
        .post(
            "/apis/example.io/v1/Widget",
            widget("named-one", "green", 3),
        )
        .await;
    assert_status(&resp, StatusCode::CREATED);

    // All watcher receives the event
    let all_event = timeout(Duration::from_secs(3), all_events.recv())
        .await
        .map_err(|_| "timeout waiting for all watcher event".to_string())?
        .ok_or("all watcher stream ended")?;
    assert_eq!(all_event.object.metadata.name, "named-one");

    // Named watcher also receives it
    let named_event = timeout(Duration::from_secs(3), named_events.recv())
        .await
        .map_err(|_| "timeout waiting for named watcher event".to_string())?
        .ok_or("named watcher stream ended")?;
    assert_eq!(named_event.object.metadata.name, "named-one");

    // Create an unnamed object — only the all watcher should receive it
    let resp = client
        .post("/apis/example.io/v1/Widget", widget("other", "yellow", 4))
        .await;
    assert_status(&resp, StatusCode::CREATED);

    // All watcher receives the event for "other"
    let all_event2 = timeout(Duration::from_secs(3), all_events.recv())
        .await
        .map_err(|_| "timeout waiting for all watcher second event".to_string())?
        .ok_or("all watcher stream ended")?;
    assert_eq!(all_event2.object.metadata.name, "other");

    // Named watcher should NOT receive the event for "other"
    let timeout_result = timeout(Duration::from_millis(500), named_events.recv()).await;
    assert!(
        timeout_result.is_err(),
        "named watcher should not receive event for 'other'"
    );

    Ok(())
}

pub async fn test_watcher_cleanup_on_client_disconnect(app: &TestApp) -> Result<(), String> {
    use kapi::event::EventPublisher;
    use kapi::object::types::{ObjectMeta, SpecData, StoredObject, SystemMetadata, WatchEvent, WatchFilter};
    use kapi::store::ResourceKey;

    let key = ResourceKey {
        group: "example.io".to_string(),
        version: "v1".to_string(),
        kind: "Widget".to_string(),
    };

    let event_bus: &dyn EventPublisher = &*app.event_bus;

    // Subscribe a watcher with WatchFilter::All
    let stream = event_bus.subscribe(&key, WatchFilter::All);

    // Verify a watcher was created
    let count = event_bus.watcher_count(&key);
    assert_eq!(count, Some(1), "watcher should exist after subscribe");

    // Drop the WatchStream (simulating HTTP client disconnect).
    // The mpsc::Receiver inside WatchStream is dropped, but the Watcher
    // (with its mpsc::Sender) stays in the Vec until the next publish().
    drop(stream);

    // Publish an event to trigger lazy retain() cleanup.
    // retain() iterates watchers; try_send returns Closed on dead receivers.
    // Use app.store to create a stored object so we don't need chrono for
    // SystemMetadata (the store populates it).
    let stored = app
        .store
        .create(StoredObject {
            key: key.clone(),
            metadata: ObjectMeta {
                name: "cleanup-test".to_string(),
                labels: std::collections::HashMap::new(),
            },
            system: SystemMetadata::initial(),
            spec: SpecData { value: serde_json::json!({}) },
            status: None,
        })
        .await
        .map_err(|e| format!("store create failed: {e}"))?;

    event_bus.publish(
        &key,
        WatchEvent {
            event_type: WatchEventType::Added,
            object: stored,
        },
    );

    // After publish, the dead watcher should be cleaned up
    let count = event_bus.watcher_count(&key).unwrap_or(0);
    assert_eq!(count, 0, "dead watcher should be cleaned up on publish");

    Ok(())
}

// Label selector watch tests

pub async fn test_watch_by_label_selector_matching(app: &TestApp) -> Result<(), String> {
    let client = app.client();
    register_widget_schema(&client).await;

    let mut events = watch_events(
        &client,
        "/apis/example.io/v1/Widget?watch=true&labelSelector=app=nginx",
    )
    .await;

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Create object with matching labels
    let resp = client
        .post(
            "/apis/example.io/v1/Widget",
            widget_with_labels("matching", "blue", 1, serde_json::json!({"app": "nginx"})),
        )
        .await;
    assert_status(&resp, StatusCode::CREATED);

    let event = timeout(Duration::from_secs(3), events.recv())
        .await
        .map_err(|_| "timeout waiting for matching label event".to_string())?
        .ok_or("watch stream ended before event".to_string())?;

    assert_eq!(
        event.object.metadata.name, "matching",
        "expected event for matching object"
    );

    Ok(())
}

pub async fn test_watch_by_label_selector_non_matching(app: &TestApp) -> Result<(), String> {
    let client = app.client();
    register_widget_schema(&client).await;

    let mut events = watch_events(
        &client,
        "/apis/example.io/v1/Widget?watch=true&labelSelector=app=nginx",
    )
    .await;

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Create object with non-matching labels
    let resp = client
        .post(
            "/apis/example.io/v1/Widget",
            widget_with_labels(
                "non-matching",
                "red",
                2,
                serde_json::json!({"app": "apache"}),
            ),
        )
        .await;
    assert_status(&resp, StatusCode::CREATED);

    // Verify no event arrived
    let result = timeout(Duration::from_millis(500), events.recv()).await;
    assert!(
        result.is_err(),
        "should not receive event for non-matching labels"
    );

    Ok(())
}

pub async fn test_watch_by_label_selector_and_combinator(app: &TestApp) -> Result<(), String> {
    let client = app.client();
    register_widget_schema(&client).await;

    let mut events = watch_events(
        &client,
        "/apis/example.io/v1/Widget?watch=true&labelSelector=app=nginx,env=prod",
    )
    .await;

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Create object with both labels
    let resp = client
        .post(
            "/apis/example.io/v1/Widget",
            widget_with_labels(
                "both-labels",
                "green",
                3,
                serde_json::json!({"app": "nginx", "env": "prod"}),
            ),
        )
        .await;
    assert_status(&resp, StatusCode::CREATED);

    let event = timeout(Duration::from_secs(3), events.recv())
        .await
        .map_err(|_| "timeout waiting for AND combinator event".to_string())?
        .ok_or("watch stream ended before event".to_string())?;

    assert_eq!(
        event.object.metadata.name, "both-labels",
        "expected event for object with both labels"
    );

    Ok(())
}

pub async fn test_watch_by_label_selector_not_exists(app: &TestApp) -> Result<(), String> {
    let client = app.client();
    register_widget_schema(&client).await;

    let mut events = watch_events(
        &client,
        "/apis/example.io/v1/Widget?watch=true&labelSelector=!experimental",
    )
    .await;

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Create object without experimental label
    let resp = client
        .post(
            "/apis/example.io/v1/Widget",
            widget_with_labels(
                "no-experimental",
                "yellow",
                4,
                serde_json::json!({"app": "nginx"}),
            ),
        )
        .await;
    assert_status(&resp, StatusCode::CREATED);

    let event = timeout(Duration::from_secs(3), events.recv())
        .await
        .map_err(|_| "timeout waiting for !experimental event".to_string())?
        .ok_or("watch stream ended before event".to_string())?;

    assert_eq!(
        event.object.metadata.name, "no-experimental",
        "expected event for object without experimental label"
    );

    Ok(())
}

pub async fn test_watch_invalid_label_selector(app: &TestApp) -> Result<(), String> {
    let client = app.client();
    register_widget_schema(&client).await;

    // Malformed selector (empty segment)
    let resp = client
        .get("/apis/example.io/v1/Widget?watch=true&labelSelector=app=nginx,,env=prod")
        .await;
    assert_eq!(
        resp.status(),
        StatusCode::BAD_REQUEST,
        "expected 400 for empty segment in label selector"
    );

    // Empty value
    let resp = client
        .get("/apis/example.io/v1/Widget?watch=true&labelSelector=app=")
        .await;
    assert_eq!(
        resp.status(),
        StatusCode::BAD_REQUEST,
        "expected 400 for empty value in label selector"
    );

    Ok(())
}

pub async fn test_watch_empty_label_selector(app: &TestApp) -> Result<(), String> {
    let client = app.client();
    register_widget_schema(&client).await;

    let mut events = watch_events(
        &client,
        "/apis/example.io/v1/Widget?watch=true&labelSelector=",
    )
    .await;

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Create any object — should receive event (empty selector matches all)
    let resp = client
        .post(
            "/apis/example.io/v1/Widget",
            widget("empty-selector", "purple", 5),
        )
        .await;
    assert_status(&resp, StatusCode::CREATED);

    let event = timeout(Duration::from_secs(3), events.recv())
        .await
        .map_err(|_| "timeout waiting for empty selector event".to_string())?
        .ok_or("watch stream ended before event".to_string())?;

    assert_eq!(
        event.object.metadata.name, "empty-selector",
        "expected event for empty selector (matches all)"
    );

    Ok(())
}

pub async fn test_label_selector_on_non_watch_returns_400(app: &TestApp) -> Result<(), String> {
    let client = app.client();
    register_widget_schema(&client).await;

    // labelSelector without watch=true
    let resp = client
        .get("/apis/example.io/v1/Widget?labelSelector=app=nginx")
        .await;
    assert_eq!(
        resp.status(),
        StatusCode::BAD_REQUEST,
        "expected 400 for labelSelector on non-watch request"
    );

    Ok(())
}
