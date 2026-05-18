## MODIFIED Requirements

### Requirement: create validates and stores objects
The `create(key, name, data)` method SHALL:
1. If `key.kind == "Schema"`: validate `data` against `meta_validator`, compile `data.jsonSchema` via `validator_for()`, cache the compiled validator under the `name` parameter (which is generated as `{targetKind}.{targetGroup}` by the handler)
2. If `key.kind != "Schema"`: look up the Schema from the store, validate `data` against the cached compiled schema
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
- **THEN** the store returns `Conflict` and no event is published
