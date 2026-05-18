//! Binary entry point for the kapi server.
//!
//! Initializes tracing, parses configuration, and delegates
//! to [`kapi::run`] for application construction and serving.

use std::env;
use std::sync::Arc;

use kapi::event::EventBus;
use kapi::store::memory::InMemoryStore;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let config = kapi::AppConfig {
        port: env::var("PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(8080),
        store: Arc::new(InMemoryStore::new()),
        event_bus: Arc::new(EventBus::default()),
    };

    kapi::run(config).await
}
