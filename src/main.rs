mod error;
mod event;
mod object;
mod openapi;
mod routes;
mod schema;
mod store;

use std::env;
use std::sync::Arc;

use axum::Router;
use tokio::net::TcpListener;
use tracing::info;

use crate::event::EventPublisher;
use crate::object::service::ObjectService;
use crate::routes::{build_router, AppState};
use crate::schema::meta_schema::compile_meta_schema;
use crate::schema::SchemaValidator;
use crate::store::memory::InMemoryStore;
use crate::store::ObjectStore;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing subscriber for structured logging
    tracing_subscriber::fmt::init();

    // Compile meta-schema at startup — used to validate Schema registrations
    let meta_validator: Arc<dyn SchemaValidator> = Arc::new(compile_meta_schema()?);
    info!("Meta-schema compiled successfully");

    // Construct storage backend
    let store: Arc<dyn ObjectStore> = Arc::new(InMemoryStore::new());

    // Construct event bus for SSE watch notifications (as trait object)
    let event_bus: Arc<dyn EventPublisher> = Arc::new(crate::event::EventBus::default());

    // Construct ObjectService with store, event bus, and meta-validator
    let object_service = ObjectService::new(store, event_bus, meta_validator);

    // Build application state — via constructor for encapsulation
    let app_state = AppState::new(Arc::new(object_service));

    // Build router with all routes and middleware
    let app: Router = build_router(app_state);

    // Bind to port from PORT env var or default 8080
    let port = env::var("PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(8080);

    let listener = TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
    info!("Server listening on port {}", port);

    axum::serve(listener, app).await?;

    Ok(())
}
