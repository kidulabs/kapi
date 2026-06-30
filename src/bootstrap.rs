//! Server bootstrap — registers built-in resources and seeds required state.
//!
//! Called from [`crate::create_app`] after the store, event bus, and
//! service layers are constructed but before the router is built.
//!
//! Currently bootstraps:
//! 1. The built-in `Namespace` schema (cluster-scoped, kind=Namespace).
//! 2. The `"default"` Namespace object (idempotent — no-op if it already
//!    exists from a previous run).
//!
//! Both steps are required for the server to be in a usable state. Any
//! bootstrap failure propagates as an error from [`bootstrap_builtins`]
//! and prevents the server from starting.

use std::sync::Arc;

use tracing::info;

use crate::error::AppError;
use crate::event::EventPublisher;
use crate::namespace::{bootstrap_default_namespace, register_namespace_schema};
use crate::object::schema_service::SchemaService;
use crate::store::ObjectStore;

/// Runs the full built-in bootstrap sequence.
///
/// 1. Registers the built-in Namespace schema (cluster-scoped).
/// 2. Creates the `"default"` Namespace object if it does not exist.
///
/// Either step may fail. On failure, the error is returned unchanged so
/// callers can wrap it with context (e.g. converting to `anyhow::Error`).
pub async fn bootstrap_builtins(
    schema_service: &SchemaService,
    store: &Arc<dyn ObjectStore>,
    event_bus: &Arc<dyn EventPublisher>,
) -> Result<(), AppError> {
    info!("Bootstrapping built-in resources");
    register_namespace_schema(schema_service).await?;
    bootstrap_default_namespace(store, event_bus).await?;
    info!("Built-in resources bootstrapped");
    Ok(())
}
