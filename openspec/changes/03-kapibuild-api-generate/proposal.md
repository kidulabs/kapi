## Why

Users need to generate the final form Rust struct (wrapper with metadata/spec/status) and JSON schema files from their skeleton structs. This bridges the gap between user-defined types and the kapi server's schema requirements.

## What Changes

- **`kapibuild api generate` command** — generates final wrapper struct and JSON schema
  - Parses skeleton structs from api/<group>/<version>/<kind>.rs
  - Uses kapi-derive proc-macro to generate wrapper struct at compile time
  - Generates JSON schema files in schemas/<group>_<kind>.json
  - Schema includes specSchema and statusSchema (if status exists)
  - No controller code generated in this phase

## Capabilities

### New Capabilities
- `03-kapibuild-api-generate`: Generate final wrapper struct and JSON schema from skeleton types

### Modified Capabilities
(none)

## Impact

- **New command**: `kapibuild api generate`
- **Dependencies**: Requires kapi-derive proc-macro, requires `kapibuild api create` to have been run
- **Workflow**: Users edit skeleton structs, then run `kapibuild api generate` to produce schemas

## Non-goals

- Controller scaffolding (handled by `kapibuild controller generate`)
- Runtime schema registration
- Schema validation against server

## Future Work

- Support for validation rules (schemars attributes)
- Incremental schema generation (only regenerate changed schemas)
- Schema diffing and migration
