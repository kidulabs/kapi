## MODIFIED Requirements

### Requirement: create validates and stores objects
The `create(key, name, data)` method SHALL:
1. If `key.kind == "Schema"`: validate `data` against `meta_validator`, compile `data.jsonSchema` via `compile_jsonschema()`, cache the compiled validator under the `name` parameter (which is generated as `{targetKind}.{targetGroup}` by the handler)
2. If `key.kind != "Schema"`: look up the Schema from the store via `lookup_object_validator()`, which compiles on cache miss if the schema exists in the store
3. Call `store.create(key, name, data)`
4. Call `event_bus.publish(key, WatchEvent::Added(obj))`
5. Return the created `StoredObject`

#### Scenario: Create valid Schema
- **WHEN** a Schema registration passes meta-schema validation and its jsonSchema compiles
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
