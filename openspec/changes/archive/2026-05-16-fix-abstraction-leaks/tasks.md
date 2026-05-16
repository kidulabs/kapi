## 1. EventPublisher trait

- [x] 1.1 Define `EventPublisher` trait in `src/event/bus.rs` with `publish()` and `subscribe()` methods, requiring `Send + Sync`
- [x] 1.2 Implement `EventPublisher` for `EventBus`, delegating to existing inherent methods
- [x] 1.3 Re-export `EventPublisher` from `src/event/mod.rs`

## 2. SchemaValidator trait and JsonSchemaValidator wrapper

- [x] 2.1 Define `SchemaValidationError` struct in `src/schema/meta_schema.rs` with `instance_path` and `message` fields
- [x] 2.2 Define `SchemaValidator` trait in `src/schema/meta_schema.rs` with `is_valid()` and `validate()` methods, requiring `Send + Sync`
- [x] 2.3 Define `JsonSchemaValidator` struct wrapping `jsonschema::Validator`, with `compile()` associated function
- [x] 2.4 Implement `SchemaValidator` for `JsonSchemaValidator`
- [x] 2.5 Update `compile_meta_schema()` to return `Result<JsonSchemaValidator, anyhow::Error>`
- [x] 2.6 Re-export `SchemaValidator`, `SchemaValidationError`, `JsonSchemaValidator` from `src/schema/mod.rs`
- [x] 2.7 Update meta-schema tests to use `SchemaValidator` trait methods instead of `jsonschema::Validator` directly

## 3. ObjectService field and method changes

- [x] 3.1 Change `event_bus` field type from `EventBus` to `Arc<dyn EventPublisher>`
- [x] 3.2 Change `meta_validator` field type from `Arc<Validator>` to `Arc<dyn SchemaValidator>`
- [x] 3.3 Change `schema_cache` field type from `DashMap<String, Arc<Validator>>` to `DashMap<String, Arc<dyn SchemaValidator>>`
- [x] 3.4 Update `new()` constructor parameters: `event_bus: Arc<dyn EventPublisher>`, `meta_validator: Arc<dyn SchemaValidator>`
- [x] 3.5 Add `subscribe(&self, key: &ResourceKey) -> WatchStream` method, remove `event_bus()` accessor
- [x] 3.6 Replace `draft202012::options().build()` calls with `JsonSchemaValidator::compile()` in `validate_and_create_schema` and `validate_and_update_schema`
- [x] 3.7 Replace `self.meta_validator.iter_errors()` with `self.meta_validator.validate()` in `validate_and_create_schema` and `validate_and_update_schema`, mapping `SchemaValidationError.message` to `String`
- [x] 3.8 Replace `validator.iter_errors()` with `validator.validate()` in `validate_and_create_object` and `validate_and_update_object`, mapping `SchemaValidationError` to `object::types::ValidationError`
- [x] 3.9 Update imports: remove `jsonschema::Validator`, `jsonschema::draft202012`; add `crate::schema::{JsonSchemaValidator, SchemaValidator}`; change `crate::event::EventBus` to `crate::event::EventPublisher`
- [x] 3.10 Update test helper `make_service()` to use `Arc<dyn EventPublisher>` and `Arc<dyn SchemaValidator>`

## 4. Handler and route updates

- [x] 4.1 In `src/object/handler.rs`: change `state.object_service.event_bus().subscribe(&key)` to `state.object_service.subscribe(&key)`; remove `use crate::event::WatchStream`
- [x] 4.2 In `src/routes.rs`: make `AppState.object_service` field private, add `AppState::new()` constructor and `object_service()` getter
- [x] 4.3 Update all handler call sites from `state.object_service.xxx()` to `state.object_service().xxx()` (6 sites in handler.rs, 1 site in openapi.rs)

## 5. Module visibility

- [x] 5.1 In `src/store/mod.rs`: change `pub mod memory;` to `pub(crate) mod memory;`

## 6. Main.rs wiring

- [x] 6.1 Cast event bus to trait object: `let event_bus: Arc<dyn EventPublisher> = Arc::new(EventBus::default())`
- [x] 6.2 Cast meta-validator to trait object via `Arc::new(compile_meta_schema()?)`
- [x] 6.3 Construct `AppState` via `AppState::new(Arc::new(object_service))`
- [x] 6.4 Update imports: add `use crate::event::EventPublisher;`

## 7. Test helper updates

- [x] 7.1 Update `openapi.rs` test helper `make_test_service()` to use trait objects for event bus and meta-validator
- [x] 7.2 Verify all existing tests compile and pass with the new types

## 8. Validation

- [x] 8.1 Run `cargo check` — confirm zero errors
- [x] 8.2 Run `cargo test` — confirm all 57 tests pass

## 9. Roadmap corrections — stale dependencies

- [x] 9.1 Remove `utoipa = "5"` and `utoipa-swagger-ui = "9"` from `Cargo.toml` (P8 replaced them with dynamic OpenAPI generation; no code imports them)
- [x] 9.2 Remove `utoipa` and `utoipa-swagger-ui` from roadmap Dependencies section (line 230)
- [x] 9.3 Fix roadmap dependency name: `futures` → `futures-util` (line 229)
- [x] 9.4 Add missing `base64` to roadmap Dependencies section (used in continue token encoding, line 21 of Cargo.toml)

## 10. Roadmap corrections — architecture and module tree

- [x] 10.1 Update Architecture diagram (lines 13-24): replace `Admission Validation` layer with a note that validation is inline in `ObjectService`; update `AppState` box to show `ObjectService` wrapping `ObjectStore` + `EventBus` instead of showing them directly
- [x] 10.2 Update Architecture diagram after abstraction fix: `EventBus` → `EventPublisher (trait)`, add `SchemaValidator (trait)` inside ObjectService
- [x] 10.3 Fix Module Tree comment for `object/types.rs` (line 210): `ResourceKey` is defined in `store/mod.rs` and re-exported, not defined in types.rs
- [x] 10.4 Update Module Tree comment for `object/service.rs` (line 211): `ObjectService<ObjectStore + EventBus>` → `ObjectService (wraps Arc<dyn ObjectStore>, Arc<dyn EventPublisher>, Arc<dyn SchemaValidator>)`
- [x] 10.5 Update Module Tree entry for `event/bus.rs` (line 215): add note about `EventPublisher` trait
- [x] 10.6 Update Module Tree entry for `schema/meta_schema.rs` (line 207): add note about `SchemaValidator` trait, `JsonSchemaValidator`, `SchemaValidationError`
- [x] 10.7 Correct Request Flow section (lines 238-290): change `event_bus.publish()` and `event_bus.subscribe()` references to go through `ObjectService` (publish via internal delegation, subscribe via `ObjectService::subscribe()`)

## 11. Roadmap corrections — task checkboxes

- [x] 11.1 Mark P8 T55 as done `[x]` — P8 change is archived, dynamic OpenAPI generation is implemented and tested
- [x] 11.2 Verify all other checkbox states in P0-P5 match actual completion (P0-P5 are all `[x]` — confirm nothing was missed)
