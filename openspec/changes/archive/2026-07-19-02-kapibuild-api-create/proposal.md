## Why

Users need a way to create skeleton Rust structs for their API resources. After initializing a project, users define their spec and status fields. This command generates the starting point structs with the correct derives and attributes.

## What Changes

- **`kapibuild api create` command** — generates skeleton Rust structs for a new API resource
  - Creates api/<group>/<version>/<kind>.rs with WidgetSpec (and optionally WidgetStatus)
  - Adds #[derive(KapiResource)] with #[kapi(...)] attributes
  - Updates Kapifile to track the new resource
  - Flags: --group, --version, --kind, --scope, --status

## Capabilities

### New Capabilities
- `02-kapibuild-api-create`: Generate skeleton Rust structs for a new API resource

### Modified Capabilities
(none)

## Impact

- **New command**: `kapibuild api create`
- **Dependencies**: Requires `kapibuild init` to have been run
- **Workflow**: Users run `kapibuild api create` after init, then edit the generated structs

## Non-goals

- Generating final wrapper struct (handled by `kapibuild api generate`)
- Generating JSON schemas (handled by `kapibuild api generate`)
- Controller scaffolding (handled by `kapibuild controller generate`)

## Future Work

- Support for multiple versions of the same API
- Validation of struct field types
