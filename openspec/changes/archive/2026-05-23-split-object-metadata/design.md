## Context

kapi's `StoredObject` currently has three fields: `key`, `metadata`, and `data`. The `ObjectMetadata` struct mixes user-controlled fields (`name`) with server-controlled fields (`resourceVersion`, `createdAt`, `updatedAt`). This design was sufficient when metadata was just `name`, but the roadmap calls for labels, and adding user-provided fields to `ObjectMetadata` would worsen the coupling. The split separates concerns before adding labels.

Current wire format:
```json
{
  "key": { "group": "example.io", "version": "v1", "kind": "Widget" },
  "metadata": {
    "name": "my-obj",
    "resourceVersion": 1,
    "createdAt": "2026-05-23T...",
    "updatedAt": "2026-05-23T..."
  },
  "data": { "value": { "color": "blue" } }
}
```

## Goals / Non-Goals

**Goals:**
- Split `ObjectMetadata` into `ObjectMeta` (user-controlled) and `SystemMetadata` (server-controlled)
- Maintain clear ownership boundary: clients set `metadata`, servers set `system`
- Preserve all existing behavior: OCC, pagination, watch, CRUD
- Make the codebase ready for labels to be added to `ObjectMeta` with minimal further change

**Non-Goals:**
- Adding label support (separate change)
- Adding annotations or other metadata fields (future)
- Changing the `ObjectStore` trait's async interface pattern
- Changing REST endpoint URLs or HTTP methods
- Kubernetes API compatibility

## Decisions

### D1: Struct layout — three-field `StoredObject`

**Decision**: `StoredObject` becomes `{ key, metadata, system, data }` where `metadata: ObjectMeta` and `system: SystemMetadata`.

**Alternative considered**: Embed `ObjectMeta` into `ObjectMetadata` with `#[serde(flatten)]` so the wire format stays flat. Rejected because `#[serde(flatten)]` has edge cases with unknown fields and makes deserialization stricter. The nested format is explicit and clear.

**Alternative considered**: Keep `ObjectMetadata` flat and add a typedef `type ObjectMeta = ObjectMetadata`. Rejected because it's cosmetic — it doesn't enforce the boundary at the type level, which is the whole point.

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ObjectMeta {
    pub name: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemMetadata {
    pub resource_version: u64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StoredObject {
    pub key: ResourceKey,
    pub metadata: ObjectMeta,
    pub system: SystemMetadata,
    pub data: UserData,
}
```

### D2: `ObjectStore::create()` signature change

**Decision**: Change from `create(&self, key: &ResourceKey, name: &str, data: Value)` to `create(&self, key: &ResourceKey, meta: ObjectMeta, data: Value)`.

**Rationale**: `name` was already one field of what will become `ObjectMeta`. Rather than adding label parameters later, passing `ObjectMeta` as a unit is cleaner now. The store assembles `SystemMetadata` internally — this is already the case for `resourceVersion`, `createdAt`, and `updatedAt`.

**Alternative considered**: Keep `create(key, name, data)` and extend later. Rejected — adding labels means either growing the parameter list or changing the signature anyway. Better to change once now.

### D3: Update request uses full `StoredObject`

**Decision**: The update handler continues to accept `StoredObject` as the request body. The client echoes back the full object from a GET response. The store uses `system.resourceVersion` for OCC and ignores `system.createdAt`/`system.updatedAt` (recalculating them), exactly as today.

**Rationale**: This matches k8s pattern (client reads, modifies, writes back). No new request type needed.

### D4: Wire format uses `system` as top-level key

**Decision**: The JSON key is `"system"`, not `"systemMetadata"` or `"sys"`.

**Rationale**: Short, clear, and unlikely to collide with user data fields. Matches the conciseness of `"key"`, `"metadata"`, `"data"`.

### D5: SQLite schema stays unchanged

**Decision**: The SQLite table columns (`resource_group`, `api_version`, `resource_kind`, `name`, `data`, `resource_version`, `created_at`, `updated_at`) do not change. The mapping between columns and struct fields adjusts — `name` maps to `metadata.name`, `resource_version` maps to `system.resourceVersion`, etc.

**Rationale**: The database schema is an implementation detail, not a promise. But there's no reason to change it — the columns are the same, just grouped differently in Rust structs.

### D6: `ObjectMeta` uses `serde(rename_all = "camelCase")` for consistency

**Decision**: Both `ObjectMeta` and `SystemMetadata` use `#[serde(rename_all = "camelCase")]`.

**Rationale**: `ObjectMeta` currently has only `name` (no camelCase effect), but when labels are added, `ObjectMeta` will have the same serialization style as the rest of the API. `SystemMetadata` already uses `camelCase` (inherited from current `ObjectMetadata`).

## Risks / Trade-offs

- **[Breaking API change]** → This is a breaking wire format change. Since kapi is pre-1.0 with no external clients, this is acceptable. No migration path needed.
- **[Broad touch surface]** → Every file that accesses `metadata.name` vs `metadata.resourceVersion` changes. The split is mechanical (s/\.metadata\.name/\.metadata\.name/ stays the same; s/\.metadata\.resource_version/\.system\.resource_version/ changes). Risk of missed access is mitigated by `cargo check` catching compile errors. 
- **[Test churn]** → Integration tests reference JSON paths like `metadata.resourceVersion`. All change to `system.resourceVersion`. Straightforward but numerous.

## Open Questions

_(None — the design is fully resolved from our exploration.)_