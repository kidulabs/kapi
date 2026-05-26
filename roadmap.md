# kapi — Roadmap

## Completed

- [x] **Persistent storage** — SQLite-backed `ObjectStore` implementation via `rusqlite` with `spawn_blocking`

## Pending

- [ ] **Middleware stack** — Wire AuthLayer, MetricsLayer, TraceLayer, compose full middleware stack
- [ ] **Periodic event bus cleanup** — Background task to scan and remove dead channels from EventBus
- [ ] **Label filtering** — Add `labelSelector` query param for watch and list, `labels` field on `ObjectMeta`
- [ ] **Watch filtering** — `fieldSelector`/`labelSelector` on list (non-watch) requests
- [ ] **Watch resume** — `resourceVersion` param for watch resume with ring buffer replay
- [ ] **Watch bookmarks** — Periodic bookmark events with current resourceVersion
- [ ] **Field selector variants** — `NameNotEquals` and `NameIn` field selectors
- [ ] **Watch filter combinators** — `WatchFilter::And` for composing field and label selectors

## Deferred Improvements

- [ ] **OpenAPI spec caching** — Cache generated OpenAPI spec in `Arc<RwLock<Value>>`, rebuild on Schema mutation

## Explorations

- [ ] **Webhook-based schema validation** — Explore admission webhooks for custom validation beyond meta-schema
- [ ] **PATCH endpoint support** — Evaluate adding strategic merge patch support

## Out of Scope

These are explicitly not being pursued:

- Auth/authorization implementation
- Multi-node clustering or consensus
- Webhook admission controllers
- Kubernetes API compatibility
- PATCH with strategic merge patch
- UI or CLI client
- Conditional delete
