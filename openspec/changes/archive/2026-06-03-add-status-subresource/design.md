## Context

kapi currently stores all user-defined data in a single `spec` field (renamed from `data` in the preceding change). Objects have one write path — `PUT /apis/{g}/{v}/{kind}/{name}` — with optimistic concurrency via `resource_version`. This works for simple CRUD but breaks down when controllers need to write status concurrently with user spec updates.

The status subresource introduces a separate write path for observed state. Controllers write status via `PUT /status` without optimistic concurrency (the server does a read-modify-write internally), while users write spec via the existing `PUT` endpoint with CAS. Both paths bump `resource_version`, so watchers see a single monotonic version stream.

This change depends on the `rename-data-to-spec` change being applied first, since it adds `status: Option<SpecData>` alongside the newly-renamed `spec` field.

## Goals / Non-Goals

**Goals:**
- Add `status: Option<SpecData>` to `StoredObject` (nullable, `None` for kinds without status)
- Add `statusSchema` to `SchemaData` and the meta-schema (opt-in per kind)
- Add `update_status()` to `ObjectStore` (no CAS, server-side read-modify-write)
- Add `PUT/GET /status` endpoints (404 for kinds without `statusSchema`)
- Add `StatusModified` event type
- Validate status against `statusSchema` on update
- Cache status validators in `SchemaRegistry`
- Status starts as `null` on create (ignored in request body)

**Non-Goals:**
- Schema object status (server-maintained objectCount, etc.)
- `generation` field (bumped only on spec changes)
- `status_version` (separate version counter)
- `count()` on `ObjectStore`
- Watch event type filtering

## Decisions

### Decision 1: Single `resource_version`, no CAS on status

**Choice**: Status updates use `update_status(key, name, status_value)` — no client-provided version. The server reads the current object, replaces only `status`, bumps `resource_version`, and writes back. No conflict possible.

**Alternatives considered**:
- Separate `status_version` counter — adds complexity to watch resume, requires two version fields per object, complicates the store
- CAS on status with `status_version` — controllers would need to read-then-write, reintroducing the conflict problem

**Rationale**: The whole point of the status subresource is to eliminate spec/status write conflicts. A server-side read-modify-write with no CAS achieves this simply. A single `resource_version` keeps the watch model simple.

### Decision 2: `status: Option<SpecData>` on `StoredObject`

**Choice**: Add `status: Option<SpecData>` as a nullable field. Kinds without `statusSchema` always have `status: null`.

**Alternatives considered**:
- Separate `status` table — adds join complexity, no benefit for the simple case
- `status: SpecData` with empty default — conflates "no status" with "empty status"

**Rationale**: `Option<SpecData>` clearly distinguishes "this kind has no status subresource" from "status is present but empty". The `Option` maps naturally to `NULL` in SQLite and `null` in JSON.

### Decision 3: `statusSchema` in meta-schema is optional

**Choice**: Add `statusSchema` as an optional property to the meta-schema. When present, the `/status` endpoint is enabled for that kind. When absent, `/status` returns `StatusSubresourceNotEnabled`.

**Alternatives considered**:
- Separate API to enable/disable status — adds operational complexity
- Convention-based (top-level `status` in `jsonSchema`) — conflates schema structure with subresource semantics

**Rationale**: Making it part of schema registration is the natural place. It's opt-in, discoverable, and validated at registration time.

### Decision 4: Status validators cached alongside spec validators

**Choice**: `SchemaRegistry` caches status validators under `{kind}.{group}.status` keys alongside spec validators under `{kind}.{group}`.

**Rationale**: Simple, consistent with existing caching pattern. On schema update, both validators are recompiled and cached. On schema delete, both are evicted.

### Decision 5: `StatusModified` event type

**Choice**: Add `StatusModified` variant to `WatchEventType`. The event carries the full `StoredObject` (including both spec and status).

**Rationale**: Watchers need the full object context. A `StatusModified` event tells watchers "only status changed" without losing the spec context. Future controller-runtime can filter on event type.

### Decision 6: Status ignored on create

**Choice**: `POST /apis/{g}/{v}/{kind}` ignores any `status` field in the request body. Status always starts as `null`.

**Rationale**: In Kubernetes, status is set by controllers after creation, not by the user at creation time. This prevents users from setting initial status values that controllers would immediately overwrite.

## Risks / Trade-offs

- **[API surface area doubles for status-enabled kinds]** → Acceptable; `/status` endpoints are only active for kinds with `statusSchema`
- **[Two schemas per kind increases registration complexity]** → Mitigated by `statusSchema` being optional; simple kinds are unaffected
- **[SQLite migration: adding nullable column]** → Low risk; `ALTER TABLE objects ADD COLUMN status TEXT` is non-destructive, existing rows get `NULL`
- **[SchemaRegistry cache key collision]** → Mitigated by using `.status` suffix for status validator keys
- **[Status update without CAS could mask bugs]** → Acceptable; controllers should be idempotent anyway

## Open Questions

_None — all decisions are resolved._