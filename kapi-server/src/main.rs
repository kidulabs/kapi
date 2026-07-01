//! Binary entry point for the kapi server.
//!
//! Initializes tracing, parses configuration, and delegates
//! to [`kapi_server::run`] for application construction and serving.

use std::env;
use std::sync::Arc;

use kapi_server::event::EventBus;
use kapi_server::store::sqlite::SQLiteStore;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let db_path = env::var("KAPI_DB_PATH").unwrap_or_else(|_| "./kapi.db".to_string());
    let store = Arc::new(SQLiteStore::new(&db_path)?);

    let config = kapi_server::AppConfig {
        port: env::var("PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(8080),
        store,
        event_bus: Arc::new(EventBus::default()),
    };

    kapi_server::run(config).await
}
