## 1. Manager Core

- [ ] 1.1 Implement `Manager` struct in `manager.rs` with fields: `client: KapiClient`, `controllers: Vec<ControllerHandle>`, `shutdown_tx: broadcast::Sender<()>`
- [ ] 1.2 Implement `Manager::new(client: KapiClient)` constructor
- [ ] 1.3 Implement `ControllerHandle` struct to hold controller configuration before registration
- [ ] 1.4 Implement `Manager::controller_for(key: ResourceKey)` method that returns a `ControllerBuilder`
- [ ] 1.5 Implement `ControllerBuilder` with methods: `reconcile_with(reconciler)`, `register()`
- [ ] 1.6 Implement `ControllerBuilder::register()` that adds the controller to the Manager's list
- [ ] 1.7 Run `cargo check` and `cargo clippy` to verify

## 2. Lifecycle Management

- [ ] 2.1 Implement `Manager::start()` async method that:
  - Spawns a tokio task for each registered controller
  - Each task runs the controller's reconcile loop
  - Waits for all tasks to complete
- [ ] 2.2 Implement shutdown signal handling: listen for SIGTERM/SIGINT using `tokio::signal`
- [ ] 2.3 On shutdown signal, broadcast shutdown to all controllers via `shutdown_tx`
- [ ] 2.4 Implement graceful shutdown: wait for all controller tasks to complete with a timeout (30s)
- [ ] 2.5 If timeout is exceeded, log warning and force-exit
- [ ] 2.6 Add panic handling: catch panics in controller tasks, log error, continue running other controllers
- [ ] 2.7 Run `cargo check` and `cargo clippy` to verify

## 3. Controller Integration

- [ ] 3.1 Update `ControllerBuilder` in Manager to pass shutdown signal (broadcast receiver) to each controller via `.shutdown_signal()` method
- [ ] 3.2 Verify that controllers use the shared client from Manager (cloned, not separate instances)
- [ ] 3.3 Run `cargo check` and `cargo clippy` to verify

## 4. Integration Tests

- [ ] 4.1 Create integration test that starts a Manager with multiple controllers
- [ ] 4.2 Test scenario: register two controllers (e.g., Pod and Node), verify both run concurrently
- [ ] 4.3 Test scenario: send shutdown signal, verify graceful shutdown (all reconciles complete)
- [ ] 4.4 Test scenario: one controller panics, verify other controllers continue running
- [ ] 4.5 Test scenario: shutdown timeout, verify Manager force-exits after timeout
- [ ] 4.6 Run integration tests and verify all pass

## 5. Documentation

- [ ] 5.1 Update `docs/controller-runtime.md` with Manager section
- [ ] 5.2 Add example: running multiple controllers with Manager
- [ ] 5.3 Document graceful shutdown behavior and timeout
- [ ] 5.4 Update `README.md` to mention Manager for multi-controller orchestration

## 6. Verification and Cleanup

- [ ] 6.1 Run `cargo build --workspace` to verify all crates build
- [ ] 6.2 Run `cargo test --workspace` to verify all tests pass
- [ ] 6.3 Run `cargo clippy --workspace -- -D warnings` to verify no clippy warnings
- [ ] 6.4 Run `cargo fmt --check` to verify formatting
- [ ] 6.5 Review code for TODO comments and resolve or document them
- [ ] 6.6 Verify `controller-runtime-single` proposal is complete and working before proceeding
- [ ] 6.7 DO NOT auto-commit — user wants to review first
