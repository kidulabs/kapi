## MODIFIED Requirements

### Requirement: create validates and stores objects
The `create(key, meta, data)` method SHALL:
1. If `key.kind == "Schema"`: validate `data` against `meta_validator`, compile `data.jsonSchema` via `compile_jsonschema()`, cache the compiled validator under the name from `meta.name` (which is generated as `{targetKind}.{targetGroup}` by the handler)
2. If `key.kind != "Schema"`: look up the Schema from the store via `lookup_object_validator()`, which compiles on cache miss if the schema exists in the store
3. Validate `meta.labels` using label validation rules (key format, value format, length limits)
4. Call `store.create(key, meta, data)`
5. Call `event_bus.publish(key, WatchEvent::Added(obj))`
6. Return the created `StoredObject`

Label validation SHALL occur after schema validation of the data payload but before store persistence. If label validation fails, an `AppError::InvalidLabel` error SHALL be returned with a descriptive message.

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

#### Scenario: Create object with invalid data
- **WHEN** creating an object whose data fails schema validation
- **THEN** the error is `SchemaValidation` with the list of validation errors

#### Scenario: Create duplicate object
- **WHEN** creating an object with a name that already exists
- **THEN** the store returns `AlreadyExists` and no event is published

#### Scenario: Create object with schema not in cache but in store
- **WHEN** creating an object for a kind whose Schema exists in the store but is not in the cache
- **THEN** the schema is compiled on-demand, cached, and the object is validated against it

#### Scenario: Create object with stored schema that fails compilation
- **WHEN** creating an object for a kind whose Schema exists in the store but whose `jsonSchema` fails compilation
- **THEN** `lookup_object_validator()` returns `AppError::StoredSchemaCompilationFailed`
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

### Requirement: update validates and stores objects
The `update(object)` method SHALL:
1. Determine if the object is a Schema or regular object based on `object.key.kind`
2. Apply the same validation flow as `create` (meta-schema for Schema, compiled schema for objects)
3. Validate `object.metadata.labels` using the same label validation rules as create
4. Call `store.update(object)`
5. Call `event_bus.publish(key, WatchEvent::Modified(obj))`
6. Return the updated `StoredObject`

Label validation SHALL occur after schema validation but before store persistence. If label validation fails, an `AppError::InvalidLabel` error SHALL be returned and no persistence SHALL occur.

#### Scenario: Update object with schema not in cache but in store
- **WHEN** updating an object for a kind whose Schema exists in the store but is not in the cache
- **THEN** `lookup_object_validator()` compiles the schema on-demand, caches it, and the object is validated against it

#### Scenario: Update with correct version
- **WHEN** `update` is called with a matching `resourceVersion`
- **THEN** the object is updated, a `Modified` event is published, and the updated object is returned

#### Scenario: Update with wrong version
- **WHEN** `update` is called with a stale `resourceVersion`
- **THEN** the store returns `Conflict` and no event is published

#### Scenario: Update with valid labels
- **WHEN** `update()` is called with valid labels
- **THEN** validation SHALL pass and the object SHALL be persisted with updated labels

#### Scenario: Update with invalid labels
- **WHEN** `update()` is called with invalid labels
- **THEN** an `AppError::InvalidLabel` error SHALL be returned and no persistence SHALL occur

### Requirement: Validation error mapping in object operations
The system SHALL map `SchemaValidationError` from `SchemaValidator::validate()` to the domain `ValidationError` type when validating regular objects, and to `Vec<String>` when validating meta-schema payloads. A `validate_labels()` function SHALL validate a `HashMap<String, String>` against label validation rules, returning `Result<(), AppError>` with descriptive error messages identifying the offending key or value.

#### Scenario: Meta-schema errors mapped to strings
- **WHEN** meta-schema validation fails during Schema create or update
- **THEN** `SchemaValidationError.message` values are collected into `Vec<String>` and returned as `AppError::InvalidSchema`

#### Scenario: Object validation errors mapped to ValidationError
- **WHEN** object validation fails during object create or update
- **THEN** `SchemaValidationError` values are mapped to `object::types::ValidationError { path, message }` and returned as `AppError::SchemaValidation`

#### Scenario: Validate key with prefix
- **WHEN** `validate_labels()` is called with key `app.example.io/name`
- **THEN** validation SHALL check prefix format (DNS subdomain, max 253 chars) and name format (max 256 chars, valid characters)

#### Scenario: Validate empty value
- **WHEN** `validate_labels()` is called with a label whose value is an empty string
- **THEN** validation SHALL pass (empty values are allowed)
