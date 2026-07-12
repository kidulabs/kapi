## Why

Kapi needs a controller runtime SDK to enable writing controllers that watch resources and reconcile desired state. Without this, users cannot implement the feedback loop pattern where controllers observe changes and converge the system toward desired state.

## What Changes

- New `kapi-controller` crate providing controller-runtime primitives
- `Reconciler` trait with `ReconcileContext` injection (client, request, extensible for future capabilities)
- `Controller` struct that ties together a reconciler, work queue, and watch stream for a single resource kind
- `WorkQueue` with deduplication (by key), exponential backoff on errors, and support for `ReconcileResult { requeue_after }`
- Standalone finalizer helper functions: `is_deleting()`, `ensure_finalizer()`, `remove_finalizer()` with compare-and-swap retry logic
- Watch stream reconnect with list-then-re-enqueue to catch missed events
- StatusModified event filtering to prevent infinite reconcile loops
- Namespace scope and watch filter support for scoping controller watches
- Optional shutdown signal for standalone controller operation

## Capabilities

### New Capabilities
- `controller-runtime-core`: Core controller runtime primitives — Reconciler trait, ReconcileContext, Controller, WorkQueue, and finalizer helpers for single-controller operation

### Modified Capabilities
<!-- No existing capabilities are being modified -->

## Impact

- **New crate**: `kapi-controller` added to workspace
- **Dependencies**: Depends on `kapi-client` for API interaction
- **API surface**: Public traits and structs for users to implement controllers
- **No breaking changes**: Purely additive

## Non-goals

- Manager for orchestrating multiple controllers (separate proposal)
- Cache/informer layer (future work)
- Secondary watches with mapping functions (future work)
- Predicate/filter system (future work)

## Future Work

- Cache/informer layer for local read-only mirror of API server
- Secondary watches with mapping functions (e.g., watch ReplicaSets to trigger Deployment reconcile)
- Predicate/filter system for event filtering before work queue
- Manager for orchestrating multiple controllers in one process
