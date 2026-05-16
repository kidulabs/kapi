## Context

After the `type-erase-objectservice` refactor (which decoupled `routes.rs` from `InMemoryStore`), four remaining abstraction leaks were identified:

1. `ObjectService` stores concrete `EventBus` — no trait, swap requires changing service code
2. `ObjectService` stores `Arc<jsonschema::Validator>` — external crate dependency leaks through the service layer
3. `InMemoryStore` is `pub` in the module tree — external consumers can construct/depend on it
4. `AppState.object_service` is `pub` — handlers reach into state struct internals

These leaks make testing with mocks harder and couple layers that should be independently swappable.

## Goals / Non-Goals

**Goals:**
- Decouple `ObjectService` from concrete `EventBus` and `jsonschema::Validator` via traits
- Restrict `InMemoryStore` to crate-internal visibility (`pub(crate)`)
- Make `AppState` fields private with a getter accessor
- Preserve all existing behavior — no functional changes to CRUD, SSE watch, or OpenAPI

**Non-Goals:**
- Introduce a trait for `ObjectService` itself (current needs don't justify the complexity)
- Add new storage backends or event bus implementations
- Change `WatchStream` type or stream abstraction
- Modify handler signatures beyond the call path

## Decisions

### Decision 1: EventPublisher trait with Arc<dyn EventPublisher>

`ObjectService` currently stores `event_bus: EventBus` (concrete). A new `EventPublisher` trait is introduced in `src/event/bus.rs`:

```rust
pub trait EventPublisher: Send + Sync {
    fn publish(&self, key: &ResourceKey, event: WatchEvent);
    fn subscribe(&self, key: &ResourceKey) -> WatchStream;
}
```

`EventBus` implements `EventPublisher` via delegation (the existing inherent methods remain for backward compat in tests). `ObjectService` stores `event_bus: Arc<dyn EventPublisher>`.

**Alternatives considered:**
- *Generic ObjectService over E*: `ObjectService<S: ObjectStore, E: EventPublisher>` — more type params propagate to every handler signature, negating the benefit of type-erasure.
- *Separate publish/subscribe traits*: Over-complicated for what is essentially one concern (event distribution).

**Decision: Single `EventPublisher` trait, stored as `Arc<dyn EventPublisher>`.**

### Decision 2: subscribe() on ObjectService, not event_bus() accessor

Currently `ObjectService` exposes `pub fn event_bus(&self) -> &EventBus`, and handlers call `state.object_service.event_bus().subscribe(&key)` to get a `WatchStream`. With the trait in place, two options exist:

1. Keep an accessor returning `&dyn EventPublisher` — handlers still see the event abstraction
2. Add `ObjectService::subscribe()` that delegates internally — handlers see only the service

Option 2 is chosen because handlers should not need to know the event bus exists at all. The new method:

```rust
pub fn subscribe(&self, key: &ResourceKey) -> WatchStream {
    self.event_bus.subscribe(key)
}
```

Handler code changes from:
```rust
let stream: WatchStream = state.object_service.event_bus().subscribe(&key);
```
to:
```rust
let stream = state.object_service.subscribe(&key);
```

This also eliminates the `use crate::event::WatchStream` import from handlers.

### Decision 3: SchemaValidator trait with JsonSchemaValidator wrapper

`ObjectService` currently stores `meta_validator: Arc<Validator>` and `schema_cache: DashMap<String, Arc<Validator>>` where `Validator = jsonschema::Validator`. The `jsonschema` crate dependency leaks into service.rs through:
- Field type annotations
- `draft202012::options().build(...)` calls during schema creation/update
- `iter_errors()` calls for collecting validation failures

A new `SchemaValidator` trait is added to `src/schema/meta_schema.rs`:

```rust
pub struct SchemaValidationError {
    pub instance_path: String,
    pub message: String,
}

pub trait SchemaValidator: Send + Sync {
    fn is_valid(&self, instance: &Value) -> bool;
    fn validate(&self, instance: &Value) -> Vec<SchemaValidationError>;
}
```

`SchemaValidationError` is a new domain type (not reusing `object::types::ValidationError`) to avoid a circular dependency: `schema` → `object::types` (which re-exports `store::ResourceKey`, and `store` already depends on `object::types`).

A `JsonSchemaValidator` wrapper struct implements `SchemaValidator`:

```rust
pub struct JsonSchemaValidator {
    inner: jsonschema::Validator,
}

impl JsonSchemaValidator {
    pub fn compile(schema_json: &Value) -> Result<Self, anyhow::Error> { ... }
}

impl SchemaValidator for JsonSchemaValidator {
    fn is_valid(&self, instance: &Value) -> bool { self.inner.is_valid(instance) }
    fn validate(&self, instance: &Value) -> Vec<SchemaValidationError> {
        self.inner.iter_errors(instance).map(|e| SchemaValidationError { ... }).collect()
    }
}
```

`compile_meta_schema()` returns `Result<JsonSchemaValidator, anyhow::Error>` instead of `Result<Validator, anyhow::Error>`.

Inside `ObjectService`, `draft202012::options().build(...)` is replaced with `JsonSchemaValidator::compile(...)`. The `validate()` method replaces `iter_errors()` with a mapping step:
- Meta-schema path: `.map(|e| e.message)` for `Vec<String>` errors
- Object validation path: `.map(|e| ValidationError { path: e.instance_path, message: e.message })` for `Vec<ValidationError>` errors

**Alternatives considered:**
- *Keep jsonschema::Validator in ObjectService*: Couples service to external crate, no mockability.
- *Return Box<dyn Iterator> from trait*: Complex lifetime management, harder to implement.
- *Use object::types::ValidationError in SchemaValidator*: Creates circular dependency.

**Decision: Domain `SchemaValidationError` type, `JsonSchemaValidator` wrapper, `Arc<dyn SchemaValidator>` in service fields.**

### Decision 4: pub(crate) visibility for InMemoryStore

A one-line change in `src/store/mod.rs`: `pub mod memory;` → `pub(crate) mod memory;`.

`pub(crate)` preserves access from main.rs (composition root) and all test modules (`#[cfg(test)]` code within the crate) while preventing external crate consumers from depending on the concrete type.

**Alternative considered:**
- *Remove pub entirely (private)*: `crate::store::memory::InMemoryStore` paths in `openapi.rs` tests would break. `pub(crate)` is the right level.

### Decision 5: Private AppState field with getter

`AppState.object_service` changes from `pub` to private:

```rust
#[derive(Clone)]
pub struct AppState {
    object_service: Arc<ObjectService>,
}

impl AppState {
    pub fn new(object_service: Arc<ObjectService>) -> Self { Self { object_service } }
    pub fn object_service(&self) -> &ObjectService { &self.object_service }
}
```

Handlers change from `state.object_service.xxx()` to `state.object_service().xxx()` — one extra function call, zero overhead after inlining.

**Alternative considered:**
- *Leave pub field*: Acceptable for simplicity, but inconsistent with the other abstraction goals. A private field with getter is a minimal change that future-proofs internal restructuring.

## Risks / Trade-offs

- **Trait object vtable overhead**: `Arc<dyn EventPublisher>` and `Arc<dyn SchemaValidator>` add an indirect call per method invocation. For CRUD operations where store I/O and JSON parsing dominate, this is negligible.
- **Arc coercion in test helpers**: `Arc::new(EventBus::default())` is `Arc<EventBus>`, must coerce to `Arc<dyn EventPublisher>`. Explicit type annotations (`let eb: Arc<dyn EventPublisher> = Arc::new(...)`) prevent inference failures.
- **SchemaValidationError → ValidationError mapping**: An extra allocation per validation failure. Acceptable — validation errors are rare and the mapping is a trivial field rename.
- **jsonschema crate still a dependency**: The `JsonSchemaValidator` wrapper hides the type but still depends on the crate internally. This is intentional — the goal is isolation, not removal. The crate can be swapped by changing only `meta_schema.rs`.
