## Why

Users need a way to initialize a new kapi controller project with the correct directory structure and dependencies. Without scaffolding, users must manually set up Cargo.toml, directory layout, and boilerplate code.

## What Changes

- **`kapibuild init` command** — scaffolds a new kapi controller project
  - Creates Cargo.toml with dependencies (kapi-core, kapi-client, kapi-controller, kapi-derive)
  - Creates directory structure: api/, schemas/, src/
  - Creates src/main.rs with Manager setup
  - Creates Kapifile manifest for tracking resources

## Capabilities

### New Capabilities
- `01-kapibuild-init`: Initialize a new kapi controller project with standard structure

### Modified Capabilities
(none)

## Impact

- **New command**: `kapibuild init`
- **Dependencies**: Uses templates for project structure
- **Workflow**: Users run `kapibuild init` to start a new project

## Non-goals

- Creating API types (handled by `kapibuild api create`)
- Generating schemas (handled by `kapibuild api generate`)
- Controller scaffolding (handled by `kapibuild controller generate`)

## Future Work

- Support for custom templates
- Interactive prompts for project name and domain
