use axum::http::StatusCode;
use serde_json::json;

use crate::{TestApp, assert_status};

pub async fn test_valid_schema_accepted(app: &TestApp) -> Result<(), String> {
    let client = app.client();

    let valid_schema = json!({
        "targetGroup": "valid.io",
        "targetVersion": "v1",
        "targetKind": "ValidThing",
        "specSchema": {
            "type": "object",
            "properties": {
                "name": { "type": "string" }
            }
        }
    });

    let resp = client.post("/apis/kapi.io/v1/Schema", valid_schema).await;
    assert_status(&resp, StatusCode::CREATED);

    Ok(())
}

pub async fn test_invalid_spec_schema_rejected(app: &TestApp) -> Result<(), String> {
    let client = app.client();

    let invalid_schema = json!({
        "targetGroup": "bad.io",
        "targetVersion": "v1",
        "targetKind": "BadThing",
        "specSchema": {
            "type": "not-a-real-type"
        }
    });

    let resp = client.post("/apis/kapi.io/v1/Schema", invalid_schema).await;
    assert_status(&resp, StatusCode::UNPROCESSABLE_ENTITY);

    Ok(())
}

pub async fn test_missing_required_fields_rejected(app: &TestApp) -> Result<(), String> {
    let client = app.client();

    let missing_target_kind = json!({
        "targetGroup": "missing.io",
        "targetVersion": "v1",
        "specSchema": {
            "type": "object"
        }
    });

    let resp = client.post("/apis/kapi.io/v1/Schema", missing_target_kind).await;
    assert_status(&resp, StatusCode::UNPROCESSABLE_ENTITY);

    Ok(())
}
