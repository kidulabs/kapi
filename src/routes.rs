//! Route composition for the kapi server.
//!
//! Defines the HTTP route structure under /apis/{group}/{version}
//! with path parameter extraction and middleware layers.

use std::sync::Arc;

use axum::Router;
use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;

use crate::object::handler;
use crate::object::service::ObjectService;
use crate::store::memory::InMemoryStore;

/// Application state shared across all handlers.
///
/// Contains the ObjectService which holds the store, event bus, and validators.
/// Wrapped in Arc for Clone (required by axum's State extractor).
#[derive(Clone)]
pub struct AppState {
    pub object_service: Arc<ObjectService<InMemoryStore>>,
}

/// Builds the router with all object CRUD routes.
///
/// Route structure:
/// - GET/POST /apis/{group}/{version}/{kind} → list/create
/// - GET/PUT/DELETE /apis/{group}/{version}/{kind}/{name} → get/update/delete
pub fn build_router(state: AppState) -> Router {
    Router::new()
        // Collection routes: list (GET) and create (POST)
        .route(
            "/apis/{group}/{version}/{kind}",
            axum::routing::get(handler::list).post(handler::create),
        )
        // Named resource routes: get (GET), update (PUT), delete (DELETE)
        .route(
            "/apis/{group}/{version}/{kind}/{name}",
            axum::routing::get(handler::get).put(handler::update).delete(handler::delete),
        )
        // Middleware layers: tracing
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http()),
        )
        .with_state(state)
}
