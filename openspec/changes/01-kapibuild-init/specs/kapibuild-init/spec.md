## ADDED Requirements

### Requirement: kapibuild init command
The system SHALL provide a `kapibuild init` command that scaffolds a new kapi controller project with the following structure:
- `Cargo.toml` with dependencies on kapi-core, kapi-client, kapi-controller, kapi-derive, serde, tokio, tracing, async-trait, schemars
- `Kapifile` manifest with domain configuration
- `src/main.rs` with Manager setup and controller wiring placeholder
- `api/` directory for resource types
- `schemas/` directory for generated schema files
- `src/controllers/` directory for reconciler implementations

#### Scenario: Initialize new project
- **WHEN** user runs `kapibuild init` in an empty directory
- **THEN** system creates Cargo.toml, Kapifile, src/main.rs, api/, schemas/, and src/controllers/ with valid scaffolding

#### Scenario: Initialize in non-empty directory
- **WHEN** user runs `kapibuild init` in a directory with existing files
- **THEN** system prompts for confirmation before overwriting existing files

### Requirement: Cargo.toml dependencies
The system SHALL generate `Cargo.toml` with the following dependencies:
- `kapi-controller` — for Reconciler trait and Manager
- `kapi-client` — for KapiClient
- `kapi-core` — for ResourceKey and other core types
- `kapi-derive` — for KapiResource derive macro
- `serde` with derive feature
- `serde_json`
- `tokio` with full feature
- `tracing`
- `tracing-subscriber`
- `async-trait`
- `schemars`

#### Scenario: Cargo.toml after init
- **WHEN** user runs `kapibuild init`
- **THEN** system creates Cargo.toml with all required dependencies

### Requirement: Kapifile manifest format
The system SHALL use a `Kapifile` manifest at the project root with the following format:
```yaml
domain: <domain>
resources: []
```

The Kapifile SHALL be updated by `kapibuild api create` to include new resources.

#### Scenario: Kapifile after init
- **WHEN** user runs `kapibuild init`
- **THEN** system creates Kapifile with empty resources list

### Requirement: Main.rs structure
The system SHALL generate `src/main.rs` with:
- Tokio runtime setup
- KapiClient initialization
- Manager creation
- Placeholder for controller wiring
- Manager start

#### Scenario: Main.rs after init
- **WHEN** user runs `kapibuild init`
- **THEN** system creates src/main.rs with Manager setup and placeholder for controller wiring
