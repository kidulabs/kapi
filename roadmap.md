# kapi — Roadmap

## Pending

- [ ] **Implement kapi-client HTTP client library** — reqwest-based wrappers for CRUD, watch, schema, status operations. Depends on kapi-core for shared types.
- [ ] **Implement kapi-cli with full command coverage** — CLI tool for schema CRUD, object CRUD, watch, status. Depends on kapi-client.
- [ ] **Implement kapi-controller controller-runtime SDK** — Informer, WorkQueue, Controller trait for building controllers. Depends on kapi-client.
- [ ] **Add resource_version to ListResponse and implement watch resume** — Prerequisite for correct Informer behavior. ListResponse needs resource_version field, watch needs resume capability with ring buffer replay.
- [ ] **Middleware stack** — Wire AuthLayer, MetricsLayer, TraceLayer, compose full middleware stack
- [ ] **Watch resume** — `resourceVersion` param for watch resume with ring buffer replay
- [ ] **Watch bookmarks** — Periodic bookmark events with current resourceVersion
- [ ] **Field selector variants** — `FieldSelector::NameNotEquals`, `FieldSelector::NameIn` for more expressive field-based filtering
- [ ] **Zombie watcher cleanup** — Dead watchers (client disconnected) are only cleaned up lazily on next `publish()` for that `ResourceKey`. If no objects of a kind ever exist, watchers accumulate unbounded. Preferred: periodic background cleanup task. Secondary: `Drop` impl on `EventBus` entries.
- [ ] **Namespace cascade deletion (Phase 2)** — `deletion_timestamp`-based cascade: when a Namespace is deleted with objects, set `deletion_timestamp`, controllers finalize their objects, then hard-delete. Currently namespace deletion is hard-blocked (409) until empty.
- [x] **Add Finalizer Support** — add finalizer support
- [x] **Status OCC decision** — Status updates are unconditional (no OCC). The status subresource exists to eliminate spec/status write conflicts; OCC on status would reintroduce them. The `generation` field provides staleness detection; controllers should be idempotent. Revisit if controller-runtime ships with multi-controller status writes AND silent clobbering causes demonstrable problems.
- [x] **Namespace-as-resource (Phase 1)** — Namespace is a first-class cluster-scoped core type. `"default"` namespace is auto-created and undeletable. Namespace existence is validated on object CREATE. Non-empty namespaces cannot be deleted. `WatchFilter::Namespace(String)` for namespace-scoped watch.

## Deferred Improvements

- [ ] **OpenAPI spec caching** — Cache generated OpenAPI spec in `Arc<RwLock<Value>>`, rebuild on Schema mutation

## Future Work

- [ ] **Version conversion webhooks** — Exploration of a conversion-hook mechanism that translates objects between registered API versions of the same kind. When multiple versions of a kind are registered (e.g., `example.io/v1/Widget` and `example.io/v2/Widget`), a conversion webhook would allow reading objects at any supported version by converting between the version's schema. This is the natural follow-up to multi-version schema support.
- [ ] **Query optimization for high-cardinality labels** — Improve SQLite EXISTS subquery performance for large label sets
- [ ] **Full label selector syntax parity** — Add set-based operators (`in`, `notin`) to `labelSelector` query parameter for full Kubernetes label selector support
- [ ] **Label indexing** — Index label key-value pairs for efficient high-cardinality label queries at scale
- [x] **Annotations** — Free-form key-value metadata on `ObjectMeta` without selection semantics (no validation beyond key-value structure)
- [ ] **Schema object status** — kapi-defined status shape for Schema objects (server-maintained: objectCount, schemaVersion, validationState)
- [ ] **Watch event type filtering** — `WatchFilter` support for filtering by `StatusModified` vs `Modified` event types
- [ ] **kapi-controller-runtime** — Separate crate/project: reconcile loops, informers, work queues, leader election, finalizer management

## Explorations

- [ ] **Webhook-based schema validation** — Explore admission webhooks for custom validation beyond meta-schema
- [ ] **PATCH endpoint support** — Evaluate adding strategic merge patch support
- [ ] **Publish Framework** Should publish framework to be moved the storage layer?
- [x] **Validations** Moved format validation (labels, annotations) to handler edge with `src/validation/` module. Service retains defense-in-depth. Stateful validation (schema lookup, OCC) stays in service.

## Out of Scope

These are explicitly not being pursued:

- Auth/authorization implementation
- Multi-node clustering or consensus
- Webhook admission controllers
- Kubernetes API compatibility
- PATCH with strategic merge patch
- UI or CLI client
- Conditional delete
