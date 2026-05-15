## 1. OpenAPI Spec Generator

- [ ] 1.1 Implement `build_openapi_spec(service: &ObjectService<InMemoryStore>) -> serde_json::Value` in `src/openapi.rs` that generates a complete OpenAPI 3.0.3 document
- [ ] 1.2 Add static component schemas: `ResourceKey`, `ObjectMetadata`, `UserData`, `StoredObject`, `ListResponse`, `WatchEvent`, `WatchEventType`, `ValidationError`, `AppError`, `SchemaData`
- [ ] 1.3 Add static paths for Schema CRUD: `GET/POST /apis/kapi.io/v1/Schema`, `GET/DELETE /apis/kapi.io/v1/Schema/{name}` with correct request/response schemas and error codes
- [ ] 1.4 Implement schema discovery: call `service.list(schema_key, ListOptions { limit: None })` to fetch all registered Schema objects
- [ ] 1.5 Implement component name generation: split schema name on dots, PascalCase each segment, concatenate (`"Widget.other.io"` → `"WidgetOtherIo"`)
- [ ] 1.6 For each registered Schema, generate dynamic component: `{Kind}{Group}` from user's `jsonSchema`, `{Kind}{Group}StoredObject` envelope, `{Kind}{Group}ListResponse`
- [ ] 1.7 For each registered Schema, generate dynamic paths: `GET/POST /apis/{group}/{version}/{kind}`, `GET/PUT/DELETE /apis/{group}/{version}/{kind}/{name}` with path parameters, query parameters (`?watch=true`), and response codes (201, 200, 404, 409, 422)

## 2. HTTP Handlers and Routes

- [ ] 2.1 Implement `GET /openapi` handler that calls `build_openapi_spec` and returns `Json<Value>` with `application/json` content type
- [ ] 2.2 Implement `GET /swagger-ui/` handler (optional) — serve HTML page with Swagger UI CDN configured to fetch from `/openapi`
- [ ] 2.3 Wire `/openapi` and `/swagger-ui/` routes in `src/routes.rs`

## 3. Roadmap Update

- [ ] 3.1 Audit current P8 tasks (T52–T55) against the new dynamic approach and document deviations
- [ ] 3.2 Replace P8 tasks in `roadmap.md` with new tasks reflecting dynamic spec generation (build on every request, component naming convention, etc.)
- [ ] 3.3 Add `P-Future — OpenAPI Spec Caching` section to roadmap with caching optimization task
- [ ] 3.4 Verify all other roadmap sections remain accurate after changes

## 4. Unit Tests

- [ ] 4.1 Test `component_name` function: `"Widget.example.io"` → `"WidgetExampleIo"`, `"Deployment.apps"` → `"DeploymentApps"`, same kind different groups produce different names
- [ ] 4.2 Test `build_static_components`: output contains all 10 required component names with correct JSON shapes (types, properties, required fields)
- [ ] 4.3 Test `build_kind_data_component`: given a user `jsonSchema`, output wraps it as an OpenAPI schema with correct `type: "object"` and properties
- [ ] 4.4 Test `build_kind_stored_object_component`: output has `key`, `metadata`, `data` properties with correct `$ref` pointers to the kind's data component
- [ ] 4.5 Test `build_kind_list_response_component`: output has `items` array of kind-specific StoredObject `$ref` and `continue_token` field
- [ ] 4.6 Test `build_static_paths`: output contains all 4 Schema CRUD paths with correct HTTP methods, path parameters, and response schemas
- [ ] 4.7 Test `build_kind_paths`: output contains GET/POST collection and GET/PUT/DELETE item paths with documented path parameters, `?watch=true` query param on list, and correct response codes (201, 200, 404, 409, 422)
- [ ] 4.8 Test full spec with registered schemas: create ObjectService with InMemoryStore, register a Schema, call `build_openapi_spec`, assert dynamic paths and components appear
- [ ] 4.9 Test spec reflects mutations: register schema → build spec → delete schema → build spec → assert dynamic paths removed
- [ ] 4.10 Verify `cargo test` passes with no warnings

## 5. Verification

- [ ] 5.1 Verify `cargo build` succeeds with no warnings
- [ ] 5.2 Verify `GET /openapi` returns valid OpenAPI 3.0.3 JSON with no registered schemas (static components + Schema CRUD paths only)
- [ ] 5.3 Register a Schema and verify `GET /openapi` includes dynamic paths and components for that kind
- [ ] 5.4 Verify component naming: register schemas with different groups and confirm no name collisions in `components/schemas`
- [ ] 5.5 Verify Swagger UI loads in browser (if implemented)
