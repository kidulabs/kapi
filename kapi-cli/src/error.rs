//! Error types for the kapi CLI.

use std::fmt;

use kapi_client::error::ClientError;
use kapi_core::ApiError;

/// Errors that can occur in the kapi CLI.
#[derive(Debug)]
pub enum CliError {
    /// Configuration loading error (missing file, parse error).
    ConfigError(String),
    /// Error resolving a kind string to a schema (ambiguous, not found).
    ResolutionError(String),
    /// Error from the underlying HTTP client.
    ClientError(ClientError),
    /// I/O error (file read/write).
    IoError(std::io::Error),
    /// Error parsing or formatting output data.
    FormatError(String),
    /// No schema registered for the requested kind.
    SchemaNotFound { kind: String },
    /// The requested object was not found (HTTP 404).
    NotFound {
        kind: String,
        name: String,
        /// `None` for cluster-scoped objects, `Some(ns)` for namespaced objects.
        namespace: Option<String>,
    },
    /// Conflict error (HTTP 409) — object was modified concurrently.
    Conflict { message: String },
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CliError::ConfigError(msg) => write!(f, "configuration error: {msg}"),
            CliError::ResolutionError(msg) => write!(f, "{msg}"),
            CliError::ClientError(err) => write!(f, "{err}"),
            CliError::IoError(err) => write!(f, "I/O error: {err}"),
            CliError::FormatError(msg) => write!(f, "{msg}"),
            CliError::SchemaNotFound { kind } => write!(
                f,
                "No schema found for kind '{kind}'. Use 'kapi get Schema' to list available kinds"
            ),
            CliError::NotFound { kind, name, namespace } => match namespace {
                Some(ns) => write!(f, "{kind} '{name}' not found in namespace '{ns}'"),
                None => write!(f, "{kind} '{name}' not found"),
            },
            CliError::Conflict { message } => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for CliError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            CliError::ClientError(err) => Some(err),
            CliError::IoError(err) => Some(err),
            _ => None,
        }
    }
}

impl CliError {
    /// Converts a `ClientError` HTTP 404 into a structured [`CliError::NotFound`].
    ///
    /// Non-404 errors are passed through as [`CliError::ClientError`].
    pub fn from_not_found(
        err: ClientError,
        kind: &str,
        name: &str,
        namespace: Option<&str>,
    ) -> Self {
        match &err {
            ClientError::Api(ApiError::NotFound { .. }) => CliError::NotFound {
                kind: kind.to_string(),
                name: name.to_string(),
                namespace: namespace.map(|s| s.to_string()),
            },
            _ => CliError::ClientError(err),
        }
    }
}

impl From<ClientError> for CliError {
    fn from(err: ClientError) -> Self {
        match &err {
            ClientError::Api(ApiError::ObjectBeingDeleted { .. }) => CliError::Conflict {
                message: "object is being deleted; only finalizer modifications are allowed"
                    .to_string(),
            },
            ClientError::Api(ApiError::Conflict { .. }) => CliError::Conflict {
                message: "conflict: object was modified, retry manually".to_string(),
            },
            ClientError::Api(
                api_err @ (ApiError::AlreadyExists { .. }
                | ApiError::SchemaHasObjects { .. }
                | ApiError::NamespaceNotEmpty { .. }),
            ) => CliError::Conflict { message: api_err.to_string() },
            _ => CliError::ClientError(err),
        }
    }
}

impl From<std::io::Error> for CliError {
    fn from(err: std::io::Error) -> Self {
        CliError::IoError(err)
    }
}

impl From<serde_json::Error> for CliError {
    fn from(err: serde_json::Error) -> Self {
        CliError::FormatError(err.to_string())
    }
}

impl From<serde_yaml::Error> for CliError {
    fn from(err: serde_yaml::Error) -> Self {
        CliError::FormatError(err.to_string())
    }
}

impl From<ApiError> for CliError {
    fn from(err: ApiError) -> Self {
        CliError::FormatError(err.to_string())
    }
}
