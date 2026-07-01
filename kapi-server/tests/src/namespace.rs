//! Integration tests for namespace-scoped operations.
//!
//! Tests cross-namespace listing, same-name-in-different-namespaces,
//! scope validation, continue token with namespace, and more.

use axum::http::StatusCode;
use serde_json::Value;

use crate::{
    DEFAULT_NS, TestApp, assert_status, parse_body, register_namespace, register_schema_with_scope,
    register_widget_schema, widget, widget_collection_url, widget_item_url,
};

// ──────────────────────────────────────────────
// Task 10.2: Cross-namespace list
// ──────────────────────────────────────────────

/// GET /apis/example.io/v1/Widget (no namespace) returns objects from all namespaces.
pub async fn test_cross_namespace_list_all_namespaces(app: &TestApp) -> Result<(), String> {
    let client = app.client();
    register_widget_schema(&client).await;

    // Register the test namespaces first (default is auto-bootstrapped)
    for ns in &["ns-a", "ns-b", "ns-c"] {
        register_namespace(&client, ns).await;
    }

    // Create objects in different namespaces
    for ns in &["ns-a", "ns-b", "ns-c"] {
        let name = format!("widget-{ns}");
        let url = widget_collection_url(ns);
        let resp = client.post(&url, widget(&name, "blue", 1)).await;
        assert_status(&resp, StatusCode::CREATED);
    }

    // Also create an object in the default namespace
    let resp =
        client.post(&widget_collection_url(DEFAULT_NS), widget("default-widget", "red", 2)).await;
    assert_status(&resp, StatusCode::CREATED);

    // Cross-namespace list via cluster-scoped route (no namespace in URL)
    let resp = client.get("/apis/example.io/v1/Widget").await;
    assert_status(&resp, StatusCode::OK);
    let body: Value = parse_body(resp).await;
    let items = body["items"].as_array().unwrap();

    // Should see all 4 objects (from all namespaces)
    assert_eq!(items.len(), 4, "cross-namespace list should return all 4 objects");

    // Verify objects are sorted by (namespace, name)
    let pairs: Vec<(String, String)> = items
        .iter()
        .map(|item| {
            let ns = item["metadata"]["namespace"].as_str().unwrap_or("").to_string();
            let name = item["metadata"]["name"].as_str().unwrap_or("").to_string();
            (ns, name)
        })
        .collect();

    // Check sort order: "default" before "ns-a" before "ns-b" before "ns-c"
    assert_eq!(pairs[0], ("default".to_string(), "default-widget".to_string()));
    assert_eq!(pairs[1], ("ns-a".to_string(), "widget-ns-a".to_string()));
    assert_eq!(pairs[2], ("ns-b".to_string(), "widget-ns-b".to_string()));
    assert_eq!(pairs[3], ("ns-c".to_string(), "widget-ns-c".to_string()));

    Ok(())
}

