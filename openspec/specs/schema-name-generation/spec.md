# schema-name-generation Specification

## Purpose
Define how Schema `metadata.name` is generated from the registration payload. The name format is `{targetKind}.{targetGroup}.{targetVersion}` to support independent schemas for the same kind across different API versions.

## Requirements
### Requirement: Schema name is generated from targetKind, targetGroup, and targetVersion
When a Schema registration payload is received, the system SHALL generate the schema name as `{targetKind}.{targetGroup}.{targetVersion}` using the values from the request body. The generated name SHALL be used as the storage key and cache key.

#### Scenario: Name generated from valid payload
- **WHEN** a Schema registration payload contains `targetKind: "Widget"`, `targetGroup: "example.io"`, and `targetVersion: "v1"`
- **THEN** the generated name is `"Widget.example.io.v1"`

#### Scenario: Different versions produce distinct names
- **WHEN** two Schemas are registered with `targetKind: "Widget"`, `targetGroup: "example.io"`, and different `targetVersion` values (`"v1"` and `"v2"`)
- **THEN** the generated names are `"Widget.example.io.v1"` and `"Widget.example.io.v2"` respectively

#### Scenario: Name uses exact payload values
- **WHEN** a Schema registration payload contains `targetKind: "Config"`, `targetGroup: "acme.corp"`, and `targetVersion: "v1"`
- **THEN** the generated name is `"Config.acme.corp.v1"`

### Requirement: Missing targetKind, targetGroup, or targetVersion returns InvalidSchema error
If a Schema registration payload is missing `targetKind`, `targetGroup`, or `targetVersion`, or if any of these fields is not a string, the system SHALL return an `InvalidSchema` error before attempting meta-schema validation.

#### Scenario: Missing targetKind
- **WHEN** a Schema registration payload has `targetGroup` and `targetVersion` but no `targetKind`
- **THEN** the response is an `InvalidSchema` error

#### Scenario: Missing targetGroup
- **WHEN** a Schema registration payload has `targetKind` and `targetVersion` but no `targetGroup`
- **THEN** the response is an `InvalidSchema` error

#### Scenario: Missing targetVersion
- **WHEN** a Schema registration payload has `targetKind` and `targetGroup` but no `targetVersion`
- **THEN** the response is an `InvalidSchema` error

#### Scenario: targetKind is not a string
- **WHEN** a Schema registration payload has `targetKind` as a number or object
- **THEN** the response is an `InvalidSchema` error

### Requirement: Schema registration response includes generated name in metadata
After successful Schema registration, the response SHALL include the generated name in `metadata.name` so clients can reference the schema for subsequent operations.

#### Scenario: Response contains generated name
- **WHEN** a Schema is successfully registered
- **THEN** the response `metadata.name` equals `{targetKind}.{targetGroup}.{targetVersion}`

