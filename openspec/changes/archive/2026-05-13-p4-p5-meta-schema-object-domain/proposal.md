## Why

P4 (Meta-Schema) and P5 (Object Domain) are the next phases in the roadmap. P4 provides the meta-schema constant and compilation function needed to validate Schema registrations. P5 builds the `ObjectService` that orchestrates validation, storage, and event publishing, plus the Axum handlers that expose the full CRUD + watch API. Without P4+P5, the server has types and a store but no business logic or HTTP surface.

## What Changes

- **Implement meta-schema** (`src/schema/meta_schema.rs`): Hardcoded JSON Schema constant (Draft 2020-12) defining valid Schema registration payloads (`targetGroup`, `targetVersion`, `targetKind`, `jsonSchema`). Compilation function returning a `jsonschema::Validator` for use at server startup.
- **Add `InvalidSchema` error variant** to `AppError`: Distinguishes "the schema itself is broken" from "the object doesn't match its schema." Maps to HTTP 422 with the compilation error message.
- **Implement `ObjectService`** (`src/object/service.rs`): Wraps `Arc<dyn ObjectStore>` + `EventBus` + compiled meta-validator + in-memory schema cache (`DashMap<ResourceKey, Arc<Validator>>`). Validation dispatch: Schema objects validated against meta-schema, regular objects validated against cached compiled schemas. Schema deletion guard (409 if objects exist). Service layer publishes events after every mutation.
- **Implement Axum handlers** (`src/object/handler.rs`): Create, get, list, update, delete handlers for `/apis/{group}/{version}/{kind}` and `/apis/{group}/{version}/{kind}/{name}`. `?watch=true` detection in list handler returns SSE stream. Update handler validates URL key/name match request body.
- **Wire routes** (`src/routes.rs`): Compose object routes under `/apis/{group}/{version}` with path parameter extraction.
- **Wire application** (`src/main.rs`): Construct `AppState` (store, event bus, object service with compiled meta-schema), build router, bind to port from env or default 8080.
- **Roadmap audit**: Verify P4 checkbox states, correct any deviations between roadmap and actual codebase.

## Impact

- Modified: `src/schema/meta_schema.rs` (from TODO stub to implementation)
- Modified: `src/object/service.rs` (from TODO stub to implementation)
- Modified: `src/object/handler.rs` (from TODO stub to implementation)
- Modified: `src/object/types.rs` (add `SchemaData` struct)
- Modified: `src/error.rs` (add `InvalidSchema` variant)
- Modified: `src/routes.rs` (from TODO stub to route composition)
- Modified: `src/main.rs` (add AppState, wiring, port from env)
- New spec: `specs/meta-schema/` — meta-schema constant and compilation
- New spec: `specs/object-service/` — service layer, schema cache, validation dispatch
- New spec: `specs/object-handlers/` — HTTP handlers, route wiring, watch detection
- Modified spec: `specs/error-handling/` — add `InvalidSchema` variant
- Modified: `roadmap.md` — correct P4 checkbox states, audit all phases
