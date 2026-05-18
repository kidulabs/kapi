## ADDED Requirements

### Requirement: AppConfig struct exists
The library SHALL provide an `AppConfig` struct in a dedicated `config` module that holds all required server configuration: `port` (u16), `store` (Arc<dyn ObjectStore>), and `event_bus` (Arc<dyn EventPublisher>). All fields SHALL be required (non-optional).

#### Scenario: Complete config can be constructed
- **WHEN** a user provides all three fields (`port`, `store`, `event_bus`)
- **THEN** an `AppConfig` instance is created successfully

#### Scenario: Incomplete config is a compile error
- **WHEN** a user attempts to construct `AppConfig` without providing all fields
- **THEN** the code fails to compile

### Requirement: Library does not read environment variables
The library SHALL NOT read environment variables or perform any process-level initialization. All configuration SHALL come from the `AppConfig` struct.

#### Scenario: Library has no env var side effects
- **WHEN** the library's `create_app` or `run` is called
- **THEN** no environment variables are read by the library code
