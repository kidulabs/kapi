## ADDED Requirements

### Requirement: InvalidSchema error for broken schema registrations
The system SHALL produce `InvalidSchema` errors when a schema registration fails meta-schema validation or when the nested `jsonSchema` fails compilation.

#### Scenario: Meta-schema validation failure
- **WHEN** a Schema registration is missing required fields (targetGroup, targetVersion, targetKind, jsonSchema)
- **THEN** the error SHALL be `InvalidSchema` with a description of the validation failure

#### Scenario: Nested jsonSchema compilation failure
- **WHEN** a Schema registration contains a `jsonSchema` that cannot be compiled as a valid JSON Schema
- **THEN** the error SHALL be `InvalidSchema` with the compilation error message

### Requirement: InvalidSchema maps to HTTP 422
The system SHALL map `InvalidSchema` to HTTP 422 Unprocessable Entity with JSON body `{ "error": "...", "code": "InvalidSchema", "details": { "message": "..." } }`.

#### Scenario: InvalidSchema response body
- **WHEN** a handler returns `InvalidSchema("missing field: targetGroup")`
- **THEN** the response is HTTP 422 with JSON body containing `"code": "InvalidSchema"` and `"details": { "message": "missing field: targetGroup" }`

### Requirement: SchemaHasObjects error blocks schema deletion
The system SHALL produce `SchemaHasObjects` errors when attempting to delete a Schema that has existing objects of the target kind.

#### Scenario: Delete schema with existing objects
- **WHEN** a DELETE request targets a Schema and objects of the target kind exist
- **THEN** the error SHALL be `SchemaHasObjects { kind: "...", count: N }`

### Requirement: SchemaHasObjects maps to HTTP 409
The system SHALL map `SchemaHasObjects` to HTTP 409 Conflict with JSON body `{ "error": "...", "code": "SchemaHasObjects", "details": { "kind": "...", "count": N } }`.

#### Scenario: SchemaHasObjects response body
- **WHEN** a handler returns `SchemaHasObjects { kind: "Widget".into(), count: 5 }`
- **THEN** the response is HTTP 409 with JSON body containing `"code": "SchemaHasObjects"` and `"details": { "kind": "Widget", "count": 5 }`
