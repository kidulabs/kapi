use std::sync::Arc;

use crate::event::EventPublisher;
use crate::store::ObjectStore;

/// Configuration for running a kapi server.
///
/// All fields are required — the library will fail at compile
/// time if any are missing. Pass this struct to [`create_app`]
/// or [`run`] to start the server.
pub struct AppConfig {
    /// TCP port the server will bind to.
    pub port: u16,
    /// Pluggable storage backend (e.g. `InMemoryStore`).
    pub store: Arc<dyn ObjectStore>,
    /// Pluggable event bus for SSE watch notifications.
    pub event_bus: Arc<dyn EventPublisher>,
}
