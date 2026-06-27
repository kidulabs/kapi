## MODIFIED Requirements

### Requirement: Schema name is generated from targetKind, targetGroup, and targetVersion
When a Schema registration payload is received, the system SHALL generate the schema name as `{targetKind}.{targetGroup}.{targetVersion}` using the values from the request body. The generated name SHALL be used as the storage key and cache key.

#### Scenario: Name generated from valid payload
- **WHEN** a Schema registration payload contains `targetKind: "Widget"`, `targetGroup: "example.io"`, and `targetVersion: "v1"`
- **THEN** the generated name is `"Widget.example.io.v1"`

#### Scenario: Name uses exact payload values
- **WHEN** a Schema registration payload contains `targetKind: "Config"`, `targetGroup: "acme.corp"`, and `targetVersion: "v2beta1"`
- **THEN** the generated name is `"Config.acme.corp.v2beta1"`

#### Scenario: Two versions of the same kind produce distinct names
- **WHEN** two Schema registration payloads are submitted with the same `targetKind: "Widget"` and `targetGroup: "example.io"` but different `targetVersion` (`"v1"` and `"v2"`)
- **THEN** the generated names are `"Widget.example.io.v1"` and `"Widget.example.io.v2"` respectively
- **AND** the store accepts both as distinct objects (no `AlreadyExists` collision)

### Requirement: Schema registration response includes generated name in metadata
After successful Schema registration, the response SHALL include the generated name in `metadata.name` so clients can reference the schema for subsequent operations.

#### Scenario: Response contains generated name
- **WHEN** a Schema is successfully registered with `targetKind: "Widget"`, `targetGroup: "example.io"`, `targetVersion: "v1"`
- **THEN** the response `metadata.name` equals `"Widget.example.io.v1"`
