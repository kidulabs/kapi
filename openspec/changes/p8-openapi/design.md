## Context

kapi's API is dynamic: users register JSON Schemas for custom kinds at runtime, and the server exposes CRUD endpoints under `/apis/{group}/{version}/{kind}`. The current codebase has `utoipa` and `utoipa-swagger-ui` as dependencies, but `src/openapi.rs` contains only a `// TODO` comment. The original roadmap (T52–T55) assumed compile-time `#[derive(ToSchema)]` on types, which is incompatible with dynamic kind registration.

The `ObjectService` stores Schema objects in the in-memory store (`InMemoryStore` via `DashMap`). The `schema_cache` holds compiled `jsonschema::Validator` instances but cannot reconstruct the original JSON Schema. To generate an OpenAPI spec, we must query the store for Schema objects and extract their `data.jsonSchema` fields.

## Goals / Non-Goals

**Goals:**
- Generate valid OpenAPI 3.0.3 JSON on every `GET /openapi` request
- Include static components for kapi's built-in types (StoredObject, ObjectMetadata, AppError, etc.)
- Include static paths for Schema CRUD (`/apis/kapi.io/v1/Schema`)
- Dynamically generate per-kind paths and component schemas from registered Schema objects
- Component names follow the pattern: `"Widget.other.io"` → `"WidgetOtherIo"`
- Optionally serve Swagger UI HTML pointing to `/openapi`

**Non-Goals:**
- No spec caching (build on every request; optimization deferred to P-Future)
- No compile-time `utoipa` derives for this capability
- No modification to existing types, services, or store implementations
- Swagger UI is optional — skip if non-trivial

## Decisions

### Decision 1: Skip utoipa derives, build spec as raw JSON

**Choice:** Build the OpenAPI spec as `serde_json::Value` at request time, without using `utoipa` derive macros.

**Rationale:** `utoipa` generates specs at compile time via `#[derive(ToSchema)]` and `#[openapi(paths(...))]`. kapi's kinds are registered at runtime, making compile-time generation impossible for the dynamic portion. Using `utoipa` for static parts and manual JSON for dynamic parts would create two parallel spec-building systems. A single approach — building the entire spec as JSON — is simpler and more maintainable.

**Alternatives considered:**
- Use `utoipa` for static types, merge with dynamic paths at runtime → adds complexity, partial benefit
- Define typed OpenAPI structs and serialize → more code, no real advantage over `serde_json::Value`

### Decision 2: Build spec on every request, no caching

**Choice:** Query the store and build the full spec on each `GET /openapi` request.

**Rationale:** `/openapi` is not a hot path — it's called rarely (tool integration, Swagger UI load). The work is entirely in-memory: iterating a `DashMap`, deserializing JSON, building a JSON object. Even with hundreds of schemas, this takes microseconds. Caching would require invalidation hooks on Schema create/update/delete, adding complexity for negligible benefit.

**Alternatives considered:**
- Cache in `Arc<RwLock<Value>>`, invalidate on Schema mutation → deferred to P-Future

### Decision 3: Schema discovery via `service.list(Schema)`

**Choice:** The openapi module discovers registered schemas by calling `service.list(schema_key, ListOptions { limit: None })`.

**Rationale:** The `ObjectService` already has a `list` method. Schema objects are stored with `kind="Schema"` in group `"kapi.io"`. No new methods are needed. The `schema_cache` cannot be used because it holds compiled validators, not the original `jsonSchema` JSON.

### Decision 4: Component naming convention

**Choice:** Split the schema name on dots, PascalCase each segment, concatenate. `"Widget.other.io"` → `"WidgetOtherIo"`.

**Rationale:** This is collision-free (includes the group), readable, and consistent with the existing schema naming convention. The alternative of using kind-only names (`"WidgetData"`) would collide if two groups register the same kind name.

### Decision 5: Per-kind component schema structure

**Choice:** For each registered kind, generate three component schemas:
- `{Kind}{Group}` — the user's `jsonSchema` (used for POST/PUT request body `data` field)
- `{Kind}{Group}StoredObject` — envelope with `key`, `metadata`, `data` (where `data` refs the kind's schema)
- `{Kind}{Group}ListResponse` — list envelope with `items` array of `{Kind}{Group}StoredObject`

**Rationale:** This mirrors the actual wire format. The `StoredObject` envelope is always the response shape; the user's schema fills the `data` field. Separate components avoid duplication and enable `$ref` reuse.

### Decision 6: Swagger UI via custom HTML

**Choice:** If implemented, serve a simple HTML page with Swagger UI from CDN, configured with `url: "/openapi"`.

**Rationale:** `utoipa-swagger-ui` embeds the spec at compile time, which doesn't work for dynamic specs. A custom HTML page is ~10 lines and points to the live `/openapi` endpoint.

## Risks / Trade-offs

| Risk | Mitigation |
|------|-----------|
| Spec generation cost grows with number of schemas | Defer caching to P-Future; current approach is fine for expected scale |
| Invalid user `jsonSchema` could produce invalid OpenAPI | User schemas are already validated against meta-schema and compiled; wrap in OpenAPI `object` type safely |
| OpenAPI spec may drift from actual API if handlers change | Spec is generated from live store data; paths are hardcoded to match route definitions |
| Swagger UI CDN dependency | Optional feature; if included, CDN URL can be pinned to a specific version |
