## Purpose

Define the meta-schema constant and compilation function that validates Schema registration payloads. The meta-schema ensures that registered schemas have the required structural fields before attempting to compile the nested JSON Schema.

## ADDED Requirements

### Requirement: Meta-schema JSON constant defines valid Schema registration shape
The system SHALL define a hardcoded JSON Schema constant (`META_SCHEMA_JSON`) using Draft 2020-12 that requires four fields: `targetGroup` (string, non-empty), `targetVersion` (string, non-empty), `targetKind` (string, non-empty), and `jsonSchema` (object). The meta-schema SHALL reject unknown fields via `unevaluatedProperties: false`.

#### Scenario: Valid Schema registration passes meta-schema
- **WHEN** a payload has all four required fields with correct types
- **THEN** meta-schema validation succeeds

#### Scenario: Missing field fails meta-schema
- **WHEN** a payload is missing `targetGroup`, `targetVersion`, `targetKind`, or `jsonSchema`
- **THEN** meta-schema validation fails with a path-specific error

#### Scenario: Unknown field fails meta-schema
- **WHEN** a payload contains fields beyond the four required ones
- **THEN** meta-schema validation fails due to `unevaluatedProperties: false`

#### Scenario: Wrong type fails meta-schema
- **WHEN** `jsonSchema` is not an object (e.g., a string or array)
- **THEN** meta-schema validation fails

### Requirement: Meta-schema compilation returns a Validator
The system SHALL provide a `compile_meta_schema()` function that parses `META_SCHEMA_JSON` and returns a `jsonschema::Validator`. The function SHALL use `draft202012::options()` to build the validator.

#### Scenario: Compilation succeeds
- **WHEN** `compile_meta_schema()` is called
- **THEN** it returns `Ok(Validator)` that can validate Schema registration payloads

#### Scenario: Validator rejects invalid payloads
- **WHEN** the compiled validator validates a payload missing `targetGroup`
- **THEN** `is_valid()` returns `false` and `iter_errors()` yields at least one error

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
