## Context

Kapi has a working API server with CRUD operations, watch (SSE), finalizers, and namespace support. The `kapi-client` crate provides HTTP client access to all server operations. What's missing is a controller runtime SDK — the framework for writing controllers that watch resources and reconcile state.

This proposal covers the **single controller** case: one controller watching one resource kind with one reconciler. Multi-controller orchestration (Manager) is a separate proposal.

## Goals / Non-Goals

**Goals:**
- Provide a `Reconciler` trait that users implement to define reconciliation logic
- Inject dependencies via `ReconcileContext` (client, request, extensible for future capabilities)
- Provide a `Controller` that ties together reconciler + work queue + watch stream
- Implement a work queue with deduplication, backoff, and rate limiting
- Provide standalone finalizer helper functions

**Non-Goals:**
- Manager for orchestrating multiple controllers (separate proposal)
- Cache/informer layer (future work)
- Secondary watches with mapping functions (future work)
- Predicate/filter system (future work)

## Decisions

### Decision 1: No cache initially

**Choice**: Controllers call `client.get()` directly on each reconcile. No local cache.

**Rationale**: 
- Simpler implementation, no cache invalidation complexity
- kapi has no known scale targets yet — premature optimization
- Cache can be added later behind a trait without changing the reconciler interface
- Reduces memory footprint and eventual consistency issues

**Alternatives considered**:
- **Informer/cache layer from the start**: More complex, requires watch loop to keep cache in sync, eventual consistency issues. Defer until scale demands it.

### Decision 2: Context injection for Reconciler trait

**Choice**: `Reconciler` receives `ReconcileContext` containing request, client, and future capabilities.

```rust
trait Reconciler {
    async fn reconcile(&self, ctx: ReconcileContext) -> Result<ReconcileResult>;
}

struct ReconcileContext {
    pub request: ReconcileRequest,
    pub client: KapiClient,
    // future: cache, event_recorder, logger, etc.
}
```

**Rationale**:
- Framework can add capabilities (cache, event recorder, logger) without breaking existing reconcilers
- Users don't need to manually wire dependencies
- Testing is easier — framework can inject mocks via context

**Alternatives considered**:
- **Minimal trait (just request in, result out)**: Simpler, but users must capture client in their struct. Harder to extend framework without breaking users.

### Decision 3: Standalone finalizer helper functions with CAS retry

**Choice**: Provide `is_deleting()`, `ensure_finalizer()`, `remove_finalizer()` as explicit functions with compare-and-swap retry logic.

```rust
async fn reconcile(&self, ctx: ReconcileContext) -> Result<ReconcileResult> {
    let obj = ctx.client.get(...).await?;
    
    if is_deleting(&obj) {
        // cleanup logic
        remove_finalizer(&ctx.client, &obj, "example.io/cleanup").await?;
        return Ok(ReconcileResult::default());
    }
    
    ensure_finalizer(&ctx.client, &obj, "example.io/cleanup").await?;
    // normal logic
}
```

**Rationale**:
- Explicit, no magic — user controls the flow
- Simple to understand and test
- Matches the pattern used in Kubernetes controller-runtime
- CAS retry prevents race conditions when multiple controllers modify the same object

**Alternatives considered**:
- **Finalizer struct with methods**: More encapsulated but feels magical. User loses control over the flow.
- **No helpers**: Too verbose — users would repeat the same finalizer logic in every controller.
- **No retry logic**: Would fail on conflicts — unacceptable for production use.

### Decision 4: Work queue with dedup and backoff (no rate limiting)

**Choice**: Implement a work queue that deduplicates events by key and applies exponential backoff on errors. Rate limiting is deferred to future work.

**Rationale**:
- Deduplication is essential — 50 events for `foo` should trigger 1 reconcile, not 50
- Backoff prevents overwhelming the API server on errors
- Rate limiting is a solution to a problem we don't have yet (API server overload)
- Simpler v1 implementation, can add rate limiting later when needed

