## Context

`src/openapi.rs` is 1062 lines containing:
- `build_openapi_spec()` — top-level spec orchestrator
- `build_static_components()` — 10 builtin schema definitions
- `build_static_paths()` — Schema CRUD path definitions
- `build_kind_data_component()`, `build_kind_stored_object_component()`, `build_kind_list_response_component()` — dynamic per-kind component builders
- `build_kind_paths()` — dynamic per-kind path builder
- `build_create_request_schema()`, `schema_create_request_schema()` — request body schema helpers
- `component_name()` — public utility for name conversion
- `get_openapi_handler()`, `get_swagger_ui_handler()` — HTTP handlers
- `SWAGGER_UI_HTML` — static HTML constant
- 20+ unit and integration tests

## Goals / Non-Goals

**Goals:**
- Split into 4 focused files (components, paths, swagger, mod)
- Preserve all public API (`get_openapi_handler`, `get_swagger_ui_handler`, `component_name`)
- All existing tests pass without behavioral changes
- Match the directory pattern used by other modules

**Non-Goals:**
- No behavioral or API changes
- No spec requirement changes (except correcting the path parameter mismatch)
- No P-Future caching implementation (separate change)

## Decisions

### Decision 1: Four-file split

```
src/openapi/
├── mod.rs          # Module declarations, re-exports, HTTP handlers, tests
├── components.rs   # Static + dynamic component schema builders
├── paths.rs        # Static + dynamic path builders + build_openapi_spec()
└── swagger.rs      # Swagger UI HTML + handler
```

**Rationale:** Components and paths are the two largest logical chunks (~300 lines each). Swagger UI is self-contained (~20 lines). `mod.rs` holds the handlers and re-exports, keeping the public API stable.

**Alternatives considered:**
- *Five files (separate tests.rs)*: Tests are tightly coupled to internal functions across components and paths. Splitting them adds indirection without benefit. Tests stay in `mod.rs` as `#[cfg(test)]` block, matching the pattern used in other modules.
- *Keep handlers in a separate handler.rs*: Only two handlers exist, both trivial. Not worth a separate file.

### Decision 2: Re-exports preserve public API

`mod.rs` re-exports key items so that external consumers see no change:

```rust
pub use components::component_name;
pub use paths::build_openapi_spec;
pub use swagger::get_swagger_ui_handler;
pub use handlers::get_openapi_handler;
```

This ensures `crate::openapi::get_openapi_handler` and `crate::openapi::component_name` continue to work.

### Decision 3: `build_openapi_spec()` lives in `paths.rs`

`build_openapi_spec()` orchestrates both components and paths. It logically belongs with paths because:
- It calls `build_static_components()` and `build_static_paths()` as starting points
- It then iterates schemas to call dynamic component and path builders
- The final document assembly (merging paths + components) is a path-centric concern

**Alternative considered:**
- *Put in mod.rs as the orchestrator*: Would make mod.rs a "god file" again. Better to keep it with its primary dependency (paths).

### Decision 4: Correct the openapi-spec requirement

The existing spec says dynamic paths must document `group`, `version`, `kind` as path parameters. This contradicts the roadmap design (GVK baked into URL) and the implementation (only `{name}` is a path param). The spec shall be corrected to:

> Dynamic paths SHALL NOT include `group`, `version`, `kind` as path parameters. These are baked into the URL path. Only `name` is documented as a path parameter on item paths.

## Risks / Trade-offs

- **Import churn**: Every function that was `pub fn` in the single file may need `pub(crate)` or `pub` adjustments. Internal helpers become `pub(crate)` or private to their submodule.
- **Test access**: Tests in `mod.rs` use `super::*` to access internal functions. Moving functions to sibling modules requires `use crate::openapi::components::...` in tests. This is a minor mechanical change.