/// Cross-namespace list with pagination using continue token.
pub async fn test_cross_namespace_list_with_pagination(app: &TestApp) -> Result<(), String> {
    let client = app.client();
    register_widget_schema(&client).await;
    register_namespace(&client, "ns-a").await;
    register_namespace(&client, "ns-b").await;

    // Create objects across namespaces with controlled names for ordering
    for ns in &["ns-a", "ns-b"] {
        for i in 0..3 {
            let name = format!("obj-{i}");
            let url = widget_collection_url(ns);
            let resp = client.post(&url, widget(&name, "green", i)).await;
            assert_status(&resp, StatusCode::CREATED);
        }
    }

    // Cross-namespace list with limit=2
    let resp = client.get("/apis/example.io/v1/Widget?limit=2").await;
    assert_status(&resp, StatusCode::OK);
    let page1: Value = parse_body(resp).await;
    let items1 = page1["items"].as_array().unwrap();
    assert_eq!(items1.len(), 2, "page1 should have 2 items");

    // First two items should be from ns-a: obj-0, obj-1
    assert_eq!(items1[0]["metadata"]["namespace"].as_str(), Some("ns-a"));
    assert_eq!(items1[0]["metadata"]["name"].as_str(), Some("obj-0"));
    assert_eq!(items1[1]["metadata"]["namespace"].as_str(), Some("ns-a"));
    assert_eq!(items1[1]["metadata"]["name"].as_str(), Some("obj-1"));

    let token = page1["continue_token"].as_str().unwrap_or("").to_string();
    assert!(!token.is_empty(), "page1 should have a continue token");

    // Fetch page 2
    let resp = client.get(&format!("/apis/example.io/v1/Widget?limit=2&continue={token}")).await;
    assert_status(&resp, StatusCode::OK);
    let page2: Value = parse_body(resp).await;
    let items2 = page2["items"].as_array().unwrap();
    assert_eq!(items2.len(), 2, "page2 should have 2 items");

    // Items should cross namespace boundary: ns-a obj-2, then ns-b obj-0
    assert_eq!(items2[0]["metadata"]["namespace"].as_str(), Some("ns-a"));
    assert_eq!(items2[0]["metadata"]["name"].as_str(), Some("obj-2"));
    assert_eq!(items2[1]["metadata"]["namespace"].as_str(), Some("ns-b"));
    assert_eq!(items2[1]["metadata"]["name"].as_str(), Some("obj-0"));

    let token2 = page2["continue_token"].as_str().unwrap_or("").to_string();
    assert!(!token2.is_empty(), "page2 should have a continue token");

    // Fetch page 3
    let resp = client.get(&format!("/apis/example.io/v1/Widget?limit=2&continue={token2}")).await;
    assert_status(&resp, StatusCode::OK);
    let page3: Value = parse_body(resp).await;
    let items3 = page3["items"].as_array().unwrap();
    assert_eq!(items3.len(), 2, "page3 should have 2 items");

    assert_eq!(items3[0]["metadata"]["namespace"].as_str(), Some("ns-b"));
    assert_eq!(items3[0]["metadata"]["name"].as_str(), Some("obj-1"));
    assert_eq!(items3[1]["metadata"]["namespace"].as_str(), Some("ns-b"));
    assert_eq!(items3[1]["metadata"]["name"].as_str(), Some("obj-2"));

    // No more pages
    assert!(page3["continue_token"].is_null(), "page3 should have no continue token");

    Ok(())
}

// ──────────────────────────────────────────────
// Task 10.3: Same name in different namespaces
// ──────────────────────────────────────────────

/// Objects with the same name can coexist in different namespaces.
pub async fn test_same_name_different_namespaces(app: &TestApp) -> Result<(), String> {
    let client = app.client();
    register_widget_schema(&client).await;
    register_namespace(&client, "ns-a").await;
    register_namespace(&client, "ns-b").await;

    let name = "shared-name";

    // Create object in ns-a
    let url_a = widget_collection_url("ns-a");
    let resp = client.post(&url_a, widget(name, "red", 1)).await;
    assert_status(&resp, StatusCode::CREATED);
    let created_a: Value = parse_body(resp).await;
    assert_eq!(created_a["metadata"]["namespace"].as_str(), Some("ns-a"));

    // Create same name in ns-b
    let url_b = widget_collection_url("ns-b");
    let resp = client.post(&url_b, widget(name, "blue", 2)).await;
    assert_status(&resp, StatusCode::CREATED);
    let created_b: Value = parse_body(resp).await;
    assert_eq!(created_b["metadata"]["namespace"].as_str(), Some("ns-b"));

    // Verify different specs
    assert_ne!(created_a["spec"]["color"], created_b["spec"]["color"]);

    // GET from ns-a returns the ns-a object
    let resp = client.get(&widget_item_url("ns-a", name)).await;
    assert_status(&resp, StatusCode::OK);
    let fetched_a: Value = parse_body(resp).await;
    assert_eq!(fetched_a["metadata"]["name"], name);
    assert_eq!(fetched_a["metadata"]["namespace"].as_str(), Some("ns-a"));
    assert_eq!(fetched_a["spec"]["color"], "red");

    // GET from ns-b returns the ns-b object
    let resp = client.get(&widget_item_url("ns-b", name)).await;
    assert_status(&resp, StatusCode::OK);
    let fetched_b: Value = parse_body(resp).await;
    assert_eq!(fetched_b["metadata"]["name"], name);
    assert_eq!(fetched_b["metadata"]["namespace"].as_str(), Some("ns-b"));
    assert_eq!(fetched_b["spec"]["color"], "blue");

    Ok(())
}

