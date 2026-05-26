# kapi — Roadmap

## Completed

- [x] **Persistent storage** — SQLite-backed `ObjectStore` implementation via `rusqlite` with `spawn_blocking`
- [x] **Predicate routing event bus** — Replaced `tokio::broadcast` with per-watcher `mpsc` channels + `WatchFilter` for filtered event delivery
- [x] **Field selector watch filtering** — `?fieldSelector=metadata.name=<name>` query parameter on watch requests with strict validation (400 for unsupported fields, malformed syntax, or fieldSelector on non-watch requests)
- [x] **OpenAPI spec for field selectors** — `fieldSelector` parameter and `400` response documented in generated OpenAPI 3.0.3 spec

## Pending

- [ ] **Middleware stack** — Wire AuthLayer, MetricsLayer, TraceLayer, compose full middleware stack
- [ ] **Label filtering** — Add `labelSelector` query param for watch and list, `labels` field on `ObjectMeta`
- [ ] **Watch filtering on list requests** — `fieldSelector`/`labelSelector` on non-watch list requests (requires store-level filtering)
- [ ] **Watch resume** — `resourceVersion` param for watch resume with ring buffer replay
- [ ] **Watch bookmarks** — Periodic bookmark events with current resourceVersion
- [ ] **Field selector variants** — `FieldSelector::NameNotEquals`, `FieldSelector::NameIn` for more expressive field-based filtering
- [ ] **Watch filter combinators** — `WatchFilter::And(Box<WatchFilter>, Box<WatchFilter>)` for composing field and label selectors

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
