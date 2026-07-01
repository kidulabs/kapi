### Requirement: Workspace structure with symmetric members
The project SHALL be organized as a Cargo workspace with the following members: `kapi-core`, `kapi-server`, `kapi-server/tests`, `kapi-client`, `kapi-cli`, and `kapi-controller`. The root `Cargo.toml` SHALL contain only `[workspace]` configuration with no `[package]` or `[dependencies]` sections.

#### Scenario: Workspace has six members
- **WHEN** inspecting the root `Cargo.toml`
- **THEN** the `workspace.members` array SHALL contain exactly `["kapi-core", "kapi-server", "kapi-server/tests", "kapi-client", "kapi-cli", "kapi-controller"]`

#### Scenario: Root Cargo.toml is pure workspace manifest
- **WHEN** inspecting the root `Cargo.toml`
- **THEN** it SHALL NOT contain a `[package]` section
- **AND** it SHALL NOT contain a `[dependencies]` section

### Requirement: Server crate relocated to kapi-server subdirectory
The server implementation SHALL be located in `kapi-server/src/` with its own `Cargo.toml`. The `kapi-server` crate SHALL contain all server code previously in the root `src/` directory.

#### Scenario: Server source files in kapi-server
- **WHEN** inspecting the project structure
- **THEN** `kapi-server/src/main.rs` SHALL exist
- **AND** `kapi-server/src/lib.rs` SHALL exist
- **AND** `kapi-server/src/object/`, `kapi-server/src/store/`, `kapi-server/src/event/`, `kapi-server/src/schema/` directories SHALL exist

#### Scenario: Root src directory removed
- **WHEN** inspecting the project structure
- **THEN** the root `src/` directory SHALL NOT exist

### Requirement: Placeholder crates with minimal structure
The `kapi-client`, `kapi-cli`, and `kapi-controller` crates SHALL exist as workspace members with minimal structure. Each SHALL have a `Cargo.toml` with name, version, edition, and path dependencies. Each SHALL have a stub source file (`lib.rs` for libraries, `main.rs` for binaries).

#### Scenario: kapi-client placeholder exists
- **WHEN** inspecting `kapi-client/`
- **THEN** `kapi-client/Cargo.toml` SHALL exist with `name = "kapi-client"`
- **AND** `kapi-client/src/lib.rs` SHALL exist (may be empty or contain a comment)

#### Scenario: kapi-cli placeholder exists
- **WHEN** inspecting `kapi-cli/`
- **THEN** `kapi-cli/Cargo.toml` SHALL exist with `name = "kapi-cli"`
- **AND** `kapi-cli/src/main.rs` SHALL exist (may be empty or contain a stub `main` function)

#### Scenario: kapi-controller placeholder exists
- **WHEN** inspecting `kapi-controller/`
- **THEN** `kapi-controller/Cargo.toml` SHALL exist with `name = "kapi-controller"`
- **AND** `kapi-controller/src/lib.rs` SHALL exist (may be empty or contain a comment)

### Requirement: Test crate relocated to kapi-server/tests
The integration tests SHALL be located in `kapi-server/tests/` with its own `Cargo.toml`. The test crate SHALL depend on `kapi-server` instead of the root `kapi` package. All integration tests SHALL continue to function.

#### Scenario: Test crate in kapi-server/tests
- **WHEN** inspecting the project structure
- **THEN** `kapi-server/tests/Cargo.toml` SHALL exist
- **AND** `kapi-server/tests/src/` SHALL exist with integration test files

#### Scenario: Root tests directory removed
- **WHEN** inspecting the project structure
- **THEN** the root `tests/` directory SHALL NOT exist

#### Scenario: tests Cargo.toml updated
- **WHEN** inspecting `kapi-server/tests/Cargo.toml`
- **THEN** it SHALL contain `kapi-server = { path = ".." }` (or equivalent with renamed import)

#### Scenario: Integration tests pass
- **WHEN** running `cargo test --workspace`
- **THEN** all integration tests in `kapi-server/tests/` SHALL pass