/// DELETE in one namespace does not affect an object with the same name in another namespace.
pub async fn test_delete_one_namespace_does_not_affect_other(app: &TestApp) -> Result<(), String> {
    let client = app.client();
    register_widget_schema(&client).await;
    register_namespace(&client, "ns-a").await;
    register_namespace(&client, "ns-b").await;

    let name = "shared-delete-test";

    // Create same name in both namespaces
    for ns in &["ns-a", "ns-b"] {
        let resp = client.post(&widget_collection_url(ns), widget(name, "green", 1)).await;
        assert_status(&resp, StatusCode::CREATED);
    }

    // DELETE from ns-a
    let resp = client.delete(&widget_item_url("ns-a", name)).await;
    assert_status(&resp, StatusCode::OK);

    // ns-a object should be gone
    let resp = client.get(&widget_item_url("ns-a", name)).await;
    assert_status(&resp, StatusCode::NOT_FOUND);

    // ns-b object should still exist
    let resp = client.get(&widget_item_url("ns-b", name)).await;
    assert_status(&resp, StatusCode::OK);
    let fetched: Value = parse_body(resp).await;
    assert_eq!(fetched["metadata"]["name"], name);
    assert_eq!(fetched["metadata"]["namespace"].as_str(), Some("ns-b"));

    Ok(())
}

// ──────────────────────────────────────────────
// Task 10.4: Scope validation
// ──────────────────────────────────────────────

/// Cluster-scoped kinds reject namespace in URL.
pub async fn test_cluster_scoped_rejects_namespace_in_url(app: &TestApp) -> Result<(), String> {
    let client = app.client();

    // Register a cluster-scoped Widget schema (scope=Cluster)
    register_schema_with_scope(
        &client,
        "cluster-test.io",
        "v1",
        "ClusterWidget",
        "Cluster",
        serde_json::json!({
            "type": "object",
            "properties": {
                "color": { "type": "string" },
            },
            "required": ["color"],
        }),
    )
    .await;

    // Try to create via namespace-scoped route → should fail
    let resp = client
        .post(
            "/apis/cluster-test.io/v1/namespaces/some-ns/ClusterWidget",
            serde_json::json!({
                "metadata": { "name": "test" },
                "spec": { "color": "red" },
            }),
        )
        .await;
    assert_status(&resp, StatusCode::BAD_REQUEST);
    let err: Value = parse_body(resp).await;
    assert!(
        err["error"].as_str().unwrap_or("").contains("does not accept namespace")
            || err["message"].as_str().unwrap_or("").contains("does not accept namespace"),
        "expected 'does not accept namespace' error, got: {:?}",
        err
    );

    // Cluster-scoped kind should work via cluster-scoped route
    let resp = client
        .post(
            "/apis/cluster-test.io/v1/ClusterWidget",
            serde_json::json!({
                "metadata": { "name": "test" },
                "spec": { "color": "red" },
            }),
        )
        .await;
    assert_status(&resp, StatusCode::CREATED);

    Ok(())
}

/// Namespaced kind defaults to "default" namespace when created via cluster-scoped route.
pub async fn test_namespaced_defaults_to_default_namespace(app: &TestApp) -> Result<(), String> {
    let client = app.client();
    register_widget_schema(&client).await;

    // Create via cluster-scoped route (no namespace in URL)
    let resp =
        client.post("/apis/example.io/v1/Widget", widget("default-ns-test", "blue", 1)).await;
    assert_status(&resp, StatusCode::CREATED);
    let created: Value = parse_body(resp).await;
    assert_eq!(
        created["metadata"]["namespace"].as_str(),
        Some("default"),
        "namespace should default to 'default'"
    );

    // Should be findable via namespace-scoped route in "default" namespace
    let resp = client.get(&widget_item_url(DEFAULT_NS, "default-ns-test")).await;
    assert_status(&resp, StatusCode::OK);

    Ok(())
}

