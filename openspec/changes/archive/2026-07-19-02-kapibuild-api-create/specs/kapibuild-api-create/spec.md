## ADDED Requirements

### Requirement: kapibuild api create command
The system SHALL provide a `kapibuild api create` command that generates skeleton Rust structs for a new API resource with the following flags:
- `--group <group>` (required, e.g., example.io)
- `--version <version>` (required, e.g., v1)
- `--kind <kind>` (required, e.g., Widget)
- `--scope <scope>` (optional, Namespaced | Cluster, default: Namespaced)
- `--status` (optional, generate status struct, default: false)

The command SHALL create:
- `src/api/<group>/<version>/<kind>.rs` with skeleton structs (WidgetSpec, optionally WidgetStatus)

#### Scenario: Create API without status
- **WHEN** user runs `kapibuild api create --group example.io --version v1 --kind Widget`
- **THEN** system creates src/api/example.io/v1/widget.rs with WidgetSpec struct

#### Scenario: Create API with status
- **WHEN** user runs `kapibuild api create --group example.io --version v1 --kind Widget --status`
- **THEN** system creates src/api/example.io/v1/widget.rs with both WidgetSpec and WidgetStatus structs

#### Scenario: Create API for existing kind
- **WHEN** user runs `kapibuild api create` for a kind that already exists
- **THEN** system returns an error and does not overwrite existing files

### Requirement: Skeleton struct generation
The system SHALL generate skeleton structs with:
- `#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]`
- Optional status struct if --status flag is provided
- Example fields that users can replace
- Resource metadata (group, version, kind, scope) is stored in the Kapifile, not as struct attributes

#### Scenario: Skeleton struct content
- **WHEN** system generates a skeleton struct
- **THEN** struct has correct derives with example fields; metadata is stored in Kapifile

### Requirement: Kapifile update
The system SHALL update the Kapifile to include the new resource with kind, version, scope, and has_status fields.

#### Scenario: Kapifile after api create
- **WHEN** user runs `kapibuild api create --kind Widget --version v1 --scope Namespaced --status`
- **THEN** system adds Widget resource entry to Kapifile with version, scope, and has_status fields
