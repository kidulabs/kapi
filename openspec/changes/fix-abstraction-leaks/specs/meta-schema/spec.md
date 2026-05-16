## ADDED Requirements

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

## MODIFIED Requirements

### Requirement: Meta-schema compilation returns a SchemaValidator
The system SHALL provide a `compile_meta_schema()` function that parses `META_SCHEMA_JSON` and returns a `JsonSchemaValidator`. The function SHALL use `draft202012::options()` to build the validator internally.

#### Scenario: Compilation succeeds
- **WHEN** `compile_meta_schema()` is called
- **THEN** it returns `Ok(JsonSchemaValidator)` that implements `SchemaValidator`

#### Scenario: Validator rejects invalid payloads
- **WHEN** the compiled validator validates a payload missing `targetGroup`
- **THEN** `is_valid()` returns `false` and `validate()` yields at least one `SchemaValidationError`
