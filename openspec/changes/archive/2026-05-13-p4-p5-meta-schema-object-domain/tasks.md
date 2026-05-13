## Phase 1: Error Handling Extension

- [x] T1: Add `InvalidSchema(String)` variant to `AppError` in `src/error.rs`
- [x] T2: Add `SchemaHasObjects { kind: String, count: usize }` variant to `AppError` in `src/error.rs`
- [x] T3: Update `IntoResponse` for `AppError`:
  - `InvalidSchema` → 422 with `{ "error": "...", "code": "InvalidSchema", "details": { "message": "..." } }`
  - `SchemaHasObjects` → 409 with `{ "error": "...", "code": "SchemaHasObjects", "details": { "kind": "...", "count": N } }`
- [x] T4: Add `SchemaData { target_group, target_version, target_kind, json_schema }` struct in `src/object/types.rs` with `#[serde(rename_all = "camelCase")]`

## Phase 2: Meta-Schema (P4)

- [x] T5: Implement `META_SCHEMA_JSON` constant in `src/schema/meta_schema.rs` — Draft 2020-12 schema requiring `targetGroup`, `targetVersion`, `targetKind`, `jsonSchema` with `unevaluatedProperties: false`
- [x] T6: Implement `compile_meta_schema() -> Result<jsonschema::Validator, anyhow::Error>` using `draft202012::options()`
- [x] T7: Write unit test: valid Schema registration payload passes meta-schema validation
- [x] T8: Write unit test: missing required field fails meta-schema validation
- [x] T9: Write unit test: unknown field fails meta-schema validation (`unevaluatedProperties: false`)
- [x] T10: Write unit test: `jsonSchema` as non-object fails meta-schema validation
- [x] T11: Write unit test: `compile_meta_schema()` returns a working validator

## Phase 3: ObjectService (P5 — Service Layer)

- [x] T12: Define `ObjectService` struct in `src/object/service.rs` with `store`, `event_bus`, `meta_validator`, `schema_cache` fields
- [x] T13: Implement `ObjectService::new(store, event_bus, meta_validator)` constructor
- [x] T14: Implement `ObjectService::create(key, name, data)`:
  - Schema path: meta-schema validate → compile jsonSchema → cache → store.create → publish Added
  - Object path: lookup schema → validate against cached schema → store.create → publish Added
- [x] T15: Implement `ObjectService::get(key, name)` — delegate to store
- [x] T16: Implement `ObjectService::list(key, opts)` — delegate to store
- [x] T17: Implement `ObjectService::update(object)`:
  - Same validation flow as create
  - store.update → publish Modified
- [x] T18: Implement `ObjectService::delete(key, name)`:
  - Schema path: fetch schema → check objects exist → SchemaHasObjects if any
  - store.delete → cache.remove → publish Deleted
- [x] T19: Write unit test: create valid Schema → stored, cached, event published
- [x] T20: Write unit test: create Schema with invalid meta-schema → InvalidSchema, nothing stored
- [x] T21: Write unit test: create Schema with uncompileable jsonSchema → InvalidSchema, nothing stored
- [x] T22: Write unit test: create object for unregistered kind → NotFound
- [x] T23: Write unit test: create object with invalid data → SchemaValidation
- [x] T24: Write unit test: update with correct version → success, Modified event published
- [x] T25: Write unit test: update with wrong version → Conflict, no event published
- [x] T26: Write unit test: delete Schema with no objects → success, cache evicted, Deleted event published
- [x] T27: Write unit test: delete Schema with existing objects → SchemaHasObjects, nothing deleted
- [x] T28: Write unit test: delete regular object → success, Deleted event published
- [x] T29: Write unit test: failed create (duplicate) → no Added event published
- [x] T30: Write unit test: schema cache eviction on Schema delete

## Phase 4: Handlers (P5 — HTTP Layer)

- [x] T31: Implement `create` handler in `src/object/handler.rs` — extract path params, deserialize body, extract name from `metadata.name`, call service, return 201
- [x] T32: Implement `get` handler — extract path params, call service, return 200
- [x] T33: Implement `list` handler — extract path params, check `?watch=true`, branch to list or watch
- [x] T34: Implement `watch` logic in list handler — subscribe to event bus, map WatchEvent to SSE events, return `Sse<impl Stream>`
- [x] T35: Implement `update` handler — extract path params, deserialize as `StoredObject`, validate URL matches body, call service, return 200
- [x] T36: Implement `delete` handler — extract path params, call service, return 200
- [x] T37: Add doc comments to all handlers
- [ ] T38: Write unit test: create valid object → 201
- [ ] T39: Write unit test: create with invalid data → 422
- [ ] T40: Write unit test: create for unregistered kind → 404
- [ ] T41: Write unit test: update correct version → 200
- [ ] T42: Write unit test: update wrong version → 409
- [ ] T43: Write unit test: create valid Schema → 201
- [ ] T44: Write unit test: create invalid Schema → 422
- [ ] T45: Write unit test: delete Schema with objects → 409 with object count

## Phase 5: Route Wiring and Application (P5 — Wiring)

- [x] T46: Implement route composition in `src/routes.rs`:
  - `GET/POST /apis/{group}/{version}/{kind}` → list/create
  - `GET/PUT/DELETE /apis/{group}/{version}/{kind}/{name}` → get/update/delete
- [x] T47: Define `AppState` struct with `object_service: ObjectService<InMemoryStore>`
- [x] T48: Wire `main.rs`:
  - Compile meta-schema at startup
  - Construct InMemoryStore, EventBus, ObjectService
  - Build router with AppState
  - Bind to port from `PORT` env var or default 8080
- [x] T49: Verify `cargo build` succeeds with no warnings
- [x] T50: Verify `cargo test` passes with no warnings
- [x] T51: Verify `cargo run` starts server, `curl http://localhost:8080/apis/kapi.io/v1/Schema` returns empty list (or 404 if no list handler for Schema kind yet)

## Phase 6: Roadmap Audit

- [x] T52: Audit P4 checkbox states in `roadmap.md` — mark T31, T32 as completed after implementation
- [x] T53: Audit all roadmap phases (P0–P3) against actual codebase — verify checkbox accuracy
- [x] T54: Correct any deviations between roadmap descriptions and actual implementation
- [x] T55: Update P5 task descriptions to match finalized design (schema cache, InvalidSchema, SchemaHasObjects)
