## MODIFIED Requirements

### Requirement: ObjectService wraps store, event bus, and validators
The system SHALL define an `ObjectService` struct containing:
- `store: Arc<dyn ObjectStore>` — the storage backend
- `event_bus: Arc<dyn EventPublisher>` — the per-kind event bus for watch notifications
- `meta_validator: Arc<dyn SchemaValidator>` — compiled meta-schema for Schema validation
- `schema_cache: DashMap<String, Arc<dyn SchemaValidator>>` — compiled user schemas keyed by schema name (e.g., `"Widget.example.io"`)

#### Scenario: Service construction
- **WHEN** `ObjectService::new(store, event_bus, meta_validator)` is called with `store: Arc<dyn ObjectStore>`, `event_bus: Arc<dyn EventPublisher>`, and `meta_validator: Arc<dyn SchemaValidator>`
- **THEN** the service is constructed with an empty schema cache

### Requirement: Service publishes events after mutations only
The service SHALL publish events only after successful store operations. If the store returns an error, no event is published.

#### Scenario: Failed create does not publish
- **WHEN** `create` fails due to a duplicate conflict
- **THEN** no `Added` event is published

#### Scenario: Failed update does not publish
- **WHEN** `update` fails due to a version conflict
- **THEN** no `Modified` event is published

## ADDED Requirements

### Requirement: Service provides subscribe() for SSE watch
The system SHALL provide an `ObjectService::subscribe(&self, key: &ResourceKey) -> WatchStream` method that delegates to the internal `EventPublisher::subscribe()`.

#### Scenario: Subscribe returns a WatchStream
- **WHEN** `object_service.subscribe(&key)` is called
- **THEN** a `WatchStream` is returned for the given resource key

### Requirement: Schema compilation uses JsonSchemaValidator
The system SHALL compile user schemas during `create` and `update` operations using `JsonSchemaValidator::compile(&schema_data.json_schema)` instead of calling `draft202012::options().build()` directly.

#### Scenario: Schema compiled via JsonSchemaValidator
- **WHEN** a Schema registration payload passes meta-schema validation
- **THEN** `JsonSchemaValidator::compile()` is called to compile the `jsonSchema` field
- **AND** the resulting `JsonSchemaValidator` is cached as `Arc<dyn SchemaValidator>`

### Requirement: Validation error mapping in object operations
The system SHALL map `SchemaValidationError` from `SchemaValidator::validate()` to the domain `ValidationError` type when validating regular objects, and to `Vec<String>` when validating meta-schema payloads.

#### Scenario: Meta-schema errors mapped to strings
- **WHEN** meta-schema validation fails during Schema create or update
- **THEN** `SchemaValidationError.message` values are collected into `Vec<String>` and returned as `AppError::InvalidSchema`

#### Scenario: Object validation errors mapped to ValidationError
- **WHEN** object validation fails during object create or update
- **THEN** `SchemaValidationError` values are mapped to `object::types::ValidationError { path, message }` and returned as `AppError::SchemaValidation`
