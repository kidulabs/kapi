## ADDED Requirements

### Requirement: create_app function exists
The library SHALL expose a `pub fn create_app(config: &AppConfig) -> axum::Router` function that constructs the full application router. This function SHALL compile the meta-schema, construct the `ObjectService` and `AppState`, and call `build_router`.

#### Scenario: create_app returns a working Router
- **WHEN** `create_app` is called with a valid `AppConfig`
- **THEN** it returns an `axum::Router` ready to serve requests

#### Scenario: create_app borrows config
- **WHEN** `create_app` is called
- **THEN** it takes `&AppConfig` (borrow), not ownership

### Requirement: run function exists
The library SHALL expose a `pub async fn run(config: AppConfig) -> anyhow::Result<()>` function that performs the full server lifecycle: call `create_app`, bind to `0.0.0.0:{port}`, and serve via `axum::serve`.

#### Scenario: run starts the server
- **WHEN** `run` is called with a valid `AppConfig`
- **THEN** the server binds to the specified port and begins serving requests

#### Scenario: run returns error on bind failure
- **WHEN** the port is already in use or invalid
- **THEN** `run` returns an `Err` with the bind error

### Requirement: main.rs is simplified
The binary `main.rs` SHALL contain only: tracing initialization, port parsing from `PORT` env var (defaulting to 8080), `AppConfig` construction, and a call to `kapi::run(config)`. All `mod` declarations SHALL be removed from `main.rs`.

#### Scenario: main.rs has no mod declarations
- **WHEN** inspecting `main.rs`
- **THEN** it contains no `mod` statements

#### Scenario: main.rs is under 20 lines
- **WHEN** counting lines in `main.rs` (excluding blank lines and comments)
- **THEN** the file is under 20 lines of code
