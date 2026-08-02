# Delta Spec: kapibuild-api-generate

## Modified Requirements

### Requirement: Schema generation via helper binary (MODIFIED)

The system SHALL use a helper binary approach for schema generation — generating a small Rust program that imports user types, calls schema_data(), and writes JSON files.

The helper binary SHALL NOT depend on `kapi-core`. The helper binary's dependencies SHALL be limited to `schemars`, `serde`, and `serde_json`.

#### Scenario: Helper binary execution (unchanged)
- **WHEN** user runs `kapibuild api generate`
- **THEN** system generates a helper binary, compiles it, runs it to produce schema files, then cleans up

#### Scenario: Helper binary independence (NEW)
- **WHEN** kapibuild is installed from crates.io (no workspace context)
- **THEN** system generates a helper binary that compiles and runs without requiring `kapi-core` to be discoverable on the filesystem

### Requirement: Helper binary generated code (MODIFIED)

The helper binary's generated wrapper struct SHALL contain only:
- `spec` field (required)
- `status` field (optional, if resource has status)
- `schema_data()` method that returns the SchemaData JSON payload

The helper binary's generated wrapper struct SHALL NOT contain:
- `metadata: ObjectMeta` field
- `key()` method returning `ResourceKey`

The helper binary's generated code SHALL NOT import `kapi_core` types.

## Unchanged Requirements

The following requirements from the base spec are unchanged:
- kapibuild api generate command (scanning, parsing, output format)
- SchemaData format (targetGroup, targetVersion, targetKind, scope, specSchema, statusSchema)
- Schema file naming convention
