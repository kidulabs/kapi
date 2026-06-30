pub mod bootstrap;
pub mod config;
pub mod error;
pub mod event;
pub mod middleware;
pub mod namespace;
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

use crate::bootstrap::bootstrap_builtins;
use crate::object::schema_service::SchemaService;
use crate::object::service::ObjectService;
use crate::routes::{AppState, build_router};
use crate::schema::SchemaValidator;
use crate::schema::meta_schema::compile_meta_schema;

/// Construct the full application [`Router`] from the given config.
///
/// Compiles the meta-schema, wires up the store, event bus, and
/// service layer, runs built-in bootstrap (Namespace schema +
/// "default" namespace), then composes all routes and middleware.
///
/// Bootstrap failure causes `create_app` to return an error,
/// preventing the server from starting in an inconsistent state.
///
/// This function is async because bootstrap requires async operations
/// (registering the Namespace schema, creating the default namespace).
/// Callers running in a sync context should use
/// `tokio::runtime::Handle::current().block_on(create_app(&config))`.
pub async fn create_app(config: &AppConfig) -> anyhow::Result<Router> {
    let meta_validator: Arc<dyn SchemaValidator> = Arc::new(compile_meta_schema()?);
    info!("Meta-schema compiled successfully");

    // SchemaService owns its own SchemaRegistry
    let schema_service = Arc::new(SchemaService::new(
        config.store.clone(),
        config.event_bus.clone(),
        meta_validator.clone(),
    ));

    // ObjectService gets its own SchemaRegistry (shared store, separate cache)
    let object_service = Arc::new(ObjectService::new(
        config.store.clone(),
        config.event_bus.clone(),
        crate::schema::SchemaRegistry::new(config.store.clone(), meta_validator),
    ));

    // Run built-in bootstrap (Namespace schema + "default" namespace).
    // Errors propagate to the caller — server startup fails fast.
    bootstrap_builtins(&schema_service, &config.store, &config.event_bus)
        .await
        .map_err(|e| anyhow::anyhow!("bootstrap failed: {e}"))?;

    let app_state = AppState::new(object_service, schema_service);
    let app: Router = build_router(app_state);

    Ok(app)
}

/// Run a full kapi server with the given config.
///
/// Calls [`create_app`] internally, binds to `0.0.0.0:{port}`,
/// and serves requests until the process is signalled to stop.
pub async fn run(config: AppConfig) -> anyhow::Result<()> {
    let app = create_app(&config).await?;

    let listener = TcpListener::bind(format!("0.0.0.0:{}", config.port)).await?;
    info!("Server listening on port {}", config.port);

    axum::serve(listener, app).await?;

    Ok(())
}