**Alternatives considered**:
- **Minimal channel, no fancy features**: Too simple — would cause redundant reconciles and API overload on errors.
- **No work queue, direct event processing**: Simplest, but will cause problems at scale.
- **Rate limiting in v1**: Over-engineered for initial implementation. Add when scale demands it.

### Decision 4b: Error handling — retry indefinitely with backoff (Option D)

**Choice**: All reconcile errors trigger exponential backoff and retry indefinitely. No distinction between transient and permanent errors at the framework level. Every error is logged with context (object key, error message, retry count).

```
Error → backoff 1s → retry → error → backoff 2s → ... → backoff 5min (max) → retry → ... (forever)
Log: "reconcile failed for foo: <error> (attempt 42)"
```

**Rationale**:
- Simplest implementation — no error type taxonomy needed
- Matches Kubernetes controller-runtime behavior
- Users control retry behavior: return `Ok(ReconcileResult { requeue: false })` after logging a permanent error to stop retrying
- Logs provide visibility for debugging stuck objects
- Can add error type distinction (transient vs permanent) later if needed

**Alternatives considered**:
- **Transient vs permanent error types**: More correct but adds API complexity. Users must categorize errors correctly. Defer until there's a clear need.
- **Max retry count then drop**: Object gets stuck in bad state with no way to retry manually. Worse than infinite retry.
- **No logging**: Users have no visibility into why an object is stuck.

### Decision 5: Watch stream reconnect with list-then-re-enqueue

**Choice**: When the watch stream terminates, the Controller reconnects and lists all objects of the watched kind, enqueuing every key to catch missed events.

**Rationale**:
- Watch streams terminate (server closes connection, network issues)
- Without reconnect, controllers miss events silently
- Without cache, list-then-re-enqueue is the only way to detect missed events
- Matches the Kubernetes list-watch pattern

**Alternatives considered**:
- **No reconnect**: Controllers would stop processing events after stream termination — unacceptable.
- **Resume from last resource_version**: Requires cache to track resource versions — deferred to future work.

### Decision 6: StatusModified event filtering

**Choice**: The Controller filters out `StatusModified` events by default to prevent infinite reconcile loops.

**Rationale**:
- When a reconciler updates status, that triggers a StatusModified event
- Without filtering, this would trigger another reconcile, creating an infinite loop
- Most reconcilers don't need to react to status changes
- Controllers that do need status changes can opt-in via configuration

**Alternatives considered**:
- **No filtering**: Would cause infinite loops in most real-world controllers.
- **User-managed filtering**: Too error-prone — users would forget to filter.

### Decision 7: Namespace scope and watch filter support

**Choice**: The Controller builder supports `.namespace("production")` and `.watch_filter(label_selector)` to scope watches.

**Rationale**:
- Real-world controllers need to watch specific namespaces or filter by labels
- Without this, controllers would watch all objects of a kind — inefficient and potentially incorrect
- Matches the Kubernetes controller-runtime API

**Alternatives considered**:
- **No namespace/filter support**: Would force users to filter in the reconciler — inefficient and error-prone.

### Decision 8: Optional shutdown signal for standalone operation

**Choice**: The Controller accepts an optional shutdown signal. In standalone mode, it's optional. In Manager mode, the Manager provides it.

**Rationale**:
- Controllers should work standalone without a Manager
- Makes the Controller more flexible and testable
- Manager can still provide coordinated shutdown for multiple controllers

**Alternatives considered**:
- **Required shutdown signal**: Would force users to create a Manager even for single-controller use cases.

## Risks / Trade-offs

**[No cache]** → Controllers make an HTTP call on every reconcile. At high scale, this could overwhelm the API server.
→ **Mitigation**: Monitor API server load. If it becomes a problem, add cache layer in a future proposal. The reconciler interface won't change.

**[Standalone finalizer helpers]** → Users must remember to call the helpers in the right order.
→ **Mitigation**: Provide clear documentation and examples. The helpers are simple and explicit, so mistakes are easy to spot.

**[Work queue complexity]** → More complex than a simple channel.
→ **Mitigation**: The complexity is contained in the work queue implementation. Users don't interact with it directly — they just implement `Reconciler`.
