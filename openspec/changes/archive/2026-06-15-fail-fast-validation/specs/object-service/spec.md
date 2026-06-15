## MODIFIED Requirements

### Requirement: create delegates schema work to SchemaRegistry and sets metadata
The `create(key, meta, spec)` method SHALL:
1. Validate `meta.labels` using label validation rules (key format, value format, length limits)
2. Validate `meta.annotations` using annotation validation rules (key format, total size limit)
3. If `key.kind == "Schema"`: call `schema_registry.validate_and_compile(&spec)` to validate and compile, then construct a `StoredObject` with `system.resource_version = 1`, `system.generation = 1`, `system.created_at = Utc::now()`, `system.updated_at = Utc::now()`, then `store.create()`, then `schema_registry.insert()` to cache, then `event_bus.publish()`
4. If `key.kind != "Schema"`: call `schema_registry.get_validator(&key)` to obtain the validator, validate `spec` against it, then construct a `StoredObject` with initial metadata, then `store.create()`, then `event_bus.publish()`

The service SHALL construct the complete `StoredObject` with all system metadata before calling `store.create()`. The store SHALL persist the object as-is.

The service SHALL set `StoredObject.spec` to the `serde_json::Value` directly. There SHALL be no `SpecData { value: ... }` construction anywhere in the service.

Label and annotation validation SHALL occur as defense-in-depth: the handler validates eagerly before calling the service, and the service re-validates to ensure non-HTTP callers (tests, future gRPC/CLI) receive the same validation guarantees. If validation fails, an `AppError::InvalidLabel` or `AppError::InvalidAnnotation` error SHALL be returned with a descriptive message.

#### Scenario: Create valid Schema
- **WHEN** a Schema registration passed meta-schema validation and its jsonSchema compiles
- **THEN** the service SHALL construct a `StoredObject` with `resource_version = 1`, `generation = 1`, and current timestamps, with `spec` set to the validated `Value` directly
- **AND** the schema is stored with the generated name, the compiled validator is cached under that name, and an `Added` event is published

#### Scenario: Create Schema with invalid meta-schema
- **WHEN** a Schema registration fails meta-schema validation
- **THEN** the error is `InvalidSchema` and nothing is stored or published

#### Scenario: Create Schema with uncompileable jsonSchema
- **WHEN** a Schema registration passes meta-schema validation but `jsonSchema` fails compilation
- **THEN** the error is `InvalidSchema` and nothing is stored or published

#### Scenario: Create object with initial metadata
- **WHEN** creating a regular object
- **THEN** the service SHALL set `system.resource_version = 1`, `system.generation = 1`, `system.created_at = Utc::now()`, `system.updated_at = Utc::now()`, with `spec` set to the validated `Value` directly
- **AND** the store SHALL persist the object with those exact metadata values

#### Scenario: Create object for unregistered kind
- **WHEN** creating an object for a kind with no registered Schema
- **THEN** the error is `NotFound` (no schema found for this kind)

#### Scenario: Create object with invalid spec
- **WHEN** creating an object whose spec fails schema validation
- **THEN** the error is `SchemaValidation` with the list of validation errors

#### Scenario: Create duplicate object
- **WHEN** creating an object with a name that already exists
- **THEN** the store returns `AlreadyExists` and no event is published

#### Scenario: Create object with schema not in cache but in store
- **WHEN** creating an object for a kind whose Schema exists in the store but is not in the cache
- **THEN** `schema_registry.get_validator()` fetches the schema from the store, compiles it on-demand, caches it, and the object is validated against it

#### Scenario: Create object with stored schema that fails compilation
- **WHEN** creating an object for a kind whose Schema exists in the store but whose `jsonSchema` fails compilation
- **THEN** `schema_registry.get_validator()` returns `AppError::StoredSchemaCompilationFailed`
- **AND** no object is created

#### Scenario: Create with valid labels
- **WHEN** `create()` is called with valid labels
- **THEN** validation SHALL pass and the object SHALL be persisted with labels

