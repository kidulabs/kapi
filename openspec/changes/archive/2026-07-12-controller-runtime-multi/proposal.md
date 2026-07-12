## Why

Kapi's controller runtime (from `controller-runtime-single`) provides single-controller operation. Users need to run multiple controllers in one process — e.g., a Deployment controller, a Service controller, and a ConfigMap controller — with shared resources and coordinated lifecycle. Without a Manager, users must manually orchestrate multiple controllers, duplicating shutdown logic and resource management.

## What Changes

- New `Manager` struct that orchestrates multiple controllers in one process
- `Manager::new(client)` constructor that takes a shared `KapiClient`
- `Manager::controller_for(key)` method that returns a builder for registering controllers
- Lifecycle management: `Manager::start()` runs all controllers until shutdown signal
- Graceful shutdown: wait for in-flight reconciles to complete before exiting
- Shared resources: client, shutdown signal, future: cache, metrics, event recorder

## Capabilities

### New Capabilities
- `controller-runtime-manager`: Manager for orchestrating multiple controllers with shared resources and coordinated lifecycle

### Modified Capabilities
<!-- No existing capabilities are being modified -->

## Impact

- **New module**: `Manager` added to `kapi-controller` crate
- **Dependencies**: Depends on `controller-runtime-single` (Reconciler, Controller, WorkQueue)
- **API surface**: Public `Manager` struct and builder methods
- **No breaking changes**: Purely additive, extends the controller runtime

## Non-goals

- Cache/informer layer (future work)
- Secondary watches with mapping functions (future work)
- Predicate/filter system (future work)
- Metrics collection (future work, but Manager will provide extension points)

## Future Work

- Cache/informer layer for local read-only mirror of API server
- Secondary watches with mapping functions (e.g., watch ReplicaSets to trigger Deployment reconcile)
- Predicate/filter system for event filtering before work queue
- Metrics collection (reconcile count, duration, error rate)
- Leader election for HA deployments
