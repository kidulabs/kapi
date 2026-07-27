use base64::Engine;

use crate::error::AppError;
use crate::object::types::ContinueToken;

/// Decodes a base64-encoded continue token back to (namespace, name).
pub fn decode_continue_token(token: &ContinueToken) -> Result<(Option<String>, String), AppError> {
    let bytes = base64::engine::general_purpose::STANDARD.decode(&token.0).map_err(|e| {
        AppError::Internal(anyhow::anyhow!("invalid continue token: base64 decoding failed: {e}"))
    })?;
    let json: serde_json::Value = serde_json::from_slice(&bytes).map_err(|e| {
        AppError::Internal(anyhow::anyhow!("invalid continue token: JSON parsing failed: {e}"))
    })?;
    let namespace = json.get("namespace").and_then(|v| v.as_str()).map(|s| s.to_string());
    let name = json.get("name").and_then(|v| v.as_str()).ok_or_else(|| {
        AppError::Internal(anyhow::anyhow!("invalid continue token: missing 'name' field"))
    })?;
    Ok((namespace, name.to_string()))
}

/// Encodes (namespace, name) into a base64 continue token.
pub fn encode_continue_token(namespace: Option<&str>, name: &str) -> ContinueToken {
    let json = serde_json::json!({
        "namespace": namespace,
        "name": name
    });
    let encoded = base64::engine::general_purpose::STANDARD.encode(json.to_string());
    ContinueToken(encoded)
}
