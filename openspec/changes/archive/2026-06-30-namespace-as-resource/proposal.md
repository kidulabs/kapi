## Why

With the schema-scope-storage-foundation change in place, the system supports namespace-aware storage and scope-based routing. However, namespaces themselves are not yet first-class resources — they're just strings in object metadata. This change introduces Namespace as a registered core resource with its own lifecycle, enabling proper namespace management, validation, and the foundation for namespace-scoped operations.

## What Changes

- Register Namespace as a built-in core type with `kind: "Namespace"`, `group: "kapi.io"`, `version: "v1"`, and `scope: "Cluster"`.
- Auto-create the `"default"` namespace at server startup.
- Make the `"default"` namespace undeletable (403 Forbidden on DELETE).
- Add namespace existence validation: object creation in a non-existent namespace is rejected with 404.
- Add namespace deletion logic: block deletion if namespace contains objects (409 Conflict). Phase 1 implementation.
- Add `WatchFilter::Namespace(String)` variant for namespace-scoped watch subscriptions.
- Update event bus publish/subscribe to support namespace filtering.

## Non-goals

- Namespace cascade deletion with finalizers (Phase 2 — roadmap item)
- Namespace quotas or resource limits
- Namespace-scoped RBAC or access control
- Cross-namespace operations beyond list and watch
- Namespace metadata beyond name, labels, annotations

## Capabilities

### New Capabilities
- `namespace-resource`: Namespace as a first-class cluster-scoped resource with lifecycle management
- `namespace-watch`: WatchFilter::Namespace variant for namespace-scoped watch

### Modified Capabilities
- `object-service`: Namespace existence validation on object creation
- `event-bus`: Namespace-scoped publish/subscribe support

## Impact

- **Bootstrap**: Server startup must create "default" namespace before accepting requests.
- **API**: New endpoints for Namespace CRUD at `/apis/kapi.io/v1/namespaces`.
- **Code**: NamespaceService or integration into existing services, startup bootstrap logic, namespace validation in ObjectService.
- **Tests**: Integration tests for namespace lifecycle, e2e tests for namespace operations.
- **Documentation**: API docs, AGENTS.md updates for namespace resource.

## Future Work

- Phase 2 namespace deletion: cascade with finalizers (set deletion_timestamp, cascade to contained objects, respect finalizers)
- Namespace quotas and resource limits
- Namespace-scoped RBAC
- Namespace annotations for metadata
