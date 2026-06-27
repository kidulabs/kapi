## Why

The kapi data model is already fully version-aware: `ResourceKey { group, version, kind }` flows through every layer, both stores (InMemory and SQLite) key on the full GVK, and routes require the version segment. A user can POST objects of `example.io/v1/Widget` and `example.io/v2/Widget` simultaneously and the store keeps them separate.

The blocker is at the **Schema identity layer** — the `Schema` objects that register validation rules for other kinds. Three places hardcode a `k.g` format that ignores version:

- `extract_schema_name` generates `Widget.example.io` and ignores `targetVersion` — the store rejects a second version as `AlreadyExists`
- `SchemaRegistry` cache key is `format!("{kind}.{group}")` — a second version would silently evict the first version's compiled validator
- OpenAPI component name is derived from the schema name — two versions collide on `components.schemas` even though they go to different URL paths

This change makes the schema identity, cache key, and component name all version-aware so users can register and use different versions of the same resource. Conversion hooks (translating objects between versions) are deliberately out of scope.

## What Changes

- **BREAKING**: Schema `metadata.name` format changes from `{kind}.{group}` to `{kind}.{group}.{version}`. Any Schema previously stored under the old format is no longer findable. Re-registration is required.
- Add a `schema_cache_key(kind, group, version) -> String` helper in `src/schema/mod.rs` to centralize the format
- `extract_schema_name` in `src/object/handler.rs` includes `targetVersion`
- `SchemaRegistry::get_validator` and `get_status_validator` cache keys include version; the status-validator store-lookup name (line 175) is fixed independently
- `build_openapi_spec` in `src/openapi/paths.rs` includes version in `schema_name`; description text updated
- Test fixtures updated: ~51 literal `"Widget.example.io"` → `"Widget.example.io.v1"` across 4 source files + integration tests
- New regression test: two versions of the same kind coexist, cache independently, validate independently, evict independently
- Documentation in `docs/` and `openspec/specs/` updated to reflect new format

## Capabilities

### New Capabilities

<!-- No new top-level capability — this is a behavior change within existing capabilities -->

### Modified Capabilities

- `schema-name-generation`: format changes from `{targetKind}.{targetGroup}` to `{targetKind}.{targetGroup}.{targetVersion}`
- `schema-registry`: cache key format includes version; status-validator store-lookup fixed to use the same key
- `openapi-spec`: component name derives from versioned schema name (`KindGroupVersion` pattern)
- `schema-service`: delete semantics unchanged, but test coverage for per-version deletion is added

## Impact

- **Code**: 5 source files modified (`handler.rs`, `registry.rs`, `paths.rs`, `schema/mod.rs` for new helper, plus test files). No new traits, no new abstractions, no store schema changes.
- **API**: Existing API contract preserved at the HTTP level. The `Schema` resource's `metadata.name` field is the only externally visible change. Routes, request/response shapes, error codes, and status codes unchanged.
- **Storage**: SQLite primary key for `Schema` objects is unchanged (the Schema is stored under `kapi.io/v1/Schema`; the `name` field is the only thing changing). No DDL migration needed.
- **Backward compat**: Pre-existing Schema records stored under the old name become invisible. Documented in commit message; no automated migration (pre-1.0 project, re-registration is trivial).

## Non-Goals

- **Conversion webhooks**: translating objects between versions (e.g., v2 spec on read, store as v1) is a separate future change
- **Versionless kind aggregation**: e.g., `GET /apis/example.io/Widget` returning objects across all versions — current version-required behavior preserved
- **`SchemaHasObjects` error enrichment**: the error currently only carries `kind`; adding `group`/`version` is a follow-up (council recommendation: punt to keep this change mechanical)
- **Cross-version validation or preference logic**: a request to one version validates against that version's schema only
