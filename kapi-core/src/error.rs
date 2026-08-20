use serde::{Deserialize, Serialize};

/// Shared API error enum representing the wire contract between server and client.
///
/// This enum is serialized using tagged serde format:
/// `{"code": "VariantName", "details": { ... }}`
///
/// The `#[non_exhaustive]` attribute allows future variants to be added without
/// breaking existing match statements (they must include a wildcard arm).
#[derive(Debug, Clone, Serialize, Deserialize, thiserror::Error, PartialEq, Eq)]
#[serde(tag = "code", content = "details", rename_all = "PascalCase")]
#[non_exhaustive]
pub enum ApiError {
    /// Resource not found (HTTP 404)
    #[error("{what} '{identifier}' not found")]
    NotFound { what: String, identifier: String },

    /// Resource already exists (HTTP 409)
    #[error("{kind} '{name}' already exists")]
    AlreadyExists { kind: String, name: String },

    /// Optimistic concurrency conflict (HTTP 409)
    #[error("conflict: expected resource version {expected}, got {actual}")]
    Conflict { expected: u64, actual: u64 },

    /// Object is being deleted and cannot be modified (HTTP 409)
    #[error("object '{name}' is being deleted")]
    ObjectBeingDeleted { name: String },

    /// Namespace is protected and cannot be deleted (HTTP 403)
    #[error("namespace '{name}' is protected")]
    ProtectedNamespace { name: String },

    /// Namespace contains objects and cannot be deleted (HTTP 409)
    #[error("namespace '{namespace}' contains {object_count} objects")]
    NamespaceNotEmpty { namespace: String, object_count: usize },

    /// Schema validation failed (HTTP 422)
    #[error("schema validation failed")]
    SchemaValidation { errors: Vec<String> },

    /// Invalid schema definition (HTTP 422)
    #[error("invalid schema: {message}")]
    InvalidSchema { message: String },

    /// Schema has objects and cannot be deleted (HTTP 409)
    #[error("schema '{kind}' has objects")]
    SchemaHasObjects { kind: String },

    /// Status subresource is not enabled for this kind (HTTP 404)
    #[error("status subresource not enabled for kind '{kind}'")]
    StatusSubresourceNotEnabled { kind: String },

    /// Generic invalid request (HTTP 400)
    #[error("invalid {what}: {message}")]
    InvalidRequest { what: String, message: String },

    /// Stored schema compilation failed (HTTP 500)
    #[error("stored schema compilation failed for '{schema_name}': {reason}")]
    StoredSchemaCompilationFailed { schema_name: String, reason: String },

    /// Internal server error (HTTP 500)
    #[error("internal error: {message}")]
    Internal { message: String },

    /// Unknown error code from a newer server version
    #[error("{message}")]
    Unknown { code: String, message: String, details: serde_json::Value },
}

impl ApiError {
    /// Map each variant to its corresponding HTTP status code.
    pub fn http_status(&self) -> u16 {
        match self {
            // 404 Not Found
            ApiError::NotFound { .. } | ApiError::StatusSubresourceNotEnabled { .. } => 404,

            // 400 Bad Request
            ApiError::InvalidRequest { .. } => 400,

            // 403 Forbidden
            ApiError::ProtectedNamespace { .. } => 403,

            // 409 Conflict
            ApiError::AlreadyExists { .. }
            | ApiError::Conflict { .. }
            | ApiError::ObjectBeingDeleted { .. }
            | ApiError::NamespaceNotEmpty { .. }
            | ApiError::SchemaHasObjects { .. } => 409,

            // 422 Unprocessable Entity
            ApiError::SchemaValidation { .. } | ApiError::InvalidSchema { .. } => 422,

            // 500 Internal Server Error
            ApiError::StoredSchemaCompilationFailed { .. } | ApiError::Internal { .. } => 500,

            // Unknown errors default to 500
            ApiError::Unknown { .. } => 500,
        }
    }
}
