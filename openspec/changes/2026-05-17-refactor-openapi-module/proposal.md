## Why

`src/openapi.rs` is a 1062-line monolithic file that mixes multiple distinct concerns: static component schemas, static path definitions, dynamic component generation, dynamic path generation, Swagger UI serving, the top-level spec builder, and HTTP handlers. Every other module in the project (`object/`, `event/`, `schema/`, `store/`, `middleware/`) uses a directory structure to separate concerns. `openapi.rs` is the sole exception, making it harder to navigate, review, and extend.

## What Changes

- Split `src/openapi.rs` into a `src/openapi/` directory with focused submodules:
  - `mod.rs` — module declarations, re-exports, and HTTP handlers
  - `components.rs` — static and dynamic component schema builders
  - `paths.rs` — static and dynamic path builders
  - `swagger.rs` — Swagger UI HTML constant and handler
- Preserve all public API: `get_openapi_handler`, `get_swagger_ui_handler`, `component_name` remain accessible at `crate::openapi::`
- Update `src/main.rs` module declaration from `mod openapi;` to `mod openapi;` (no change needed — Rust resolves both)
- Update `src/routes.rs` import path from `crate::openapi::` (no change needed — re-exports preserved)
- No behavioral changes — all existing tests pass unchanged

## Capabilities

### New Capabilities

- *(none — this is a structural refactor)*

### Modified Capabilities

- `openapi-spec`: Module restructured from single file to directory. No spec requirements change — all ADDED requirements remain valid.

## Impact

- **New**: `src/openapi/mod.rs` — module tree, re-exports, HTTP handlers
- **New**: `src/openapi/components.rs` — `build_static_components()`, `build_kind_data_component()`, `build_kind_stored_object_component()`, `build_kind_list_response_component()`, `component_name()`
- **New**: `src/openapi/paths.rs` — `build_static_paths()`, `build_kind_paths()`, `build_create_request_schema()`, `schema_create_request_schema()`, `build_openapi_spec()`
- **New**: `src/openapi/swagger.rs` — `SWAGGER_UI_HTML`, `get_swagger_ui_handler()`
- **Deleted**: `src/openapi.rs`
- **Updated**: `src/routes.rs` — import paths unchanged (re-exports preserve compatibility)
- **Updated**: `roadmap.md` — Module Tree section updated to show `openapi/` directory structure

## Roadmap Deviations Found

During analysis, the following deviations between the roadmap and the codebase were identified:

1. **P6 Middleware not wired** (T43–T46): `auth.rs` and `metrics.rs` are `//! TODO` stubs. The middleware stack in `routes.rs` only includes `TraceLayer` and `CorsLayer`, not the planned `AuthLayer` + `MetricsLayer` chain. Roadmap checkboxes correctly show these as `[ ]` — no correction needed, but the deviation is documented.

2. **OpenAPI spec path parameter requirement mismatch**: The existing `openapi-spec` spec requires dynamic paths to document `group`, `version`, `kind` as path parameters. However, the roadmap design explicitly bakes GVK into the URL (no path params), and the implementation only uses `{name}`. The spec requirement is wrong — it should be corrected to reflect that GVK is in the URL, not path parameters.

3. **`/swagger-ui` route**: Roadmap only lists `/swagger-ui/`, but `routes.rs` also registers `/swagger-ui` (without trailing slash). This is a minor enhancement, not a deviation.
