//! Route composition for the kapi server.
//!
//! Defines the HTTP route structure under /apis/{group}/{version}
//! with path parameter extraction and middleware layers.

use std::sync::Arc;

use axum::Router;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use crate::object::handler;
use crate::object::service::ObjectService;

/// Application state shared across all handlers.
///
/// Contains the ObjectService which holds the store, event bus, and validators.
/// Wrapped in Arc for Clone (required by axum's State extractor).
#[derive(Clone)]
pub struct AppState {
    object_service: Arc<ObjectService>,
}

impl AppState {
    /// Creates a new AppState wrapping an ObjectService.
    pub fn new(object_service: Arc<ObjectService>) -> Self {
        Self { object_service }
    }

    /// Returns a reference to the ObjectService.
    pub fn object_service(&self) -> &ObjectService {
        &self.object_service
    }
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
            axum::routing::get(handler::get)
                .put(handler::update)
                .delete(handler::delete),
        )
        // Status subresource routes: get status (GET), update status (PUT)
        .route(
            "/apis/{group}/{version}/{kind}/{name}/status",
            axum::routing::get(handler::get_status)
                .put(handler::update_status),
        )
        // OpenAPI spec endpoint: dynamically generated on every request
        .route(
            "/openapi",
            axum::routing::get(crate::openapi::get_openapi_handler),
        )
        // Swagger UI: loads Swagger UI from CDN and fetches spec from /openapi
        .route(
            "/swagger-ui",
            axum::routing::get(crate::openapi::get_swagger_ui_handler),
        )
        .route(
            "/swagger-ui/",
            axum::routing::get(crate::openapi::get_swagger_ui_handler),
        )
        // Middleware layers: tracing, CORS (outermost for preflight interception)
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .with_state(state)
}
