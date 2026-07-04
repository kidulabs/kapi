use thiserror::Error;

/// Errors that can occur when using the kapi client.
#[derive(Error, Debug)]
pub enum ClientError {
    /// Network-level or HTTP transport error (timeout, connection refused, etc.).
    #[error("HTTP error: {0}")]
    HttpError(#[from] reqwest::Error),

    /// The server returned a non-success status code with a structured error body.
    #[error("API error: {status} - {code}: {message}")]
    ApiError { status: u16, code: String, message: String },

    /// JSON serialization or deserialization failure.
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    /// Error while reading a server-sent events (SSE) stream.
    #[error("Stream error: {0}")]
    StreamError(String),
}
