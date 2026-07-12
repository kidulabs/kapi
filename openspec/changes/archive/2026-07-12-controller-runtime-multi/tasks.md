## 1. Manager Core

- [x] 1.1 Implement `Manager` struct in `manager.rs` with fields: `client: KapiClient`, `controllers: Vec<ControllerHandle>`, `shutdown_tx: broadcast::Sender<()>`
- [x] 1.2 Implement `Manager::new(client: KapiClient)` constructor
- [x] 1.3 Implement `ControllerHandle` struct to hold controller configuration before registration
- [x] 1.4 Implement `Manager::controller_for(key: ResourceKey)` method that returns a `ControllerBuilder`
- [x] 1.5 Implement `ControllerBuilder` with methods: `reconcile_with(reconciler)`, `register()`
- [x] 1.6 Implement `ControllerBuilder::register()` that adds the controller to the Manager's list
- [x] 1.7 Run `cargo check` and `cargo clippy` to verify

## 2. Lifecycle Management

- [x] 2.1 Implement `Manager::start()` async method that:
  - Spawns a tokio task for each registered controller
  - Each task runs the controller's reconcile loop
  - Waits for all tasks to complete
- [x] 2.2 Implement shutdown signal handling: listen for SIGTERM/SIGINT using `tokio::signal`
- [x] 2.3 On shutdown signal, broadcast shutdown to all controllers via `shutdown_tx`
- [x] 2.4 Implement graceful shutdown: wait for all controller tasks to complete with a timeout (30s)
- [x] 2.5 If timeout is exceeded, log warning and force-exit
- [x] 2.6 Add panic handling: catch panics in controller tasks, log error, continue running other controllers
- [x] 2.7 Run `cargo check` and `cargo clippy` to verify

## 3. Controller Integration

- [x] 3.1 Update `ControllerBuilder` in Manager to pass shutdown signal (broadcast receiver) to each controller via `.shutdown_signal()` method
- [x] 3.2 Verify that controllers use the shared client from Manager (cloned, not separate instances)
- [x] 3.3 Run `cargo check` and `cargo clippy` to verify

## 4. Integration Tests

- [x] 4.1 Create integration test that starts a Manager with multiple controllers
- [x] 4.2 Test scenario: register two controllers (e.g., Pod and Node), verify both run concurrently
- [x] 4.3 Test scenario: send shutdown signal, verify graceful shutdown (all reconciles complete)
- [x] 4.4 Test scenario: one controller panics, verify other controllers continue running
- [x] 4.5 Test scenario: shutdown timeout, verify Manager force-exits after timeout
- [x] 4.6 Run integration tests and verify all pass

## 5. Documentation

- [x] 5.1 Update `docs/controller-runtime.md` with Manager section
- [x] 5.2 Add example: running multiple controllers with Manager
- [x] 5.3 Document graceful shutdown behavior and timeout
- [x] 5.4 Update `README.md` to mention Manager for multi-controller orchestration

## 6. Verification and Cleanup

- [x] 6.1 Run `cargo build --workspace` to verify all crates build
- [x] 6.2 Run `cargo test --workspace` to verify all tests pass
- [x] 6.3 Run `cargo clippy --workspace -- -D warnings` to verify no clippy warnings
- [x] 6.4 Run `cargo fmt --check` to verify formatting
- [x] 6.5 Review code for TODO comments and resolve or document them
- [x] 6.6 Verify `controller-runtime-single` proposal is complete and working before proceeding
- [x] 6.7 DO NOT auto-commit — user wants to review first
