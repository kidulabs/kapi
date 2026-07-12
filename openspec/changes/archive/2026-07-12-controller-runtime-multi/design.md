## Context

The `controller-runtime-single` proposal provides the foundation: Reconciler trait, Controller, WorkQueue, and finalizer helpers. Users can run a single controller that watches one kind and reconciles state.

This proposal adds the Manager — the orchestrator that runs multiple controllers in one process with shared resources and coordinated lifecycle. This matches the Kubernetes pattern where `controller-manager` is a binary that runs many controllers.

## Goals / Non-Goals

**Goals:**
- Provide a `Manager` that orchestrates multiple controllers
- Share resources across controllers (client, shutdown signal)
- Coordinated lifecycle: start all controllers, graceful shutdown
- Fluent API: `manager.controller_for(key).reconcile_with(...).register()`
- Wait for in-flight reconciles to complete on shutdown

**Non-Goals:**
- Cache/informer layer (future work)
- Secondary watches with mapping functions (future work)
- Predicate/filter system (future work)
- Metrics collection (future work, but Manager will provide extension points)

## Decisions

### Decision 1: Manager owns shared resources

**Choice**: Manager owns the `KapiClient` and shutdown signal. Controllers borrow from Manager.

```rust
let manager = Manager::new(client);
manager.controller_for(pod_key).reconcile_with(PodReconciler).register();
manager.controller_for(node_key).reconcile_with(NodeReconciler).register();
manager.start().await?;
```

**Rationale**:
- Single source of truth for shared resources
- Controllers don't need to manage their own client instances
- Easier to add shared capabilities later (cache, metrics, event recorder)

**Alternatives considered**:
- **Each controller owns its own client**: More flexible, but duplicates resources and makes it harder to add shared capabilities.

### Decision 2: Fluent builder API for controller registration

**Choice**: `manager.controller_for(key).reconcile_with(...).register()` returns a builder that configures the controller before registering it.

**Rationale**:
- Fluent API is ergonomic and extensible
- Can add optional configuration (predicates, secondary watches) without breaking changes
- Clear separation: configure, then register

**Alternatives considered**:
- **Pass fully-built Controller to Manager**: Less ergonomic, user must construct Controller manually.
- **Closure-based registration**: Less clear, harder to extend.

### Decision 3: Graceful shutdown with in-flight reconcile wait

**Choice**: On shutdown signal (Ctrl+C, SIGTERM), Manager stops accepting new work, waits for in-flight reconciles to complete, then exits.

**Rationale**:
- Prevents data corruption from interrupted reconciles
- Matches Kubernetes controller-manager behavior
- Users can implement cleanup logic in reconcilers that will complete before shutdown

**Alternatives considered**:
- **Immediate shutdown on signal**: Simpler, but risks incomplete reconciles and data corruption.
- **Timeout-based shutdown**: More complex, can add later if needed.

### Decision 4: Controllers run as independent tasks

**Choice**: Each controller runs as an independent tokio task. Manager spawns all tasks on `start()` and waits for all to complete on shutdown.

**Rationale**:
- Simple concurrency model
- Controllers are independent — failure in one doesn't affect others
- Easy to reason about and debug

**Alternatives considered**:
- **Single-threaded event loop**: More complex, harder to scale.
- **Thread-per-controller**: Overkill for most use cases, tokio tasks are sufficient.

## Risks / Trade-offs

**[Manager owns client]** → All controllers share one client, so connection pool is shared.
→ **Mitigation**: This is actually a benefit — reduces resource usage. If a controller needs a separate client, it can create one in its reconciler.

**[Graceful shutdown wait]** → Shutdown can hang if a reconciler is stuck.
→ **Mitigation**: Add a timeout (e.g., 30s) after which Manager force-exits. Log a warning if timeout is hit.

**[Independent controller tasks]** → If one controller panics, it doesn't affect others, but the Manager doesn't know about it.
→ **Mitigation**: Catch panics in controller tasks, log them, and continue. Optionally, add a "restart failed controller" feature later.
