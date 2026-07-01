//! Route composition for the kapi server.
//!
//! Defines the HTTP route structure under /apis/{group}/{version}
//! with path parameter extraction and middleware layers.

use std::sync::Arc;

use axum::Router;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use crate::object::handler;
use crate::object::schema_service::SchemaService;
use crate::object::service::ObjectService;

/// Application state shared across all handlers.
///
/// Contains the ObjectService and SchemaService, which together hold the store,
/// event bus, and validators. Wrapped in Arc for Clone (required by axum's State
/// extractor).
#[derive(Clone)]
pub struct AppState {
    object_service: Arc<ObjectService>,
    schema_service: Arc<SchemaService>,
}

impl AppState {
    /// Creates a new AppState wrapping ObjectService and SchemaService.
    pub fn new(object_service: Arc<ObjectService>, schema_service: Arc<SchemaService>) -> Self {
        Self { object_service, schema_service }
    }

    /// Returns a reference to the ObjectService.
    pub fn object_service(&self) -> &ObjectService {
        &self.object_service
    }

    /// Returns a reference to the SchemaService.
    pub fn schema_service(&self) -> &SchemaService {
        &self.schema_service
    }
}

/// Builds the router with all object CRUD routes.
///
/// Route structure:
/// - Cluster-scoped:
///   - GET/POST /apis/{group}/{version}/{kind} → list/create
///   - GET/PUT/DELETE /apis/{group}/{version}/{kind}/{name} → get/update/delete
///   - GET/PUT /apis/{group}/{version}/{kind}/{name}/status → get_status/update_status
/// - Namespace-scoped:
///   - GET/POST /apis/{group}/{version}/namespaces/{namespace}/{kind} → list/create (namespaced)
///   - GET/PUT/DELETE /apis/{group}/{version}/namespaces/{namespace}/{kind}/{name} → get/update/delete (namespaced)
///   - GET/PUT /apis/{group}/{version}/namespaces/{namespace}/{kind}/{name}/status → get_status/update_status (namespaced)
pub fn build_router(state: AppState) -> Router {
    Router::new()
        // === Cluster-scoped routes ===
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
        // Status subresource routes: get status (GET), update status (PUT)
        .route(
            "/apis/{group}/{version}/{kind}/{name}/status",
            axum::routing::get(handler::get_status).put(handler::update_status),
        )
        // === Namespace-scoped routes ===
        // Collection routes: list (GET) and create (POST)
        .route(
            "/apis/{group}/{version}/namespaces/{namespace}/{kind}",
            axum::routing::get(handler::list_namespaced).post(handler::create_namespaced),
        )
        // Named resource routes: get (GET), update (PUT), delete (DELETE)
        .route(
            "/apis/{group}/{version}/namespaces/{namespace}/{kind}/{name}",
            axum::routing::get(handler::get_namespaced)
                .put(handler::update_namespaced)
                .delete(handler::delete_namespaced),
        )
        // Status subresource routes: get status (GET), update status (PUT)
        .route(
            "/apis/{group}/{version}/namespaces/{namespace}/{kind}/{name}/status",
            axum::routing::get(handler::get_status_namespaced)
                .put(handler::update_status_namespaced),
        )
        // OpenAPI spec endpoint: dynamically generated on every request
        .route("/openapi", axum::routing::get(crate::openapi::get_openapi_handler))
        // Swagger UI: loads Swagger UI from CDN and fetches spec from /openapi
        .route("/swagger-ui", axum::routing::get(crate::openapi::get_swagger_ui_handler))
        .route("/swagger-ui/", axum::routing::get(crate::openapi::get_swagger_ui_handler))
        // Middleware layers: tracing, CORS (outermost for preflight interception)
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .with_state(state)
}
