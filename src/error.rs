use axum::Json;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde_json::json;

use crate::object::types::ValidationError;

#[derive(thiserror::Error, Debug)]
pub enum AppError {
    #[error("{what} '{identifier}' not found")]
    NotFound { what: String, identifier: String },

    #[error("{kind} '{name}' already exists")]
    AlreadyExists { kind: String, name: String },

    #[error("conflict: expected version {expected}, actual version {actual}")]
    Conflict { expected: u64, actual: u64 },

    #[error("schema validation failed")]
    SchemaValidation(Vec<ValidationError>),

    // fieldSelector query parameter parsing failed (unsupported field, malformed syntax)
    // Maps to HTTP 400 Bad Request in into_response
    #[error("invalid field selector: {0}")]
    InvalidFieldSelector(String),

    // Label validation failure (key/value format, length limits)
    // Maps to HTTP 400 Bad Request in into_response
    #[error("invalid label: {0}")]
    InvalidLabel(String),

    // labelSelector query parameter parsing failed (malformed syntax)
    // Maps to HTTP 400 Bad Request in into_response
    #[error("invalid label selector: {0}")]
    InvalidLabelSelector(String),

    // The schema itself is broken (meta-schema validation or compilation failure)
    #[error("invalid schema: {0}")]
    InvalidSchema(String),

    // Attempting to delete a Schema that has existing objects of the target kind
    #[error("schema has objects: kind={kind}, count={count}")]
    SchemaHasObjects { kind: String, count: usize },

    #[error("stored schema '{schema_name}' compilation failed: {reason}")]
    StoredSchemaCompilationFailed { schema_name: String, reason: String },

    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        let (status, code, error, details) = match self {
            AppError::NotFound { what, identifier } => (
                StatusCode::NOT_FOUND,
                "NotFound",
                format!("{what} '{identifier}' not found"),
                json!({ "what": what, "identifier": identifier }),
            ),
            AppError::AlreadyExists { kind, name } => (
                StatusCode::CONFLICT,
                "AlreadyExists",
                format!("{kind} '{name}' already exists"),
                json!({ "kind": kind, "name": name }),
            ),
            AppError::Conflict { expected, actual } => (
                StatusCode::CONFLICT,
                "Conflict",
                format!("conflict: expected version {expected}, actual version {actual}"),
                json!({ "expected": expected, "actual": actual }),
            ),
            AppError::SchemaValidation(errors) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "SchemaValidation",
                "schema validation failed".to_string(),
                json!({ "errors": errors }),
            ),
            // InvalidFieldSelector maps to HTTP 400 with the error message
            AppError::InvalidFieldSelector(msg) => (
                StatusCode::BAD_REQUEST,
                "InvalidFieldSelector",
                format!("invalid field selector: {msg}"),
                json!({ "message": msg }),
            ),
            // InvalidLabel maps to HTTP 400 with the error message
            AppError::InvalidLabel(msg) => (
                StatusCode::BAD_REQUEST,
                "InvalidLabel",
                format!("invalid label: {msg}"),
                json!({ "message": msg }),
            ),
            // InvalidLabelSelector maps to HTTP 400 with the error message
            AppError::InvalidLabelSelector(msg) => (
                StatusCode::BAD_REQUEST,
                "InvalidLabelSelector",
                format!("invalid label selector: {msg}"),
                json!({ "message": msg }),
            ),
            // InvalidSchema maps to HTTP 422 Unprocessable Entity
            AppError::InvalidSchema(msg) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "InvalidSchema",
                format!("invalid schema: {msg}"),
                json!({ "message": msg }),
            ),
            // StoredSchemaCompilationFailed maps to HTTP 500 Internal Server Error
            AppError::StoredSchemaCompilationFailed {
                schema_name,
                reason,
            } => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "StoredSchemaCompilationFailed",
                format!("stored schema '{schema_name}' compilation failed: {reason}"),
                json!({ "schemaName": schema_name, "reason": reason }),
            ),
            // SchemaHasObjects maps to HTTP 409 Conflict
            AppError::SchemaHasObjects { kind, count } => (
                StatusCode::CONFLICT,
                "SchemaHasObjects",
                format!("schema has objects: kind={kind}, count={count}"),
                json!({ "kind": kind, "count": count }),
            ),
            AppError::Internal(_err) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal",
                "internal error".to_string(),
                json!(null),
            ),
        };

        let body = json!({
            "error": error,
            "code": code,
            "details": details,
        });

        (status, Json(body)).into_response()
    }
}
