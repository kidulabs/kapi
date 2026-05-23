## Purpose

Define the `AlreadyExists` error variant for duplicate resource creation scenarios, providing clients with clear, structured information about what resource already exists.

## Requirements

### Requirement: AlreadyExists error represents duplicate resource creation
The system SHALL produce `AlreadyExists { kind: String, name: String }` errors when a `create` operation targets a resource name that already exists within the same scope. The `kind` field SHALL contain the resource kind (e.g., "Widget", "Schema"), and the `name` field SHALL contain the resource name.

#### Scenario: Duplicate object creation
- **WHEN** creating an object with a name that already exists
- **THEN** the error SHALL be `AlreadyExists { kind: "Widget", name: "my-widget" }`

#### Scenario: Duplicate schema creation
- **WHEN** creating a Schema with a name that already exists
- **THEN** the error SHALL be `AlreadyExists { kind: "Schema", name: "Widget.example.io" }`

### Requirement: AlreadyExists maps to HTTP 409
The system SHALL map `AlreadyExists` to HTTP 409 Conflict with JSON body `{ "error": "...", "code": "AlreadyExists", "details": { "kind": "...", "name": "..." } }`.

#### Scenario: AlreadyExists response body
- **WHEN** a handler returns `AlreadyExists { kind: "Widget".into(), name: "my-widget".into() }`
- **THEN** the response is HTTP 409 with JSON body containing `"code": "AlreadyExists"` and `"details": { "kind": "Widget", "name": "my-widget" }`

### Requirement: AlreadyExists Display produces human-readable message
The `Display` implementation for `AlreadyExists` SHALL produce a message in the format `"{kind} '{name}' already exists"`.

#### Scenario: Display format for AlreadyExists
- **WHEN** formatting `AlreadyExists { kind: "Widget", name: "my-widget" }`
- **THEN** the string output is `"Widget 'my-widget' already exists"`
