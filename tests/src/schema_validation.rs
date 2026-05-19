use axum::http::StatusCode;
use serde_json::json;

use crate::{assert_status, TestApp};

pub async fn test_valid_schema_accepted() -> Result<(), String> {
    let app = TestApp::new();
    let client = app.client();

    let valid_schema = json!({
        "targetGroup": "valid.io",
        "targetVersion": "v1",
        "targetKind": "ValidThing",
        "jsonSchema": {
            "type": "object",
            "properties": {
                "name": { "type": "string" }
            }
        }
    });

    let resp = client
        .post("/apis/kapi.io/v1/Schema", valid_schema)
        .await;
    assert_status(&resp, StatusCode::CREATED);

    Ok(())
}

pub async fn test_invalid_json_schema_rejected() -> Result<(), String> {
    let app = TestApp::new();
    let client = app.client();

    let invalid_schema = json!({
        "targetGroup": "bad.io",
        "targetVersion": "v1",
        "targetKind": "BadThing",
        "jsonSchema": {
            "type": "not-a-real-type"
        }
    });

    let resp = client
        .post("/apis/kapi.io/v1/Schema", invalid_schema)
        .await;
    assert_status(&resp, StatusCode::UNPROCESSABLE_ENTITY);

    Ok(())
}

pub async fn test_missing_required_fields_rejected() -> Result<(), String> {
    let app = TestApp::new();
    let client = app.client();

    let missing_target_kind = json!({
        "targetGroup": "missing.io",
        "targetVersion": "v1",
        "jsonSchema": {
            "type": "object"
        }
    });

    let resp = client
        .post("/apis/kapi.io/v1/Schema", missing_target_kind)
        .await;
    assert_status(&resp, StatusCode::UNPROCESSABLE_ENTITY);

    Ok(())
}
