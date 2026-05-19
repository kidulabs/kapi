use std::time::Duration;

use axum::http::StatusCode;
use serde_json::Value;
use tokio::time::timeout;

use crate::{
    assert_status, parse_body, register_widget_schema, watch_events, widget, widget_schema,
    TestApp, WatchEventType,
};

pub async fn test_watch_schema_added() -> Result<(), String> {
    let app = TestApp::new();
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

pub async fn test_watch_object_events() -> Result<(), String> {
    let app = TestApp::new();
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
        "metadata": { "name": "watch-test", "resourceVersion": rv, "createdAt": created_at, "updatedAt": updated_at },
        "data": { "value": { "color": "orange", "size": 99 } }
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

    let resp = client
        .delete("/apis/example.io/v1/Widget/watch-test")
        .await;
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
