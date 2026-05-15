## Why

The project needs a machine-readable OpenAPI specification so that clients can discover available endpoints, understand request/response shapes, and generate client code. Since kapi's API is dynamic — users register JSON Schemas for custom kinds at runtime — the OpenAPI spec cannot be generated at compile time. It must be built on demand from the current set of registered schemas.

## What Changes

- Add `GET /openapi` endpoint that returns a dynamically generated OpenAPI 3.0.3 JSON document
- The spec includes static components (StoredObject, ObjectMetadata, AppError, etc.) and static paths (Schema CRUD)
- For each registered Schema, the spec dynamically generates per-kind paths and component schemas based on the user's `jsonSchema`
- Component names use the pattern `"Widget.other.io"` → `"WidgetOtherIo"` (split on dots, PascalCase each segment, concatenate)
- The spec is built from scratch on every request by listing Schema objects from the store
- Optional: add `GET /swagger-ui/` endpoint serving an HTML page with Swagger UI CDN pointing to `/openapi`
- Replace original P8 tasks (T52–T55) which assumed compile-time `utoipa` derives with new tasks reflecting the dynamic approach
- Add a future optimization item: cache the generated spec and invalidate on Schema mutations

## Capabilities

### New Capabilities

- `openapi-spec`: Dynamic OpenAPI 3.0.3 specification generation at request time, including static components/paths for Schema CRUD and dynamic per-kind paths/schemas from registered schemas

### Modified Capabilities

- *(none — this is a new capability, no existing spec requirements change)*

## Impact

- New module: `src/openapi.rs` — OpenAPI spec builder and HTTP handlers
- Modified: `src/routes.rs` — add `/openapi` and optional `/swagger-ui/` routes
- Modified: `roadmap.md` — replace P8 tasks with new dynamic-generation tasks, add P-Future caching item
- No changes to existing types, services, or store implementations
- `utoipa` and `utoipa-swagger-ui` dependencies remain but are not used for spec generation (kept for potential future use)
