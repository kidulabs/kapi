use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde_json::json;

use crate::object::types::ValidationError;

#[derive(thiserror::Error, Debug)]
pub enum AppError {
    #[error("{what} '{identifier}' not found")]
    NotFound { what: String, identifier: String },

    #[error("conflict: expected version {expected}, actual version {actual}")]
    Conflict { expected: u64, actual: u64 },

    #[error("schema validation failed")]
    SchemaValidation(Vec<ValidationError>),

    // The schema itself is broken (meta-schema validation or compilation failure)
    #[error("invalid schema: {0}")]
    InvalidSchema(String),

    // Attempting to delete a Schema that has existing objects of the target kind
    #[error("schema has objects: kind={kind}, count={count}")]
    SchemaHasObjects { kind: String, count: usize },

    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        let (status, code, error, details) = match self {
            AppError::NotFound { what, identifier } => {
                (StatusCode::NOT_FOUND, "NotFound", format!("{what} '{identifier}' not found"), json!({ "what": what, "identifier": identifier }))
            }
            AppError::Conflict { expected, actual } => {
                (StatusCode::CONFLICT, "Conflict", format!("conflict: expected version {expected}, actual version {actual}"), json!({ "expected": expected, "actual": actual }))
            }
            AppError::SchemaValidation(errors) => {
                (StatusCode::UNPROCESSABLE_ENTITY, "SchemaValidation", "schema validation failed".to_string(), json!({ "errors": errors }))
            }
            // InvalidSchema maps to HTTP 422 Unprocessable Entity
            AppError::InvalidSchema(msg) => {
                (StatusCode::UNPROCESSABLE_ENTITY, "InvalidSchema", format!("invalid schema: {msg}"), json!({ "message": msg }))
            }
            // SchemaHasObjects maps to HTTP 409 Conflict
            AppError::SchemaHasObjects { kind, count } => {
                (StatusCode::CONFLICT, "SchemaHasObjects", format!("schema has objects: kind={kind}, count={count}"), json!({ "kind": kind, "count": count }))
            }
            AppError::Internal(_err) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal", "internal error".to_string(), json!(null))
            }
        };

        let body = json!({
            "error": error,
            "code": code,
            "details": details,
        });

        (status, Json(body)).into_response()
    }
}
