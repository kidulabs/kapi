use axum::http::StatusCode;
use serde_json::Value;

use crate::{assert_status, parse_body, register_widget_schema, widget, TestApp};

pub async fn test_create_schema_then_object() -> Result<(), String> {
    let app = TestApp::new();
    let client = app.client();

    register_widget_schema(&client).await;

    let resp = client
        .post(
            "/apis/example.io/v1/Widget",
            widget("my-widget", "blue", 42),
        )
        .await;
    assert_status(&resp, StatusCode::CREATED);
    let created: Value = parse_body(resp).await;
    let name = created["metadata"]["name"]
        .as_str()
        .unwrap_or("")
        .to_string();
    assert_eq!(name, "my-widget", "expected name 'my-widget'");

    let resp = client.get("/apis/example.io/v1/Widget/my-widget").await;
    assert_status(&resp, StatusCode::OK);
    let fetched: Value = parse_body(resp).await;
    assert_eq!(
        fetched["metadata"]["name"], "my-widget",
        "GET returned wrong name"
    );
    assert_eq!(
        fetched["data"]["value"]["color"], "blue",
        "GET returned wrong color"
    );
    assert_eq!(
        fetched["data"]["value"]["size"], 42,
        "GET returned wrong size"
    );

    Ok(())
}

pub async fn test_full_crud_flow() -> Result<(), String> {
    let app = TestApp::new();
    let client = app.client();

    register_widget_schema(&client).await;

    let resp = client
        .post(
            "/apis/example.io/v1/Widget",
            widget("crud-widget", "red", 10),
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

    let update_body = serde_json::json!({
        "key": { "group": "example.io", "version": "v1", "kind": "Widget" },
        "metadata": { "name": "crud-widget", "resourceVersion": rv, "createdAt": created_at, "updatedAt": updated_at },
        "data": { "value": { "color": "green", "size": 20 } }
    });
    let resp = client
        .put("/apis/example.io/v1/Widget/crud-widget", update_body)
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

    let resp = client
        .delete("/apis/example.io/v1/Widget/crud-widget")
        .await;
    assert_status(&resp, StatusCode::OK);

    let resp = client
        .get("/apis/example.io/v1/Widget/crud-widget")
        .await;
    assert_status(&resp, StatusCode::NOT_FOUND);

    Ok(())
}

pub async fn test_list_single_page() -> Result<(), String> {
    let app = TestApp::new();
    let client = app.client();

    register_widget_schema(&client).await;

    for i in 0..2 {
        let name = format!("list-sp-{i}");
        let resp = client
            .post("/apis/example.io/v1/Widget", widget(&name, "red", i as i64))
            .await;
        assert_status(&resp, StatusCode::CREATED);
    }

    let resp = client.get("/apis/example.io/v1/Widget?limit=5").await;
    assert_status(&resp, StatusCode::OK);
    let list: Value = parse_body(resp).await;
    let items = list["items"].as_array().map(|a| a.len()).unwrap_or(0);
    assert_eq!(items, 2, "expected 2 items, got {items}");
    assert!(
        list["continue_token"].is_null(),
        "expected no continue token"
    );

    Ok(())
}

pub async fn test_list_two_pages() -> Result<(), String> {
    let app = TestApp::new();
    let client = app.client();

    register_widget_schema(&client).await;

    for i in 0..4 {
        let name = format!("list-tp-{i}");
        let resp = client
            .post("/apis/example.io/v1/Widget", widget(&name, "blue", i as i64))
            .await;
        assert_status(&resp, StatusCode::CREATED);
    }

    let resp = client.get("/apis/example.io/v1/Widget?limit=2").await;
    assert_status(&resp, StatusCode::OK);
    let page1: Value = parse_body(resp).await;
    let items1 = page1["items"].as_array().map(|a| a.len()).unwrap_or(0);
    assert_eq!(items1, 2, "page1 expected 2 items, got {items1}");
    let token = page1["continue_token"]
        .as_str()
        .unwrap_or("")
        .to_string();
    assert!(!token.is_empty(), "page1 should have continue token");

    let resp = client
        .get(&format!(
            "/apis/example.io/v1/Widget?limit=2&continue={token}"
        ))
        .await;
    assert_status(&resp, StatusCode::OK);
    let page2: Value = parse_body(resp).await;
    let items2 = page2["items"].as_array().map(|a| a.len()).unwrap_or(0);
    assert_eq!(items2, 2, "page2 expected 2 items, got {items2}");
    assert!(
        page2["continue_token"].is_null(),
        "page2 should have no continue token"
    );

    Ok(())
}

pub async fn test_list_resume_position() -> Result<(), String> {
    let app = TestApp::new();
    let client = app.client();

    register_widget_schema(&client).await;

    for name in ["a", "b", "c", "d"] {
        let resp = client
            .post("/apis/example.io/v1/Widget", widget(name, "green", 1))
            .await;
        assert_status(&resp, StatusCode::CREATED);
    }

    let resp = client.get("/apis/example.io/v1/Widget?limit=2").await;
    assert_status(&resp, StatusCode::OK);
    let page1: Value = parse_body(resp).await;
    let names1: Vec<&str> = page1["items"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|item| item["metadata"]["name"].as_str())
                .collect()
        })
        .unwrap_or_default();
    assert_eq!(names1, vec!["a", "b"], "page1 should have [a, b]");

    let token = page1["continue_token"]
        .as_str()
        .unwrap_or("")
        .to_string();

    let resp = client
        .get(&format!(
            "/apis/example.io/v1/Widget?limit=2&continue={token}"
        ))
        .await;
    assert_status(&resp, StatusCode::OK);
    let page2: Value = parse_body(resp).await;
    let names2: Vec<&str> = page2["items"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|item| item["metadata"]["name"].as_str())
                .collect()
        })
        .unwrap_or_default();
    assert_eq!(names2, vec!["c", "d"], "page2 should have [c, d]");
    assert!(
        page2["continue_token"].is_null(),
        "page2 should have no continue token"
    );

    Ok(())
}

pub async fn test_list_exhausted() -> Result<(), String> {
    let app = TestApp::new();
    let client = app.client();

    register_widget_schema(&client).await;

    let resp = client
        .post(
            "/apis/example.io/v1/Widget",
            widget("exhausted", "yellow", 1),
        )
        .await;
    assert_status(&resp, StatusCode::CREATED);

    let resp = client.get("/apis/example.io/v1/Widget?limit=10").await;
    assert_status(&resp, StatusCode::OK);
    let list: Value = parse_body(resp).await;
    let items = list["items"].as_array().map(|a| a.len()).unwrap_or(0);
    assert_eq!(items, 1, "expected 1 item, got {items}");
    assert!(
        list["continue_token"].is_null(),
        "last page should have no continue token"
    );

    Ok(())
}