#### Scenario: Create with invalid label key
- **WHEN** `create()` is called with a label key that violates format rules
- **THEN** an `AppError::InvalidLabel` error SHALL be returned with a descriptive message

#### Scenario: Create with invalid label value
- **WHEN** `create()` is called with a label value that violates format rules
- **THEN** an `AppError::InvalidLabel` error SHALL be returned with a descriptive message

#### Scenario: Create with empty labels map
- **WHEN** `create()` is called with an empty labels `HashMap`
- **THEN** validation SHALL pass and the object SHALL be persisted

#### Scenario: Create with invalid annotations
- **WHEN** `create()` is called with annotations that violate format rules (empty key, key exceeding 256 chars, or total size exceeding 256KB)
- **THEN** an `AppError::InvalidAnnotation` error SHALL be returned with a descriptive message

### Requirement: update delegates schema work to SchemaRegistry and uses centralized metadata
The `update(object)` method SHALL:
1. Validate `object.metadata.labels` using label validation rules
2. Validate `object.metadata.annotations` using annotation validation rules
3. If `object.key.kind == "Schema"`: call `schema_registry.validate_and_compile(&spec)`, then use `store.transaction()` with a callback that performs OCC check and returns `TransactionOp::Apply` with updated metadata, then `schema_registry.insert()`, then `event_bus.publish()`
4. If `object.key.kind != "Schema"`: call `schema_registry.get_validator(&key)`, validate spec, then use `store.transaction()` with a callback that performs OCC check and returns `TransactionOp::Apply` with updated metadata, then `event_bus.publish()`

The service SHALL use a centralized metadata wrapper that automatically handles:
- `resource_version = existing.resource_version + 1`
- `generation = existing.generation + 1` if `existing.spec != new_obj.spec` (direct `Value` equality), else `existing.generation`
- `updated_at = Utc::now()`
- `created_at = existing.created_at` (preserved)

The service SHALL perform OCC (optimistic concurrency control) check inside the transaction callback: if `object.system.resource_version != existing.system.resource_version`, return `TransactionOp::Abort(AppError::Conflict)`.

Label and annotation validation SHALL occur as defense-in-depth: the handler validates eagerly before calling the service, and the service re-validates to ensure non-HTTP callers receive the same validation guarantees. If validation fails, an `AppError::InvalidLabel` or `AppError::InvalidAnnotation` error SHALL be returned and no persistence SHALL occur.

#### Scenario: Update object with schema not in cache but in store
- **WHEN** updating an object for a kind whose Schema exists in the store but is not in the cache
- **THEN** `schema_registry.get_validator()` compiles the schema on-demand, caches it, and the object is validated against it

#### Scenario: Update with correct version
- **WHEN** `update` is called with a matching `resourceVersion`
- **THEN** the service SHALL increment `resource_version`, preserve `created_at`, update `updated_at`, bump `generation` if spec changed, and publish a `Modified` event

#### Scenario: Update with wrong version
- **WHEN** `update` is called with a stale `resourceVersion`
- **THEN** the transaction callback SHALL return `TransactionOp::Abort(AppError::Conflict)` and no event is published

#### Scenario: Update with same spec does not bump generation
- **WHEN** `update` is called with the same `spec` (direct `Value` equality) but different `metadata.labels`
- **THEN** the centralized metadata wrapper SHALL preserve `generation` (not increment it)

#### Scenario: Update with different spec bumps generation
- **WHEN** `update` is called with a different `spec` `Value`
- **THEN** the centralized metadata wrapper SHALL increment `generation` by 1

#### Scenario: Update with valid labels
- **WHEN** `update()` is called with valid labels
- **THEN** validation SHALL pass and the object SHALL be persisted with updated labels

#### Scenario: Update with invalid labels
- **WHEN** `update()` is called with invalid labels
- **THEN** an `AppError::InvalidLabel` error SHALL be returned and no persistence SHALL occur

#### Scenario: Update with invalid annotations
- **WHEN** `update()` is called with annotations that violate format rules
- **THEN** an `AppError::InvalidAnnotation` error SHALL be returned and no persistence SHALL occur
