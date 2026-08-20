use axum::Json;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde_json::json;

use crate::ApiError;

/// Application error type wrapping the shared [`ApiError`] wire contract plus
/// server-only variants.
///
/// - [`AppError::Api`] — structured domain errors serialized via
///   [`ApiError`]'s tagged serde representation.
/// - [`AppError::Internal`] — unexpected internal failures (logged, HTTP 500).
/// - [`AppError::InvalidSchema`] — broken schema registration payloads (HTTP 422).
/// - [`AppError::StoredSchemaCompilationFailed`] — a stored schema that fails
///   to compile (HTTP 500).
#[derive(thiserror::Error, Debug)]
pub enum AppError {
    #[error(transparent)]
    Api(ApiError),

    #[error(transparent)]
    Internal(#[from] anyhow::Error),

    #[error("invalid schema: {0}")]
    InvalidSchema(String),

    #[error("stored schema '{schema_name}' compilation failed: {reason}")]
    StoredSchemaCompilationFailed { schema_name: String, reason: String },
}

impl From<ApiError> for AppError {
    fn from(err: ApiError) -> Self {
        AppError::Api(err)
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        match self {
            AppError::Api(api_err) => {
                let status = StatusCode::from_u16(api_err.http_status()).unwrap();
                (status, Json(api_err)).into_response()
            }
            AppError::Internal(err) => {
                tracing::error!(error = %err, "internal error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({
                        "code": "Internal",
                        "details": { "message": "internal error" }
                    })),
                )
                    .into_response()
            }
            AppError::InvalidSchema(msg) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({
                    "code": "InvalidSchema",
                    "details": { "message": msg }
                })),
            )
                .into_response(),
            AppError::StoredSchemaCompilationFailed { schema_name, reason } => {
                tracing::error!(%schema_name, %reason, "stored schema compilation failed");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({
                        "code": "StoredSchemaCompilationFailed",
                        "details": { "schema_name": schema_name, "reason": reason }
                    })),
                )
                    .into_response()
            }
        }
    }
}
