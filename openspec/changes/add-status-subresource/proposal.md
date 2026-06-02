## Why

kapi needs a status subresource to support controller-runtime semantics. In a reconciliation loop, a user writes `spec` (desired state) and a controller writes `status` (observed state). Without separate write paths, concurrent spec and status updates conflict on `resource_version`. The status subresource provides: separate validation schemas, a dedicated update endpoint without optimistic concurrency, and a distinct `StatusModified` event type for watch filtering. This is a foundational primitive for the planned kapi-controller-runtime project.

## What Changes

- Add `status: Option<SpecData>` field to `StoredObject` (nullable, `None` for kinds without status)
- Add `status_schema: Option<serde_json::Value>` field to `SchemaData` (opt-in per kind)
- Add `statusSchema` optional property to the meta-schema
- Add `update_status(key, name, status)` method to `ObjectStore` trait (no CAS, server-side read-modify-write)
- Add `StatusModified` variant to `WatchEventType`
- Add `StatusSubresourceNotEnabled` error variant to `AppError`
- Add `GET /apis/{group}/{version}/{kind}/{name}/status` endpoint (returns 404 for kinds without `statusSchema`)
- Add `PUT /apis/{group}/{version}/{kind}/{name}/status` endpoint (returns 404 for kinds without `statusSchema`)
- Add `update_status()` method to `ObjectService` (validates against `statusSchema`, publishes `StatusModified` event)
- Add `status` column to SQLite `objects` table (nullable TEXT)
- Update `SchemaRegistry` to cache status validators alongside spec validators
- Update `create()` to ignore any `status` field in the request body (status starts as `null`)

## Capabilities

### New Capabilities

- `status-subresource`: Status subresource for controller-runtime semantics — separate spec/status write paths, validation, and events

### Modified Capabilities

- `core-types`: Add `status: Option<SpecData>` to `StoredObject`, `status_schema` to `SchemaData`, `StatusModified` to `WatchEventType`
- `object-store`: Add `update_status()` method to `ObjectStore` trait
- `object-service`: Add `update_status()` method, update `create()` to ignore status, update schema registration to handle `statusSchema`
- `object-handlers`: Add `get_status` and `update_status` handlers, add `/status` route
- `meta-schema`: Add `statusSchema` optional property
- `schema-registry`: Cache status validators alongside spec validators

## Impact

- **API**: New endpoints `GET/PUT /status`, new `StatusModified` event type, new `status` field in StoredObject responses
- **Storage**: New nullable `status` column in SQLite, new `update_status()` method
- **Schema**: Meta-schema gains optional `statusSchema` property
- **Backward compatible**: Kinds without `statusSchema` work exactly as before; `status` is `null` in responses

## Non-goals

- Schema object status (server-maintained objectCount, schemaVersion, etc.) — deferred to future work
- `generation` field (separate from `resource_version`) — deferred to future work
- `status_version` (separate version counter for status) — deferred; single `resource_version` bumped on any change
- `count()` method on `ObjectStore` — only needed for Schema status, deferred
- Watch filtering by event type (e.g., `?watch=spec-only`) — deferred to controller-runtime project

## Future Work

- Schema object status (kapi-defined shape, server-maintained: objectCount, schemaVersion, validationState)
- `generation` field on `SystemMetadata` (bumped only on spec changes, not status changes)
- Watch event type filtering in `WatchFilter`
- kapi-controller-runtime project (separate crate: reconcile loops, informers, work queues, leader election)