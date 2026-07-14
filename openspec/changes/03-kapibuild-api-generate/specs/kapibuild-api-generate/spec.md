## ADDED Requirements

### Requirement: kapibuild api generate command
The system SHALL provide a `kapibuild api generate` command that generates the final wrapper struct and JSON schema files from skeleton types.

The command SHALL:
- Scan api/<group>/<version>/<kind>.rs files
- Parse Rust structs with #[derive(KapiResource)]
- Use the kapi-derive proc-macro to generate wrapper struct at compile time
- Generate JSON Schema from the types using schemars
- Extract group/version/kind/scope from #[kapi(...)] attributes
- Write full SchemaData payload to schemas/<group>_<kind>.json (flat directory structure)
- NOT generate any controller code

#### Scenario: Generate schemas for all resources
- **WHEN** user runs `kapibuild api generate`
- **THEN** system regenerates schema files in schemas/ directory for all resources in api/ directory

#### Scenario: Generate schema with validation rules
- **WHEN** user has added schemars validation attributes (e.g., `#[schemars(length(min = 1))]`) to types.rs
- **THEN** system includes validation rules in the generated schema file

#### Scenario: Generate schema with status
- **WHEN** user has defined both WidgetSpec and WidgetStatus in types.rs
- **THEN** system generates schema file with both specSchema and statusSchema fields

#### Scenario: Schema file naming
- **WHEN** user generates schema for Widget with group example.io
- **THEN** system creates schemas/example.io_Widget.json

### Requirement: Schema generation via helper binary
The system SHALL use a helper binary approach for schema generation — generating a small Rust program that imports user types, calls schema_data(), and writes JSON files.

#### Scenario: Helper binary execution
- **WHEN** user runs `kapibuild api generate`
- **THEN** system generates a helper binary, compiles it, runs it to produce schema files, then cleans up

### Requirement: SchemaData format
The system SHALL generate schema files containing the full SchemaData payload:
- `targetGroup` from #[kapi(group = "...")]
- `targetVersion` from #[kapi(version = "...")]
- `targetKind` from #[kapi(kind = "...")]
- `scope` from #[kapi(scope = "...")] (default: "Namespaced")
- `specSchema` from schemars-generated JSON Schema of the spec struct
- `statusSchema` from schemars-generated JSON Schema of the status struct (if status attribute is provided)

#### Scenario: Schema file content
- **WHEN** system generates a schema file
- **THEN** file contains targetGroup, targetVersion, targetKind, scope, specSchema, and optionally statusSchema
