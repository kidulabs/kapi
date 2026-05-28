## Context

kapi objects currently carry only a `name` in `ObjectMeta`. Labels — arbitrary key-value string pairs — are the standard mechanism for organizing and selecting resources. This change adds labels as a first-class metadata field, with persistent storage in both backends and Kubernetes-compatible validation.

Current state:
- `ObjectMeta { name: String }` — single field, `#[serde(rename_all = "camelCase")]`
- SQLite: single `objects` table, no migration system, `CREATE TABLE IF NOT EXISTS` pattern
- Handler extracts `name` from `metadata.name`, strips `metadata` from body before service call
- Schema objects have a separate name-generation path (from `targetKind`/`targetGroup`)
- Update handler receives full `StoredObject`, validates URL matches body

## Goals / Non-Goals

**Goals:**
- Labels stored, validated, serialized, and round-tripped correctly on all objects
- SQLite labels stored in a separate table for future SQL-level filtering (Phase 3)
- Label validation follows Kubernetes semantics (key/value format, length limits)
- Labels work on both regular objects and Schema objects
- Label updates use diff-based strategy (read → diff → apply in transaction)
- OpenAPI spec and Swagger UI reflect the new `labels` field

**Non-Goals:**
- Label selector query parameters (Phase 2)
- Label/field filtering on list requests (Phase 3)
- Watch filter combinators (Phase 3)
- Annotations or other metadata extensions

## Decisions

### 1. Labels as `HashMap<String, String>` on ObjectMeta

**Decision:** `labels: HashMap<String, String>` field on `ObjectMeta`, always serialized (empty `{}` when no labels).

**Alternatives considered:**
- `BTreeMap<String, String>` — deterministic ordering, but HashMap is the K8s convention and ordering is irrelevant for labels
- `Option<HashMap<String, String>>` — avoids empty map allocation, but complicates serialization (need `skip_serializing_if`) and every consumer must handle `None`. Always-present is simpler.

**Rationale:** Matches K8s API shape. `HashMap` is the natural Rust type for string→string maps. Always-present avoids `Option` handling at every call site.

### 2. Separate `labels` table in SQLite

**Decision:** New `labels` table with composite PK `(resource_group, api_version, resource_kind, name, label_key)`, FK to `objects` with `ON DELETE CASCADE`.

**Alternatives considered:**
- JSON blob in `objects.data` — simpler schema, but makes SQL-level label filtering impossible (Phase 3 requirement)
- JSON column on `objects` table — same filtering problem, plus SQLite JSON functions are limited

**Rationale:** Separate table enables `WHERE EXISTS` subqueries for label-based filtering in Phase 3. The `ON DELETE CASCADE` ensures labels are cleaned up when objects are deleted. The composite PK enforces key uniqueness per object and provides an efficient index for label lookups.

```sql
CREATE TABLE IF NOT EXISTS labels (
    resource_group  TEXT NOT NULL,
    api_version     TEXT NOT NULL,
    resource_kind   TEXT NOT NULL,
    name            TEXT NOT NULL,
    label_key       TEXT NOT NULL,
    label_value     TEXT NOT NULL,
    PRIMARY KEY (resource_group, api_version, resource_kind, name, label_key),
    FOREIGN KEY (resource_group, api_version, resource_kind, name)
        REFERENCES objects(resource_group, api_version, resource_kind, name)
        ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_labels_gvkn ON labels(resource_group, api_version, resource_kind, name);
```

### 3. Diff-based label updates

**Decision:** On update, read existing labels, compute diff (to_delete, to_upsert), apply changes in a single transaction alongside the object update.

**Alternatives considered:**
- Delete-all-then-insert — simpler code, but writes unchanged labels unnecessarily
- Upsert-only with `INSERT OR REPLACE` — doesn't handle deletions without a separate pass

**Rationale:** Diff-based is semantically cleaner and writes only changed labels. For typical label counts (< 20), the performance difference is negligible, but the pattern scales better and is more auditable. The transaction ensures atomicity with the object update.

### 4. Kubernetes label validation semantics

**Decision:** Validate labels with K8s-compatible rules:
- **Keys:** max 256 chars, `[a-zA-Z0-9][-_.a-zA-Z0-9]*` with optional `/` separator for prefix (`prefix/name` format). Prefix: max 253 chars, DNS subdomain format.
- **Values:** max 256 chars, `[a-zA-Z0-9][-_.a-zA-Z0-9]*` or empty string.
- Non-empty keys required.

**Alternatives considered:**
- No validation — accepts garbage, makes label selectors unreliable
- Loose validation (any non-empty string) — simpler but diverges from K8s, causes surprises if users expect K8s compatibility
- Full K8s validation (63-char name limit, 253-char prefix, strict DNS regex) — more restrictive than needed for a non-K8s system

**Rationale:** Moderate K8s semantics: enough structure to be useful and predictable, without the full K8s strictness. The 256-char limit for both keys and values is generous but bounded. The `/` prefix separator is included because it's the standard K8s convention for namespaced label keys (e.g., `app.kubernetes.io/name`).

### 5. Label extraction in handler, validation in service

**Decision:** Handler extracts labels from `metadata.labels` (same as `metadata.name`). Service validates labels before persistence.

**Alternatives considered:**
- Validate in handler — mixes validation with extraction, breaks the handler=translation/service=business-logic pattern
- Validate in store — store shouldn't know about validation rules

**Rationale:** Follows the existing pattern: handlers extract, services validate, stores persist. Label validation is a business rule, not a storage concern.

### 6. Labels on Schema objects

**Decision:** Schema objects support labels. Label extraction is unified across both paths.

**Alternatives considered:**
- Labels only on regular objects — creates inconsistency, limits Schema organization

**Rationale:** Labels are a metadata concept, not a data concept. Schema objects benefit from labels (team ownership, lifecycle status). The handler refactor extracts labels once, regardless of kind.

## Risks / Trade-offs

- **[Schema change]** Adding `labels` table is additive and idempotent (`CREATE TABLE IF NOT EXISTS`), but existing databases won't have it until restart. → Mitigation: `init_schema()` runs on every `SQLiteStore::new()`, so the table is created on next startup.
- **[API contract change]** `ObjectMeta` serialization gains a `labels` field. Existing clients that don't send labels get `"labels": {}`. → Mitigation: This is additive, not breaking. Clients that ignore unknown fields are unaffected.
- **[Update complexity]** Diff-based label updates add code complexity vs delete-all-then-insert. → Mitigation: The diff logic is straightforward (set operations on HashMap keys), and the pattern is well-understood.
- **[Validation regex]** K8s label validation regex may reject labels users expect to work. → Mitigation: Error messages clearly indicate the format requirements. Future work can relax rules if needed.
