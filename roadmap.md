# kapi — Roadmap

## Completed

- [x] **Persistent storage** — SQLite-backed `ObjectStore` implementation via `rusqlite` with `spawn_blocking`
- [x] **Predicate routing event bus** — Replaced `tokio::broadcast` with per-watcher `mpsc` channels + `WatchFilter` for filtered event delivery
- [x] **Field selector watch filtering** — `?fieldSelector=metadata.name=<name>` query parameter on watch requests with strict validation (400 for unsupported fields, malformed syntax)
- [x] **OpenAPI spec for field selectors** — `fieldSelector` parameter and `400` response documented in generated OpenAPI 3.0.3 spec
- [x] **Label selector watch filtering** — `?labelSelector=<selector>` query parameter on watch requests with moderate K8s syntax (equality, inequality, existence, non-existence, AND combinator)
- [x] **OpenAPI spec for label selectors** — `labelSelector` parameter and `400` response documented in generated OpenAPI 3.0.3 spec
- [x] **Label filtering (watch)** — `labels` field on `ObjectMeta` with validation; `labelSelector` query param for watch with moderate K8s syntax (equality, inequality, existence, non-existence, AND)
- [x] **Label filtering (list)** — `labelSelector` on non-watch list requests with store-level filtering in both InMemoryStore and SQLiteStore
- [x] **Watch filtering on list requests** — `fieldSelector`/`labelSelector` on non-watch list requests with store-level filtering before pagination
- [x] **Watch filter combinators** — `WatchFilter::And(Box<WatchFilter>, Box<WatchFilter>)` for composing field and label selectors on watch
- [x] **Rename data to spec** — Rename `StoredObject.data` → `.spec` and `UserData` → `SpecData` across all layers (`openspec/changes/rename-data-to-spec`)
- [x] **Add status subresource** — `StoredObject.status: Option<SpecData>`, `PUT/GET /status` endpoint, `StatusModified` event, `update_status()` on store, `statusSchema` in meta-schema (`openspec/changes/add-status-subresource`)
- [x] **Extract SchemaRegistry** — Extract schema compilation, caching, and lookup from `ObjectService` into a `SchemaRegistry` collaborator (`openspec/changes/extract-schema-registry`)
- [x] **Generation field** — `SystemMetadata.generation: u64` bumped only on spec changes, not status changes; enables controllers to detect spec drift

## Pending

- [ ] **Middleware stack** — Wire AuthLayer, MetricsLayer, TraceLayer, compose full middleware stack
- [ ] **Watch resume** — `resourceVersion` param for watch resume with ring buffer replay
- [ ] **Watch bookmarks** — Periodic bookmark events with current resourceVersion
- [ ] **Field selector variants** — `FieldSelector::NameNotEquals`, `FieldSelector::NameIn` for more expressive field-based filtering
- [ ] **Zombie watcher cleanup** — Dead watchers (client disconnected) are only cleaned up lazily on next `publish()` for that `ResourceKey`. If no objects of a kind ever exist, watchers accumulate unbounded. Preferred: periodic background cleanup task. Secondary: `Drop` impl on `EventBus` entries.
- [ ] **Add Finalizer Support** — add finalizer support
- [ ] **Make the store dumb** - the store need not worry about generation, resourceversion, updated timestamps etc



## Deferred Improvements

- [ ] **OpenAPI spec caching** — Cache generated OpenAPI spec in `Arc<RwLock<Value>>`, rebuild on Schema mutation

## Future Work

- [ ] **OR combinators for label selectors** — Support OR logic between label requirements (Kubernetes doesn't support this natively, but may be useful)
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
