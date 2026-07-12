## 1. Setup and Dependencies

- [x] 1.1 Add `kapi-controller` crate to workspace `Cargo.toml` with dependencies: `kapi-client`, `tokio`, `async-trait`, `thiserror`, `tracing`
- [x] 1.2 Create `kapi-controller/src/lib.rs` with module structure: `reconciler`, `controller`, `workqueue`, `finalizer`
- [x] 1.3 Run `cargo check` to verify workspace setup

## 2. Core Types

- [x] 2.1 Implement `ReconcileRequest` struct in `reconciler.rs` with fields: `key: ResourceKey`, `name: String`, `namespace: Option<String>`
- [x] 2.2 Implement `ReconcileResult` struct in `reconciler.rs` with fields: `requeue: bool`, `requeue_after: Option<Duration>`
- [x] 2.3 Implement `ReconcileContext` struct in `reconciler.rs` with fields: `request: ReconcileRequest`, `client: KapiClient`
- [x] 2.4 Define `Reconciler` trait in `reconciler.rs` with `async fn reconcile(&self, ctx: ReconcileContext) -> Result<ReconcileResult>`
- [x] 2.5 Add unit tests for core types (serialization, construction)
- [x] 2.6 Run `cargo check` and `cargo clippy` to verify

## 3. Work Queue

- [x] 3.1 Implement `WorkQueue` struct in `workqueue.rs` with internal state: pending keys (HashSet), queue (VecDeque), backoff tracking (HashMap)
- [x] 3.2 Implement `WorkQueue::add(key)` method that deduplicates keys (if already pending, no-op)
- [x] 3.3 Implement `WorkQueue::get()` async method that returns next key, blocking if queue is empty
- [x] 3.4 Implement `WorkQueue::done(key, success: bool)` method that removes key from pending and applies backoff if `success == false`
- [x] 3.5 Implement exponential backoff logic: on error, log error with context (object key, error message, retry count), requeue key with delay (1s, 2s, 4s, 8s, ... max 5min), retry indefinitely. Reset backoff and retry count on success.
- [ ] 3.6 Implement rate limiting: use token bucket or similar to limit processing rate (e.g., max 10 reconciles/sec) (deferred to future work per design decision)
- [x] 3.7 Implement `WorkQueue::requeue_after(key, duration)` method for delayed requeue
- [x] 3.8 Add unit tests for work queue: deduplication, backoff, rate limiting, requeue_after
- [x] 3.9 Run `cargo check` and `cargo clippy` to verify

## 4. Controller

- [x] 4.1 Implement `Controller` struct in `controller.rs` with fields: `key: ResourceKey`, `namespace: Option<String>`, `watch_filter: WatchFilter`, `reconciler: Arc<dyn Reconciler>`, `client: KapiClient`, `work_queue: WorkQueue`, `shutdown_rx: Option<broadcast::Receiver<()>>`
- [x] 4.2 Implement `Controller::new()` constructor with builder methods: `.namespace()`, `.watch_filter()`, `.shutdown_signal()`
- [x] 4.3 Implement `Controller::start()` async method that:
  - Opens a watch stream for the watched kind using `client.watch()` with namespace and filter
  - Spawns a task to read watch events and enqueue keys in the work queue
  - Spawns a task to process work queue items by calling reconciler
  - Monitors shutdown signal (if provided) and stops gracefully
- [x] 4.4 Implement watch event processing: extract `object.key` and `object.metadata.name`, filter out `StatusModified` events, call `work_queue.add(key)`
- [x] 4.5 Implement watch stream reconnect: when stream terminates or errors, log warning, reconnect, list all objects (respecting namespace/filter), enqueue every key
- [x] 4.6 Implement reconcile loop: call `work_queue.get()`, construct `ReconcileContext`, call `reconciler.reconcile(ctx)`, handle result (requeue if needed), call `work_queue.done(key, success)`
- [x] 4.7 Handle `ReconcileResult`: if `requeue == true`, call `work_queue.add(key)` or `work_queue.requeue_after(key, duration)`
- [x] 4.8 Add error handling: log errors with context (object key, error message, retry count), apply backoff via work queue
- [x] 4.9 Implement graceful shutdown: when shutdown signal received, stop watch stream, stop processing new work, wait for in-flight reconcile to complete, exit
- [x] 4.10 Add unit tests for controller: watch event processing, StatusModified filtering, reconnect behavior, namespace/filter scoping, shutdown handling
- [x] 4.11 Run `cargo check` and `cargo clippy` to verify

