## MODIFIED Requirements

### Requirement: create delegates schema work to SchemaRegistry
The `create(key, meta, spec)` method SHALL:
1. Validate `meta.labels` using label validation rules (key format, value format, length limits)
2. If `key.kind == "Schema"`: call `schema_registry.validate_and_compile(&spec)` to validate and compile, then `store.create()`, then `schema_registry.insert()` to cache, then `event_bus.publish()`
3. If `key.kind != "Schema"`: call `schema_registry.get_validator(&key)` to obtain the validator, validate `spec` against it, then `store.create()`, then `event_bus.publish()`

Label validation SHALL occur after schema validation of the spec payload but before store persistence. If label validation fails, an `AppError::InvalidLabel` error SHALL be returned with a descriptive message.

#### Scenario: Create valid Schema
- **WHEN** a Schema registration passed meta-schema validation and its jsonSchema compiles
- **THEN** the schema is stored with the generated name, the compiled validator is cached under that name, and an `Added` event is published

#### Scenario: Create Schema with invalid meta-schema
- **WHEN** a Schema registration fails meta-schema validation
- **THEN** the error is `InvalidSchema` and nothing is stored or published

#### Scenario: Create Schema with uncompileable jsonSchema
- **WHEN** a Schema registration passes meta-schema validation but `jsonSchema` fails compilation
- **THEN** the error is `InvalidSchema` and nothing is stored or published

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

### Requirement: update delegates schema work to SchemaRegistry
The `update(object)` method SHALL:
1. Validate `object.metadata.labels` using label validation rules
2. If `object.key.kind == "Schema"`: call `schema_registry.validate_and_compile(&spec)`, then `store.update()`, then `schema_registry.insert()`, then `event_bus.publish()`
3. If `object.key.kind != "Schema"`: call `schema_registry.get_validator(&key)`, validate spec, then `store.update()`, then `event_bus.publish()`

Label validation SHALL occur after schema validation but before store persistence. If label validation fails, an `AppError::InvalidLabel` error SHALL be returned and no persistence SHALL occur.

#### Scenario: Update with correct version
- **WHEN** `update` is called with a matching `resourceVersion`
- **THEN** the object is updated, a `Modified` event is published, and the updated object is returned

#### Scenario: Update with wrong version
- **WHEN** `update` is called with a stale `resourceVersion`
- **THEN** the store returns `Conflict` and no event is published