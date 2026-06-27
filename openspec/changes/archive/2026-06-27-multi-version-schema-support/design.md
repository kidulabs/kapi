## Context

kapi's data model is fully GVK-aware: `ResourceKey { group, version, kind }` flows through handlers, services, both `InMemoryStore` and `SQLiteStore`, and the event bus. A user can already POST objects of `example.io/v1/Widget` and `example.io/v2/Widget` simultaneously — the store primary key (`(resource_group, api_version, resource_kind, name)`) keeps them separate.

The blocker is at the **Schema identity** layer. The `Schema` kind is itself a regular object stored under `kapi.io/v1/Schema`; its `metadata.name` is generated from the registration payload. Three call sites in the current code derive that name (or a key derived from it) from `targetKind` and `targetGroup` only, ignoring `targetVersion`:

- `src/object/handler.rs::extract_schema_name` (lines 68-72) — generates `Widget.example.io`
- `src/schema/registry.rs` — cache keys at lines 103, 166, plus an independently-built store-lookup name at line 175
- `src/openapi/paths.rs` (line 60) — `format!("{}.{}", target_kind, target_group)` fed to `component_name()`

Council review of this plan (2/2 councillors responding) identified one critical gap: `get_status_validator` builds its store-lookup name *independently* of its cache key (line 175: `let schema_name = format!("{}.{}", key.kind, key.group)`), so a naive cache-key fix leaves the store lookup broken. The plan below fixes both.

## Goals / Non-Goals

**Goals:**
- A user can register `example.io/v1/Widget` and `example.io/v2/Widget` as two separate Schemas
- The two compiled validators do not interfere (independent cache entries, independent eviction)
- The two appear as distinct kinds in the generated OpenAPI spec
- Schema `metadata.name` is stable, deterministic, and human-readable
- `SchemaHasObjects` deletion conflict remains per-version (no behavior change)
- No new traits, no new abstractions beyond a single small helper

**Non-Goals:**
- Conversion webhooks / cross-version translation
- Versionless kind aggregation
- `SchemaHasObjects` error variant enrichment (council: punt)
- Smarter version-string transforms (e.g., `v1alpha1` → `V1Alpha1`); raw string flows through
- A migration of pre-existing stored schemas (documented breaking change)

## Decisions

### 1. Schema name format: `{kind}.{group}.{version}`

**Choice**: `format!("{}.{}.{}", target_kind, target_group, target_version)`

**Rationale**: The user picked `k.g.v` order. This is also the natural reading order ("the Widget in example.io v1"). Raw `version` string flows through — no normalization, so `v1alpha1` and `V1` are distinct (the latter collides with `v1` only in the OpenAPI component name, which is a display concern, not an identity concern).

**Alternatives considered**:
- `k.v.g` — rejected by user
- Separate `metadata.name` from the version lookup (e.g., user-supplied name + index) — adds a lookup indirection; store already requires unique names so the simpler form works
- Embedding the version only in the cache key, not in `metadata.name` — would still let the store accept both versions, but the cache would silently corrupt; rejected

### 2. Centralize format in `schema_cache_key` helper

**Choice**: New `pub fn schema_cache_key(kind: &str, group: &str, version: &str) -> String` in `src/schema/mod.rs`. All four call sites use it.

**Rationale**: 4-line helper, prevents the format string from drifting. Council flagged this as a nice-to-have; we include it because the cost is trivial and the benefit (one place to change) compounds over time.

**Alternatives considered**:
- Inline `format!()` at each call site — fewer lines, more drift risk
- New `SchemaIdentity` newtype — over-engineering for 4 call sites

### 3. Fix the `get_status_validator` store-lookup name independently

**Choice**: Line 175 is rebuilt with the helper, not just the cache key.

**Rationale**: Council review caught this. `get_validator` works because line 113 does `let schema_name = cache_key.clone()`. `get_status_validator` does not — it builds its own `schema_name`. Both must be updated.

### 4. OpenAPI component name: `KindGroupVersion` pattern

**Choice**: `component_name("Widget.example.io.v1")` → `WidgetExampleIoV1`

**Rationale**: The existing `component_name` helper (`src/openapi/components.rs:23-40`) already splits on `.` and PascalCases each segment. No transform changes needed — feeding it a 4-segment input produces a 4-segment PascalCase output. The pattern is also the natural extension of the existing `KindGroup` convention.

**Pre-existing edge case** (not introduced by this change): `component_name("Widget.example.io.V1")` and `component_name("Widget.example.io.v1")` both produce `WidgetExampleIoV1`. The source-of-truth identity (the Schema's `metadata.name`) is the raw string, so this only affects the generated doc and is a follow-up concern.

### 5. No automated migration

**Choice**: Pre-existing Schema records stored under the old name become invisible after this change. Documented in the commit message; no migration code.

**Rationale**: Pre-1.0 project, re-registration is trivial, the data being migrated is validation metadata (cheap to recreate). Adding a one-time migration bloats the change with code that will never run again. Council aligned with this.

## Risks / Trade-offs

- **[High test churn, low runtime risk]** → Mitigation: ~51 mechanical literal updates, all caught by `cargo test`. Run the full test suite before merging.

- **[SQLite backward incompat: existing schemas orphaned]** → Mitigation: document the breaking change prominently in the commit message; re-registration is a single POST per kind.

- **[OpenAPI display collision: `V1` vs `v1` produce same component name]** → Mitigation: out of scope; the source-of-truth identity uses the raw string so the store and cache are correct. Document in commit message as a known display quirk.

- **[Drift between four call sites using the same format string]** → Mitigation: the `schema_cache_key` helper centralizes the format. Lint-style: if a future PR adds a new call site, code review should catch any non-helper usage.

- **[Status subresource cache key correctness]** → Mitigation: `get_status_validator` cache key is `{k}.{g}.{v}.status`, store lookup name is `{k}.{g}.{v}` (without `.status`). Both are rebuilt with the helper. `evict(name)` removes both `name` and `name.status`, which now match.

## Migration Plan

No code migration. The breaking change is contained to the `Schema` resource's `metadata.name`:
- Before: `"Widget.example.io"`
- After: `"Widget.example.io.v1"`

Deployment steps (documented in commit message, not automated):
1. Deploy the new server binary
2. Re-register all Schemas (POST to `/apis/kapi.io/v1/Schema` with the same `targetGroup`, `targetKind`, `targetVersion` as before — the response's `metadata.name` will be the new format)
3. Optionally delete the old-name Schema records (they are no longer reachable by the server but may still exist in the store)

Rollback: revert the commit. Old-name Schemas become reachable again. New-name Schemas (if any were registered) become invisible.

## Open Questions

None blocking. The only deferred item is the `SchemaHasObjects` error variant enrichment (`kind` only → `kind`, `group`, `version`), which council recommended punting. Could be a follow-up change of ~10 lines plus a spec delta.
