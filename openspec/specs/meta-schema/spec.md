## Purpose

Define the meta-schema constant and compilation function that validates Schema registration payloads. The meta-schema ensures that registered schemas have the required structural fields before attempting to compile the nested JSON Schema.

## Requirements

### Requirement: Meta-schema JSON constant defines valid Schema registration shape
The system SHALL define a hardcoded JSON Schema constant (`META_SCHEMA_JSON`) using Draft 2020-12 that requires four fields: `targetGroup` (string, non-empty), `targetVersion` (string, non-empty), `targetKind` (string, non-empty), and `jsonSchema` (object), plus an optional `statusSchema` property of type `"object"`. The meta-schema SHALL reject unknown fields via `unevaluatedProperties: false`, so only `targetGroup`, `targetVersion`, `targetKind`, `jsonSchema`, and `statusSchema` are allowed.

#### Scenario: Valid Schema registration passes meta-schema
- **WHEN** a payload has all required fields with correct types
- **THEN** meta-schema validation succeeds

#### Scenario: Missing field fails meta-schema
- **WHEN** a payload is missing `targetGroup`, `targetVersion`, `targetKind`, or `jsonSchema`
- **THEN** meta-schema validation fails with a path-specific error

#### Scenario: Unknown field (other than statusSchema) fails meta-schema
- **WHEN** a payload contains fields beyond the five allowed ones (four required + optional `statusSchema`)
- **THEN** meta-schema validation fails due to `unevaluatedProperties: false`

#### Scenario: Wrong type fails meta-schema
- **WHEN** `jsonSchema` is not an object (e.g., a string or array)
- **THEN** meta-schema validation fails

#### Scenario: Schema registration with statusSchema passes meta-schema validation
- **WHEN** a Schema registration payload includes `statusSchema` as a valid JSON Schema object
- **THEN** meta-schema validation passes

#### Scenario: Schema registration without statusSchema passes meta-schema validation
- **WHEN** a Schema registration payload does not include `statusSchema`
- **THEN** meta-schema validation passes (it is optional)

#### Scenario: Schema registration with invalid statusSchema type fails
- **WHEN** a Schema registration payload includes `statusSchema` as a non-object type (e.g., string)
- **THEN** meta-schema validation fails

### Requirement: Meta-schema compilation returns a JsonSchemaValidator (implements SchemaValidator)
The system SHALL provide a `compile_meta_schema()` function that parses `META_SCHEMA_JSON` and returns a `JsonSchemaValidator`. The function SHALL use `draft202012::options()` to build the validator internally.

#### Scenario: Compilation succeeds
- **WHEN** `compile_meta_schema()` is called
- **THEN** it returns `Ok(JsonSchemaValidator)` that implements `SchemaValidator`

#### Scenario: Validator rejects invalid payloads
- **WHEN** the compiled validator validates a payload missing `targetGroup`
- **THEN** `is_valid()` returns `false` and `validate()` yields at least one `SchemaValidationError`

### Requirement: SchemaValidator trait isolates jsonschema dependency
The system SHALL define a `SchemaValidator` trait in `src/schema/meta_schema.rs` with methods `is_valid(&self, instance: &Value) -> bool` and `validate(&self, instance: &Value) -> Vec<SchemaValidationError>`. The trait SHALL require `Send + Sync`.

#### Scenario: Trait is object-safe
- **WHEN** a type implements `SchemaValidator`
- **THEN** it can be used as `dyn SchemaValidator` inside `Arc`

### Requirement: SchemaValidationError carries structured validation failures
The system SHALL define a `SchemaValidationError` struct in `src/schema/meta_schema.rs` with fields `instance_path: String` and `message: String`.

#### Scenario: Error carries path and message
- **WHEN** a `JsonSchemaValidator` reports validation failures
- **THEN** each failure is mapped to `SchemaValidationError` with the instance path and error message

### Requirement: JsonSchemaValidator wraps jsonschema::Validator
The system SHALL define a `JsonSchemaValidator` struct in `src/schema/meta_schema.rs` that wraps a `jsonschema::Validator` and implements `SchemaValidator`. It SHALL provide a `compile(schema_json: &Value) -> Result<Self, anyhow::Error>` associated function that delegates to `draft202012::options().build()`.

#### Scenario: JsonSchemaValidator compiles a valid schema
- **WHEN** `JsonSchemaValidator::compile(&json_schema_value)` is called with a valid JSON Schema
- **THEN** it returns `Ok(JsonSchemaValidator)` that can validate instances

#### Scenario: JsonSchemaValidator rejects invalid schema
- **WHEN** `JsonSchemaValidator::compile(&json_schema_value)` is called with an invalid JSON Schema
- **THEN** it returns `Err` with a compilation error message

### Requirement: Schema types re-exported from schema module
The system SHALL re-export `SchemaValidator`, `SchemaValidationError`, and `JsonSchemaValidator` from `src/schema/mod.rs` alongside the existing `compile_meta_schema` and `META_SCHEMA_JSON`.

#### Scenario: Types are importable from crate::schema
- **WHEN** a module imports `use crate::schema::{SchemaValidator, JsonSchemaValidator}`
- **THEN** both types are in scope

### Requirement: Meta-schema validates envelope only
The meta-schema SHALL NOT validate the contents of the `jsonSchema` field beyond checking it is a JSON object. Schema content validation is the responsibility of `jsonschema::compile()`.

#### Scenario: Invalid jsonSchema content passes meta-schema
- **WHEN** `jsonSchema` is `{"type": "not-a-real-type"}` (invalid JSON Schema)
- **THEN** meta-schema validation succeeds (it's still a valid object)
- **AND** `jsonschema::compile()` will fail separately

### Requirement: Meta-schema module location
The meta-schema SHALL be defined in `src/schema/meta_schema.rs` and exported via `src/schema/mod.rs`.

#### Scenario: Module compiles
- **WHEN** the project is built
- **THEN** `src/schema/meta_schema.rs` is compiled and exported through `src/schema/mod.rs`