/// Namespaced kind uses the provided namespace when specified via namespace-scoped route.
pub async fn test_namespaced_uses_provided_namespace(app: &TestApp) -> Result<(), String> {
    let client = app.client();
    register_widget_schema(&client).await;
    register_namespace(&client, "custom-ns").await;

    // Create via namespace-scoped route with a custom namespace
    let resp = client
        .post(&widget_collection_url("custom-ns"), widget("custom-ns-test", "green", 2))
        .await;
    assert_status(&resp, StatusCode::CREATED);
    let created: Value = parse_body(resp).await;
    assert_eq!(
        created["metadata"]["namespace"].as_str(),
        Some("custom-ns"),
        "namespace should be 'custom-ns'"
    );

    // Should be findable via namespace-scoped route in "custom-ns"
    let resp = client.get(&widget_item_url("custom-ns", "custom-ns-test")).await;
    assert_status(&resp, StatusCode::OK);

    // Should NOT be findable via namespace-scoped route in "default"
    let resp = client.get(&widget_item_url(DEFAULT_NS, "custom-ns-test")).await;
    assert_status(&resp, StatusCode::NOT_FOUND);

    Ok(())
}

// ──────────────────────────────────────────────
// Task 10.5: Continue token with namespace
// ──────────────────────────────────────────────

/// Continue token correctly resumes across namespace boundaries.
pub async fn test_continue_token_across_namespace_boundary(app: &TestApp) -> Result<(), String> {
    let client = app.client();
    register_widget_schema(&client).await;
    for ns in &["ns-a", "ns-b", "ns-c"] {
        register_namespace(&client, ns).await;
    }

    // Create objects across three namespaces with specific ordering
    // Namespace "ns-a": obj-aa, obj-ab
    // Namespace "ns-b": obj-ba, obj-bb
    // Namespace "ns-c": obj-ca, obj-cb
    let ns_objects = vec![
        ("ns-a", "obj-aa"),
        ("ns-a", "obj-ab"),
        ("ns-b", "obj-ba"),
        ("ns-b", "obj-bb"),
        ("ns-c", "obj-ca"),
        ("ns-c", "obj-cb"),
    ];

    for (ns, name) in &ns_objects {
        let resp = client.post(&widget_collection_url(ns), widget(name, "red", 1)).await;
        assert_status(&resp, StatusCode::CREATED);
    }

    // Paginate through all objects with limit=2
    let mut all_items: Vec<Value> = Vec::new();
    let mut continue_token: Option<String> = None;

    loop {
        let url = match &continue_token {
            Some(token) => format!("/apis/example.io/v1/Widget?limit=2&continue={token}"),
            None => "/apis/example.io/v1/Widget?limit=2".to_string(),
        };
        let resp = client.get(&url).await;
        assert_status(&resp, StatusCode::OK);
        let page: Value = parse_body(resp).await;
        let items = page["items"].as_array().unwrap();
        all_items.extend(items.iter().cloned());
        continue_token = page["continue_token"].as_str().map(|s| s.to_string());
        if continue_token.is_none() {
            break;
        }
    }

    // Should have all 6 objects
    assert_eq!(all_items.len(), 6, "pagination should return all 6 objects");

    // Verify order: sorted by (namespace, name)
    let pairs: Vec<(String, String)> = all_items
        .iter()
        .map(|item| {
            let ns = item["metadata"]["namespace"].as_str().unwrap_or("").to_string();
            let name = item["metadata"]["name"].as_str().unwrap_or("").to_string();
            (ns, name)
        })
        .collect();

    assert_eq!(pairs[0], ("ns-a".to_string(), "obj-aa".to_string()));
    assert_eq!(pairs[1], ("ns-a".to_string(), "obj-ab".to_string()));
    assert_eq!(pairs[2], ("ns-b".to_string(), "obj-ba".to_string()));
    assert_eq!(pairs[3], ("ns-b".to_string(), "obj-bb".to_string()));
    assert_eq!(pairs[4], ("ns-c".to_string(), "obj-ca".to_string()));
    assert_eq!(pairs[5], ("ns-c".to_string(), "obj-cb".to_string()));

    Ok(())
}
