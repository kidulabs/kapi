## 1. Create directory structure and mod.rs

- [ ] 1.1 Create `src/openapi/` directory
- [ ] 1.2 Create `src/openapi/mod.rs` with module declarations (`mod components;`, `mod paths;`, `mod swagger;`) and re-exports (`pub use components::component_name;`, `pub use paths::build_openapi_spec;`, `pub use swagger::get_swagger_ui_handler;`, and handler re-export)
- [ ] 1.3 Delete `src/openapi.rs`

## 2. Extract components.rs

- [ ] 2.1 Move `component_name()`, `build_static_components()`, `build_kind_data_component()`, `build_kind_stored_object_component()`, `build_kind_list_response_component()` to `src/openapi/components.rs`
- [ ] 2.2 Update imports in `components.rs` (use `crate::object::types::SchemaData`)
- [ ] 2.3 Set visibility: `pub fn component_name()`, `pub(crate)` for internal builders

## 3. Extract paths.rs

- [ ] 3.1 Move `build_static_paths()`, `build_kind_paths()`, `build_create_request_schema()`, `schema_create_request_schema()`, `build_openapi_spec()` to `src/openapi/paths.rs`
- [ ] 3.2 Update imports in `paths.rs` (use `crate::openapi::components::*`, `crate::object::types::SchemaData`, `crate::object::service::ObjectService`, `crate::error::AppError`, `crate::store::ResourceKey`)
- [ ] 3.3 Set visibility: `pub(crate) fn build_openapi_spec()`, private for internal builders

## 4. Extract swagger.rs

- [ ] 4.1 Move `SWAGGER_UI_HTML` constant and `get_swagger_ui_handler()` to `src/openapi/swagger.rs`
- [ ] 4.2 Update imports (use `axum::response::Html`)
- [ ] 4.3 Set visibility: `pub fn get_swagger_ui_handler()`

## 5. Move handlers to mod.rs

- [ ] 5.1 Move `get_openapi_handler()` to `src/openapi/mod.rs` (or a `handlers` submodule if preferred)
- [ ] 5.2 Update imports in `mod.rs` (use `crate::routes::AppState`, `crate::error::AppError`, `axum::*`)
- [ ] 5.3 Set visibility: `pub fn get_openapi_handler()`

## 6. Migrate tests

- [ ] 6.1 Move all `#[cfg(test)]` tests from deleted `openapi.rs` into `src/openapi/mod.rs`
- [ ] 6.2 Update test imports to reference sibling modules (`use super::components::*`, `use super::paths::*`, etc.)
- [ ] 6.3 Verify `make_test_service()` helper compiles with new module paths

## 7. Update roadmap.md

- [ ] 7.1 Update Module Tree section: change `openapi.rs` to `openapi/` directory with `mod.rs`, `components.rs`, `paths.rs`, `swagger.rs`
- [ ] 7.2 Correct openapi-spec requirement: dynamic paths do NOT include group/version/kind as path parameters (GVK is baked into URL)

## 8. Correct openapi-spec delta spec

- [ ] 8.1 Update `specs/openapi-spec/spec.md` to fix the path parameter requirement (Requirement: Path parameters are documented in OpenAPI) — change from requiring group/version/kind params to stating only `name` is a path param, GVK is in URL

## 9. Validation

- [ ] 9.1 Run `cargo check` — confirm zero errors
- [ ] 9.2 Run `cargo test` — confirm all tests pass
- [ ] 9.3 Run `cargo clippy` — confirm no new warnings
- [ ] 9.4 Verify `curl /openapi` still returns valid OpenAPI 3.0.3 JSON
- [ ] 9.5 Verify `curl /swagger-ui` still returns Swagger UI HTML
