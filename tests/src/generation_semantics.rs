use axum::http::StatusCode;
use serde_json::Value;

use crate::{TestApp, assert_status, parse_body, widget_with_labels};

fn widget_schema_with_status() -> Value {
    serde_json::json!({
        "targetGroup": "example.io",
        "targetVersion": "v1",
        "targetKind": "Widget",
        "specSchema": {
            "type": "object",
            "properties": {
                "color": { "type": "string" },
                "size": { "type": "integer" }
            },
            "required": ["color", "size"]
        },
        "statusSchema": {
            "type": "object",
            "properties": {
                "phase": { "type": "string" },
                "message": { "type": "string" }
            }
        }
    })
}

/// Generation starts at 1 on create, bumps only on spec changes,
/// and stays constant on metadata-only and status-only updates.
pub async fn test_generation_semantics(app: &TestApp) -> Result<(), String> {
    let client = app.client();

    // Register schema with status support
    let resp = client
        .post("/apis/kapi.io/v1/Schema", widget_schema_with_status())
        .await;
    assert_status(&resp, StatusCode::CREATED);

    // 1. Create an object and verify generation == 1
    let resp = client
        .post(
            "/apis/example.io/v1/Widget",
            widget_with_labels(
                "gen-widget",
                "blue",
                10,
                serde_json::json!({"app": "nginx"}),
            ),
        )
        .await;
    assert_status(&resp, StatusCode::CREATED);
    let created: Value = parse_body(resp).await;
    let generation_val = created["system"]["generation"]
        .as_u64()
        .ok_or("missing generation on created object")?;
    assert_eq!(
        generation_val, 1,
        "generation should start at 1 on create, got {generation_val}"
    );

    let rv = created["system"]["resourceVersion"]
        .as_u64()
        .ok_or("missing resourceVersion on created object")?;
    let created_at = created["system"]["createdAt"]
        .as_str()
        .ok_or("missing createdAt")?
        .to_string();
    let updated_at = created["system"]["updatedAt"]
        .as_str()
        .ok_or("missing updatedAt")?
        .to_string();

    // 2. Update with same spec but different labels — generation should NOT change
    let update_body = serde_json::json!({
        "key": { "group": "example.io", "version": "v1", "kind": "Widget" },
        "metadata": { "name": "gen-widget", "labels": { "env": "prod" } },
        "system": { "resourceVersion": rv, "createdAt": created_at, "updatedAt": updated_at },
        "spec": { "color": "blue", "size": 10 }
    });
    let resp = client
        .put("/apis/example.io/v1/Widget/gen-widget", update_body)
        .await;
    assert_status(&resp, StatusCode::OK);
    let updated: Value = parse_body(resp).await;
    let gen_after_labels = updated["system"]["generation"]
        .as_u64()
        .ok_or("missing generation after label update")?;
    assert_eq!(
        gen_after_labels, 1,
        "generation should stay at 1 after metadata-only update, got {gen_after_labels}"
    );

    let rv2 = updated["system"]["resourceVersion"]
        .as_u64()
        .ok_or("missing resourceVersion after label update")?;
    let updated_at2 = updated["system"]["updatedAt"]
        .as_str()
        .ok_or("missing updatedAt after label update")?
        .to_string();

    // 3. Update with different spec — generation should increment to 2
    let update_body = serde_json::json!({
        "key": { "group": "example.io", "version": "v1", "kind": "Widget" },
        "metadata": { "name": "gen-widget", "labels": { "env": "prod" } },
        "system": { "resourceVersion": rv2, "createdAt": created_at, "updatedAt": updated_at2 },
        "spec": { "color": "red", "size": 20 }
    });
    let resp = client
        .put("/apis/example.io/v1/Widget/gen-widget", update_body)
        .await;
    assert_status(&resp, StatusCode::OK);
    let updated: Value = parse_body(resp).await;
    let gen_after_spec = updated["system"]["generation"]
        .as_u64()
        .ok_or("missing generation after spec update")?;
    assert_eq!(
        gen_after_spec, 2,
        "generation should bump to 2 after spec change, got {gen_after_spec}"
    );

    let rv3 = updated["system"]["resourceVersion"]
        .as_u64()
        .ok_or("missing resourceVersion after spec update")?;

    // 4. Update status — generation should NOT change (still 2)
    let resp = client
        .put(
            "/apis/example.io/v1/Widget/gen-widget/status",
            serde_json::json!({
                "status": { "phase": "Running" }
            }),
        )
        .await;
    if resp.status() != StatusCode::OK {
        let body: Value = parse_body(resp).await;
        return Err(format!(
            "status update failed: expected 200, got error body: {}",
            serde_json::to_string_pretty(&body).unwrap_or_default()
        ));
    }
    let updated: Value = parse_body(resp).await;
    let gen_after_status = updated["system"]["generation"]
        .as_u64()
        .ok_or("missing generation after status update")?;
    assert_eq!(
        gen_after_status, 2,
        "generation should stay at 2 after status update, got {gen_after_status}"
    );

    let rv4 = updated["system"]["resourceVersion"]
        .as_u64()
        .ok_or("missing resourceVersion after status update")?;
    let updated_at4 = updated["system"]["updatedAt"]
        .as_str()
        .ok_or("missing updatedAt after status update")?
        .to_string();

    // 5. Update with same spec but different labels again — generation should NOT change (still 2)
    let update_body = serde_json::json!({
        "key": { "group": "example.io", "version": "v1", "kind": "Widget" },
        "metadata": { "name": "gen-widget", "labels": { "app": "httpd", "env": "staging" } },
        "system": { "resourceVersion": rv4, "createdAt": created_at, "updatedAt": updated_at4 },
        "spec": { "color": "red", "size": 20 }
    });
    let resp = client
        .put("/apis/example.io/v1/Widget/gen-widget", update_body)
        .await;
    if resp.status() != StatusCode::OK {
        let status = resp.status();
        let body: Value = parse_body(resp).await;
        return Err(format!(
            "second label update failed: expected 200, got {}, body: {}",
            status,
            serde_json::to_string_pretty(&body).unwrap_or_default()
        ));
    }
    assert_status(&resp, StatusCode::OK);
    let updated: Value = parse_body(resp).await;
    let gen_final = updated["system"]["generation"]
        .as_u64()
        .ok_or("missing generation after second label update")?;
    assert_eq!(
        gen_final, 2,
        "generation should stay at 2 after second metadata-only update, got {gen_final}"
    );

    // Verify resourceVersion incremented on every update
    assert!(rv4 > rv3, "resourceVersion should bump after status update");
    assert!(
        updated["system"]["resourceVersion"]
            .as_u64()
            .ok_or("missing resourceVersion")?
            > rv4,
        "resourceVersion should bump after second label update"
    );

    Ok(())
}
