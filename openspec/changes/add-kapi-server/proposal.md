## Why

We need an extensible API server ("kapi") that can register JSON schemas at runtime and serve CRUD operations for objects validated against those schemas. This enables a Kubernetes-like API model where users define their own resource types and interact with them through a uniform REST interface, with real-time change notification via event streaming.

## What Changes

- Introduce a new Rust API server built on Axum with Tower middleware
- Schema registry: register JSON Schema definitions for custom object kinds
- Object CRUD: create, read, update, delete objects validated against registered schemas
- Kube-native API paths: `/apis/{group}/{version}/{kind}` and `/apis/{group}/{version}/{kind}/{name}`
- Optimistic concurrency using monotonic `resourceVersion` on updates and deletes
- Watch support via `?watch=true` query parameter with Server-Sent Events (SSE)
- OpenAPI schema generation and `/openapi.json` endpoint with Swagger UI
- Pluggable storage abstraction: `SchemaStore` + `ObjectStore` traits
- In-memory storage engine as default for development
- Modular service layer that orchestrates store mutations and event publishing
- Tower middleware stubs for authentication, metrics, and tracing
- Per-resource-kind event bus for real-time streaming

## Capabilities

### New Capabilities
- `kapi-server`: Core API server with schema registry, object CRUD, admission validation, event streaming, and OpenAPI support
- `pluggable-storage`: Storage abstraction layer with `SchemaStore` and `ObjectStore` async traits, in-memory v1 implementation

### Modified Capabilities
- None

## Impact

- New Rust project structure under `src/` with modular domain separation
- New dependencies: axum, tokio, dashmap, jsonschema, utoipa, tower, chrono, async-trait, thiserror
- No breaking changes to existing code (new project)
