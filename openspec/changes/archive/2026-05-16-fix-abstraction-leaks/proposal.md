## Why

Several internal modules expose concrete implementation types that should be hidden behind traits. `ObjectService` stores a concrete `EventBus` and `jsonschema::Validator`, leaking external crate dependencies and preventing substitution of alternative implementations. `InMemoryStore` is publicly exported from the crate, allowing external consumers to depend on the concrete memory store. These leaks make the codebase harder to test with mocks and couple layers that should be decoupled.

## What Changes

- Introduce an `EventPublisher` trait abstracting the event bus behind `publish()` and `subscribe()` methods, implemented by `EventBus`
- Introduce a `SchemaValidator` trait and `JsonSchemaValidator` wrapper to isolate the `jsonschema` crate from `ObjectService`
- Change `ObjectService` fields to trait objects (`Arc<dyn EventPublisher>`, `Arc<dyn SchemaValidator>`) instead of concrete types
- Replace `ObjectService::event_bus()` public accessor with a `subscribe()` method that delegates to the event publisher
- Restrict `InMemoryStore` module visibility from `pub` to `pub(crate)` so external consumers cannot depend on it
- Make `AppState.object_service` field private, adding a constructor and getter accessor

## Capabilities

### New Capabilities

<!-- None — all changes are modifications to existing capabilities -->

### Modified Capabilities

- `event-bus`: Add `EventPublisher` trait with `publish()` and `subscribe()` methods. `EventBus` implements `EventPublisher`. Trait re-exported from event module.
- `meta-schema`: Add `SchemaValidator` trait (`is_valid()`, `validate()`) and `SchemaValidationError` struct. Add `JsonSchemaValidator` wrapper implementing `SchemaValidator`. `compile_meta_schema()` returns `JsonSchemaValidator` instead of raw `jsonschema::Validator`.
- `object-service`: `event_bus` field changed to `Arc<dyn EventPublisher>`. `meta_validator` and `schema_cache` fields changed to use `Arc<dyn SchemaValidator>`. `subscribe()` method replaces `event_bus()` accessor. Constructor parameters use trait objects.
- `object-store`: `InMemoryStore` module visibility restricted to `pub(crate)`. External crate consumers can no longer construct or reference `InMemoryStore` directly.

## Impact

- **Event module** (`src/event/`): New `EventPublisher` trait in `bus.rs`, re-export from `mod.rs`
- **Schema module** (`src/schema/`): New `SchemaValidator` trait, `SchemaValidationError`, and `JsonSchemaValidator` in `meta_schema.rs`, re-export from `mod.rs`
- **ObjectService** (`src/object/service.rs`): Field types changed to trait objects, `subscribe()` added, `event_bus()` removed
- **Handlers** (`src/object/handler.rs`): Watch subscription calls changed from `event_bus().subscribe()` to `subscribe()`, `WatchStream` import removed
- **Routes** (`src/routes.rs`): `AppState.object_service` field made private, constructor and getter added
- **Main** (`src/main.rs`): Wire-up updated to cast `EventBus` to `Arc<dyn EventPublisher>`, `AppState` constructed via `AppState::new()`
- **Tests**: Helpers in `object/service.rs`, `openapi.rs`, and `schema/meta_schema.rs` updated with new types
- No API behavior changes — all existing CRUD endpoints, SSE watch, and OpenAPI spec generation work identically
