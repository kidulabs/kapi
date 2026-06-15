pub mod config;
pub mod error;
pub mod event;
pub mod middleware;
pub mod object;
pub mod openapi;
pub mod routes;
pub mod schema;
pub mod store;
pub mod validation;

pub use config::AppConfig;
pub use event::EventPublisher;
pub use object::types::{ContinueToken, ListOptions, ListResponse};
pub use store::ObjectStore;

use std::sync::Arc;

use axum::Router;
use tokio::net::TcpListener;
use tracing::info;

use crate::object::service::ObjectService;
use crate::routes::{AppState, build_router};
use crate::schema::SchemaValidator;
use crate::schema::meta_schema::compile_meta_schema;

/// Construct the full application [`Router`] from the given config.
///
/// Compiles the meta-schema, wires up the store, event bus, and
/// service layer, then composes all routes and middleware.
pub fn create_app(config: &AppConfig) -> anyhow::Result<Router> {
    let meta_validator: Arc<dyn SchemaValidator> = Arc::new(compile_meta_schema()?);
    info!("Meta-schema compiled successfully");

    let object_service = Arc::new(ObjectService::new(
        config.store.clone(),
        config.event_bus.clone(),
        meta_validator,
    ));

    let app_state = AppState::new(object_service);
    let app: Router = build_router(app_state);

    Ok(app)
}

/// Run a full kapi server with the given config.
///
/// Calls [`create_app`] internally, binds to `0.0.0.0:{port}`,
/// and serves requests until the process is signalled to stop.
pub async fn run(config: AppConfig) -> anyhow::Result<()> {
    let app = create_app(&config)?;

    let listener = TcpListener::bind(format!("0.0.0.0:{}", config.port)).await?;
    info!("Server listening on port {}", config.port);

    axum::serve(listener, app).await?;

    Ok(())
}