## 5. Finalizer Helpers

- [x] 5.1 Implement `is_deleting(obj: &StoredObject) -> bool` in `finalizer.rs` that checks `obj.system.deletion_timestamp.is_some()`
- [x] 5.2 Implement `ensure_finalizer(client: &KapiClient, obj: &StoredObject, finalizer: &str) -> Result<()>` in `finalizer.rs`:
  - If finalizer already in `obj.metadata.finalizers`, return Ok (no-op)
  - Otherwise, clone obj, add finalizer to `metadata.finalizers`, call `client.update()`
  - If update fails with 409 Conflict, re-fetch the object and retry (CAS loop)
- [x] 5.3 Implement `remove_finalizer(client: &KapiClient, obj: &StoredObject, finalizer: &str) -> Result<()>` in `finalizer.rs`:
  - If finalizer not in `obj.metadata.finalizers`, return Ok (no-op)
  - Otherwise, clone obj, remove finalizer from `metadata.finalizers`, call `client.update()`
  - If update fails with 409 Conflict, re-fetch the object and retry (CAS loop)
- [x] 5.4 Add unit tests for finalizer helpers: is_deleting, ensure_finalizer (present/absent/conflict retry), remove_finalizer (present/absent/conflict retry)
- [x] 5.5 Run `cargo check` and `cargo clippy` to verify

## 6. Integration Tests

- [x] 6.1 Create integration test in `kapi-server/tests/` that starts a kapi server and registers a test schema
- [x] 6.2 Write a test reconciler that counts reconcile calls and updates object status
- [x] 6.3 Test scenario: create object, verify reconciler is called, verify status is updated
- [x] 6.4 Test scenario: update object, verify reconciler is called again (deduplication test)
- [x] 6.5 Test scenario: reconciler returns error, verify backoff is applied (multiple rapid updates should not overwhelm)
- [x] 6.6 Test scenario: reconciler uses finalizer helpers to add/remove finalizer, verify object lifecycle
- [x] 6.7 Run integration tests and verify all pass

## 7. Documentation

- [x] 7.1 Check `docs/` directory for existing documentation structure
- [x] 7.2 Add `docs/controller-runtime.md` with overview, examples, and API reference
- [x] 7.3 Include example: simple reconciler that watches a kind and logs events
- [x] 7.4 Include example: reconciler with finalizer cleanup logic
- [x] 7.5 Update `README.md` to mention the new `kapi-controller` crate

## 8. Roadmap and Future Work

- [ ] 8.1 Check `ROADMAP.md` or similar for existing roadmap items (ROADMAP.md does not exist — deferred)
- [ ] 8.2 Add roadmap items for future work: cache/informer layer, secondary watches, predicate/filter system, Manager for multi-controller orchestration (ROADMAP.md does not exist — deferred)
- [ ] 8.3 Verify roadmap is up-to-date with current implementation status (ROADMAP.md does not exist — deferred)

## 9. Verification and Cleanup

- [x] 9.1 Run `cargo build --workspace` to verify all crates build
- [x] 9.2 Run `cargo test --workspace` to verify all tests pass
- [x] 9.3 Run `cargo clippy --workspace -- -D warnings` to verify no clippy warnings
- [x] 9.4 Run `cargo fmt --check` to verify formatting
- [x] 9.5 Review code for TODO comments and resolve or document them
- [x] 9.6 DO NOT auto-commit — user wants to review first
