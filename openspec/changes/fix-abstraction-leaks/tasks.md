## 1. EventPublisher trait

- [ ] 1.1 Define `EventPublisher` trait in `src/event/bus.rs` with `publish()` and `subscribe()` methods, requiring `Send + Sync`
- [ ] 1.2 Implement `EventPublisher` for `EventBus`, delegating to existing inherent methods
- [ ] 1.3 Re-export `EventPublisher` from `src/event/mod.rs`

## 2. SchemaValidator trait and JsonSchemaValidator wrapper

- [ ] 2.1 Define `SchemaValidationError` struct in `src/schema/meta_schema.rs` with `instance_path` and `message` fields
- [ ] 2.2 Define `SchemaValidator` trait in `src/schema/meta_schema.rs` with `is_valid()` and `validate()` methods, requiring `Send + Sync`
- [ ] 2.3 Define `JsonSchemaValidator` struct wrapping `jsonschema::Validator`, with `compile()` associated function
- [ ] 2.4 Implement `SchemaValidator` for `JsonSchemaValidator`
- [ ] 2.5 Update `compile_meta_schema()` to return `Result<JsonSchemaValidator, anyhow::Error>`
- [ ] 2.6 Re-export `SchemaValidator`, `SchemaValidationError`, `JsonSchemaValidator` from `src/schema/mod.rs`
- [ ] 2.7 Update meta-schema tests to use `SchemaValidator` trait methods instead of `jsonschema::Validator` directly

## 3. ObjectService field and method changes

- [ ] 3.1 Change `event_bus` field type from `EventBus` to `Arc<dyn EventPublisher>`
- [ ] 3.2 Change `meta_validator` field type from `Arc<Validator>` to `Arc<dyn SchemaValidator>`
- [ ] 3.3 Change `schema_cache` field type from `DashMap<String, Arc<Validator>>` to `DashMap<String, Arc<dyn SchemaValidator>>`
- [ ] 3.4 Update `new()` constructor parameters: `event_bus: Arc<dyn EventPublisher>`, `meta_validator: Arc<dyn SchemaValidator>`
- [ ] 3.5 Add `subscribe(&self, key: &ResourceKey) -> WatchStream` method, remove `event_bus()` accessor
- [ ] 3.6 Replace `draft202012::options().build()` calls with `JsonSchemaValidator::compile()` in `validate_and_create_schema` and `validate_and_update_schema`
- [ ] 3.7 Replace `self.meta_validator.iter_errors()` with `self.meta_validator.validate()` in `validate_and_create_schema` and `validate_and_update_schema`, mapping `SchemaValidationError.message` to `String`
- [ ] 3.8 Replace `validator.iter_errors()` with `validator.validate()` in `validate_and_create_object` and `validate_and_update_object`, mapping `SchemaValidationError` to `object::types::ValidationError`
- [ ] 3.9 Update imports: remove `jsonschema::Validator`, `jsonschema::draft202012`; add `crate::schema::{JsonSchemaValidator, SchemaValidator}`; change `crate::event::EventBus` to `crate::event::EventPublisher`
- [ ] 3.10 Update test helper `make_service()` to use `Arc<dyn EventPublisher>` and `Arc<dyn SchemaValidator>`

## 4. Handler and route updates

- [ ] 4.1 In `src/object/handler.rs`: change `state.object_service.event_bus().subscribe(&key)` to `state.object_service.subscribe(&key)`; remove `use crate::event::WatchStream`
- [ ] 4.2 In `src/routes.rs`: make `AppState.object_service` field private, add `AppState::new()` constructor and `object_service()` getter
- [ ] 4.3 Update all handler call sites from `state.object_service.xxx()` to `state.object_service().xxx()` (6 sites in handler.rs, 1 site in openapi.rs)

## 5. Module visibility

- [ ] 5.1 In `src/store/mod.rs`: change `pub mod memory;` to `pub(crate) mod memory;`

## 6. Main.rs wiring

- [ ] 6.1 Cast event bus to trait object: `let event_bus: Arc<dyn EventPublisher> = Arc::new(EventBus::default())`
- [ ] 6.2 Cast meta-validator to trait object via `Arc::new(compile_meta_schema()?)`
- [ ] 6.3 Construct `AppState` via `AppState::new(Arc::new(object_service))`
- [ ] 6.4 Update imports: add `use crate::event::EventPublisher;`

## 7. Test helper updates

- [ ] 7.1 Update `openapi.rs` test helper `make_test_service()` to use trait objects for event bus and meta-validator
- [ ] 7.2 Verify all existing tests compile and pass with the new types

## 8. Validation

- [ ] 8.1 Run `cargo check` — confirm zero errors
- [ ] 8.2 Run `cargo test` — confirm all 57 tests pass

## 9. Roadmap corrections — stale dependencies

- [ ] 9.1 Remove `utoipa = "5"` and `utoipa-swagger-ui = "9"` from `Cargo.toml` (P8 replaced them with dynamic OpenAPI generation; no code imports them)
- [ ] 9.2 Remove `utoipa` and `utoipa-swagger-ui` from roadmap Dependencies section (line 230)
- [ ] 9.3 Fix roadmap dependency name: `futures` → `futures-util` (line 229)
- [ ] 9.4 Add missing `base64` to roadmap Dependencies section (used in continue token encoding, line 21 of Cargo.toml)

## 10. Roadmap corrections — architecture and module tree

- [ ] 10.1 Update Architecture diagram (lines 13-24): replace `Admission Validation` layer with a note that validation is inline in `ObjectService`; update `AppState` box to show `ObjectService` wrapping `ObjectStore` + `EventBus` instead of showing them directly
- [ ] 10.2 Update Architecture diagram after abstraction fix: `EventBus` → `EventPublisher (trait)`, add `SchemaValidator (trait)` inside ObjectService
- [ ] 10.3 Fix Module Tree comment for `object/types.rs` (line 210): `ResourceKey` is defined in `store/mod.rs` and re-exported, not defined in types.rs
- [ ] 10.4 Update Module Tree comment for `object/service.rs` (line 211): `ObjectService<ObjectStore + EventBus>` → `ObjectService (wraps Arc<dyn ObjectStore>, Arc<dyn EventPublisher>, Arc<dyn SchemaValidator>)`
- [ ] 10.5 Update Module Tree entry for `event/bus.rs` (line 215): add note about `EventPublisher` trait
- [ ] 10.6 Update Module Tree entry for `schema/meta_schema.rs` (line 207): add note about `SchemaValidator` trait, `JsonSchemaValidator`, `SchemaValidationError`
- [ ] 10.7 Correct Request Flow section (lines 238-290): change `event_bus.publish()` and `event_bus.subscribe()` references to go through `ObjectService` (publish via internal delegation, subscribe via `ObjectService::subscribe()`)

## 11. Roadmap corrections — task checkboxes

- [ ] 11.1 Mark P8 T55 as done `[x]` — P8 change is archived, dynamic OpenAPI generation is implemented and tested
- [ ] 11.2 Verify all other checkbox states in P0-P5 match actual completion (P0-P5 are all `[x]` — confirm nothing was missed)
