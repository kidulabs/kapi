### Requirement: Manager orchestrates multiple controllers
The system SHALL provide a `Manager` struct that orchestrates multiple controllers in one process, sharing resources and coordinating lifecycle.

#### Scenario: Manager runs multiple controllers
- **WHEN** Manager is started with multiple registered controllers
- **THEN** all controllers SHALL run concurrently as independent tasks

#### Scenario: Controllers share resources
- **WHEN** multiple controllers are registered with a Manager
- **THEN** all controllers SHALL share the same `KapiClient` instance provided to the Manager

### Requirement: Manager constructor takes shared client
The system SHALL provide a `Manager::new(client: KapiClient)` constructor that creates a Manager with a shared client instance.

#### Scenario: Create Manager with client
- **WHEN** user calls `Manager::new(client)`
- **THEN** the Manager SHALL store the client for sharing with all registered controllers

### Requirement: Manager provides controller_for builder
The system SHALL provide a `Manager::controller_for(key: ResourceKey)` method that returns a builder for configuring and registering a controller.

#### Scenario: Register a controller
- **WHEN** user calls `manager.controller_for(pod_key).reconcile_with(PodReconciler).register()`
- **THEN** the Manager SHALL register a controller that watches `pod_key` and uses `PodReconciler`

#### Scenario: Builder is fluent
- **WHEN** user chains builder methods
- **THEN** the builder SHALL support method chaining for configuration (e.g., `.reconcile_with(...).register()`)

### Requirement: Manager starts all controllers
The system SHALL provide a `Manager::start()` async method that starts all registered controllers and runs until a shutdown signal is received.

#### Scenario: Start Manager
- **WHEN** user calls `manager.start().await`
- **THEN** the Manager SHALL spawn a task for each registered controller and wait for all to complete

#### Scenario: Controllers run concurrently
- **WHEN** Manager is started with multiple controllers
- **THEN** each controller SHALL run as an independent tokio task

### Requirement: Manager handles graceful shutdown
The system SHALL handle graceful shutdown on receiving a shutdown signal (SIGTERM, SIGINT), stopping all controllers and waiting for in-flight reconciles to complete.

#### Scenario: Shutdown signal received
- **WHEN** Manager receives a shutdown signal (e.g., Ctrl+C)
- **THEN** the Manager SHALL signal all controllers to stop and wait for in-flight reconciles to complete

#### Scenario: In-flight reconciles complete
- **WHEN** controllers are shutting down
- **THEN** the Manager SHALL wait for all in-flight reconcile calls to complete before exiting

#### Scenario: Shutdown timeout
- **WHEN** in-flight reconciles do not complete within a timeout (e.g., 30 seconds)
- **THEN** the Manager SHALL log a warning and force-exit

### Requirement: Manager catches controller panics
The system SHALL catch panics in controller tasks, log them, and continue running other controllers.

#### Scenario: Controller panics
- **WHEN** a controller task panics
- **THEN** the Manager SHALL catch the panic, log an error, and continue running other controllers

#### Scenario: Other controllers unaffected
- **WHEN** one controller panics
- **THEN** other controllers SHALL continue running normally

### Requirement: Manager provides shutdown signal
The system SHALL provide a shutdown signal mechanism that controllers can use to detect when the Manager is shutting down.

#### Scenario: Controller detects shutdown
- **WHEN** Manager is shutting down
- **THEN** controllers SHALL be able to detect the shutdown signal and stop processing new work

### Requirement: Manager is extensible for future capabilities
The system SHALL design the Manager to be extensible for future capabilities (cache, metrics, event recorder) without breaking existing code.

#### Scenario: Add future capability
- **WHEN** a new capability is added to the Manager (e.g., cache)
- **THEN** existing controller registration code SHALL continue to work without modification
