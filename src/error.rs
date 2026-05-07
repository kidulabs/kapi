use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde_json::json;

use crate::schema::types::ValidationError;

#[derive(thiserror::Error, Debug)]
pub enum AppError {
    #[error("{what} '{identifier}' not found")]
    NotFound { what: String, identifier: String },

    #[error("conflict: expected version {expected}, actual version {actual}")]
    Conflict { expected: u64, actual: u64 },

    #[error("schema validation failed")]
    SchemaValidation(Vec<ValidationError>),

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
