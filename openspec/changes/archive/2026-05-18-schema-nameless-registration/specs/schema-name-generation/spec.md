## ADDED Requirements

### Requirement: Schema name is generated from targetKind and targetGroup
When a Schema registration payload is received, the system SHALL generate the schema name as `{targetKind}.{targetGroup}` using the values from the request body. The generated name SHALL be used as the storage key and cache key.

#### Scenario: Name generated from valid payload
- **WHEN** a Schema registration payload contains `targetKind: "Widget"` and `targetGroup: "example.io"`
- **THEN** the generated name is `"Widget.example.io"`

#### Scenario: Name uses exact payload values
- **WHEN** a Schema registration payload contains `targetKind: "Config"` and `targetGroup: "acme.corp"`
- **THEN** the generated name is `"Config.acme.corp"`

### Requirement: Missing targetKind or targetGroup returns InvalidSchema error
If a Schema registration payload is missing `targetKind` or `targetGroup`, or if either field is not a string, the system SHALL return an `InvalidSchema` error before attempting meta-schema validation.

#### Scenario: Missing targetKind
- **WHEN** a Schema registration payload has `targetGroup` but no `targetKind`
- **THEN** the response is an `InvalidSchema` error

#### Scenario: Missing targetGroup
- **WHEN** a Schema registration payload has `targetKind` but no `targetGroup`
- **THEN** the response is an `InvalidSchema` error

#### Scenario: targetKind is not a string
- **WHEN** a Schema registration payload has `targetKind` as a number or object
- **THEN** the response is an `InvalidSchema` error

### Requirement: Schema registration response includes generated name in metadata
After successful Schema registration, the response SHALL include the generated name in `metadata.name` so clients can reference the schema for subsequent operations.

#### Scenario: Response contains generated name
- **WHEN** a Schema is successfully registered
- **THEN** the response `metadata.name` equals `{targetKind}.{targetGroup}`
