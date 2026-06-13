# kapi — Roadmap

## Pending

- [ ] **Middleware stack** — Wire AuthLayer, MetricsLayer, TraceLayer, compose full middleware stack
- [ ] **Watch resume** — `resourceVersion` param for watch resume with ring buffer replay
- [ ] **Watch bookmarks** — Periodic bookmark events with current resourceVersion
- [ ] **Field selector variants** — `FieldSelector::NameNotEquals`, `FieldSelector::NameIn` for more expressive field-based filtering
- [ ] **Zombie watcher cleanup** — Dead watchers (client disconnected) are only cleaned up lazily on next `publish()` for that `ResourceKey`. If no objects of a kind ever exist, watchers accumulate unbounded. Preferred: periodic background cleanup task. Secondary: `Drop` impl on `EventBus` entries.
- [ ] **Add Finalizer Support** — add finalizer support

## Deferred Improvements

- [ ] **OpenAPI spec caching** — Cache generated OpenAPI spec in `Arc<RwLock<Value>>`, rebuild on Schema mutation

## Future Work

- [ ] **Query optimization for high-cardinality labels** — Improve SQLite EXISTS subquery performance for large label sets
- [ ] **Full label selector syntax parity** — Add set-based operators (`in`, `notin`) to `labelSelector` query parameter for full Kubernetes label selector support
- [ ] **Label indexing** — Index label key-value pairs for efficient high-cardinality label queries at scale
- [ ] **Annotations** — Free-form key-value metadata on `ObjectMeta` without selection semantics (no validation beyond key-value structure)
- [ ] **Schema object status** — kapi-defined status shape for Schema objects (server-maintained: objectCount, schemaVersion, validationState)
- [ ] **Watch event type filtering** — `WatchFilter` support for filtering by `StatusModified` vs `Modified` event types
- [ ] **kapi-controller-runtime** — Separate crate/project: reconcile loops, informers, work queues, leader election, finalizer management

## Explorations

- [ ] **Webhook-based schema validation** — Explore admission webhooks for custom validation beyond meta-schema
- [ ] **PATCH endpoint support** — Evaluate adding strategic merge patch support
- [ ] **Publish Framework** Should publish framework to be moved the storage layer?
- [ ] **Validations** now label and other validations are scattered all around in service layer, should we push them near the storage? early to web handlers?

## Out of Scope

These are explicitly not being pursued:

- Auth/authorization implementation
- Multi-node clustering or consensus
- Webhook admission controllers
- Kubernetes API compatibility
- PATCH with strategic merge patch
- UI or CLI client
- Conditional delete
