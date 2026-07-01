## MODIFIED Requirements

### Requirement: main.rs is simplified

The binary `main.rs` in `kapi-server` SHALL contain only: tracing initialization, port parsing from `PORT` env var (defaulting to 8080), `AppConfig` construction, and a call to `kapi_server::run(config)`. All `mod` declarations SHALL be removed from `main.rs`.

#### Scenario: main.rs has no mod declarations
- **WHEN** inspecting `kapi-server/src/main.rs`
- **THEN** it contains no `mod` statements

#### Scenario: main.rs is under 20 lines
- **WHEN** counting lines in `kapi-server/src/main.rs` (excluding blank lines and comments)
- **THEN** the file is under 20 lines of code

**Note**: `main.rs` is now located at `kapi-server/src/main.rs` instead of `src/main.rs`. The library function is now `kapi_server::run` instead of `kapi::run`.
