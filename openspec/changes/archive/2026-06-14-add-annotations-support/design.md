## Context

kapi currently supports labels on `ObjectMeta` with strict Kubernetes-style validation, dedicated SQLite table storage with indexes, and labelSelector query support. Labels serve selection/filtering use cases where clients need to query objects by key-value pairs.

The roadmap calls for annotations as a complementary metadata mechanism for arbitrary, non-queryable metadata. This follows the Kubernetes model where labels and annotations serve distinct purposes:
- **Labels**: Indexed, queryable, validated — for selection
- **Annotations**: Not indexed, not queryable, minimal validation — for arbitrary metadata

Current state:
- `ObjectMeta` has `name: String` and `labels: HashMap<String, String>`
- Labels use a separate `labels` table in SQLite with composite primary key
- Label validation enforces character restrictions, length limits, DNS subdomain prefixes
- InMemoryStore stores labels directly in `ObjectMeta`

## Goals / Non-Goals

**Goals:**
- Add `annotations: HashMap<String, String>` to `ObjectMeta` with minimal validation
- Store annotations efficiently without indexing overhead
- Maintain API consistency with labels (same wire format patterns)
- Support annotations on both regular objects and Schema objects
- Provide clear error messages for invalid annotations
- Ensure backward compatibility (existing objects without annotations work seamlessly)

**Non-Goals:**
- `annotationSelector` query parameter (annotations have no selection semantics by design)
- Separate `annotations` table with indexes (annotations are never queried in WHERE clauses)
- Strict character validation like labels (annotations accept arbitrary strings)
- Per-value size limits (256KB total cap is sufficient)
- System annotation prefix handling (`kapi.io/*`) — deferred to future work
- Migration tooling for existing objects (handled by `#[serde(default)]`)

## Decisions

### Decision 1: Minimal Validation Strategy

**Choice**: Non-empty keys (max 256 chars), any string values, 256KB total limit per object.

**Alternatives considered:**
1. **No validation at all**: Rejected — allows abuse (empty keys, massive payloads)
2. **Kubernetes-style validation (same as labels)**: Rejected — breaks legitimate use cases (JSON configs, URLs, base64 data contain characters label validation rejects)
3. **Per-value size limits (64KB or 256 chars)**: Rejected — total 256KB cap subsumes this, and per-value limits break `last-applied-configuration` patterns

**Rationale**: The roadmap explicitly says "no validation beyond key-value structure." Real-world annotations contain JSON, URLs, commit SHAs, base64 data — characters that label validation would reject. The 256KB total limit prevents abuse while allowing flexibility.

### Decision 2: JSON Column Storage (Not Separate Table)

**Choice**: Add `annotations TEXT` column to `objects` table, storing JSON-serialized `HashMap<String, String>`.

**Alternatives considered:**
1. **Separate `annotations` table mirroring labels**: Rejected — annotations are never queried in WHERE clauses, so indexing provides no benefit. Extra table means extra queries on read, extra complexity in transactions.
2. **Inline in ObjectMeta serialization**: Rejected — couples metadata structure to storage format, harder to evolve independently.

**Rationale**: Storage strategy follows from selection semantics. Labels get a separate table because of `EXISTS` subqueries for `labelSelector`. Annotations don't need this. The JSON column pattern is already used for `spec` and `status` fields. InMemoryStore needs no changes (stores `StoredObject` directly).

**Trade-offs:**
- ✅ Zero extra queries on read (annotations come with the object row)
- ✅ Simpler transaction logic (no separate table writes)
- ✅ Consistent with existing `spec`/`status` JSON column pattern
- ❌ Cannot query annotations later (but this is by design — no selection semantics)

### Decision 3: Validation Placement

**Choice**: Validate in service layer (`ObjectService`), mirroring label validation pattern.

**Rationale**: Labels are validated in `ObjectService::validate_labels()`, called in create/update paths. Annotations should follow the same pattern for consistency. Handler layer extracts raw data, service layer validates.

### Decision 4: Error Handling

**Choice**: Add `AppError::InvalidAnnotation(String)` variant returning HTTP 400 Bad Request.

**Rationale**: Mirrors `AppError::InvalidLabel(String)` pattern. Clear separation between label and annotation validation errors helps clients debug issues.

### Decision 5: Wire Format Consistency

**Choice**: `metadata.annotations` field, optional in requests, always present in responses (empty map if none).

**Rationale**: Mirrors labels wire format exactly. Clients already understand `metadata.labels`, so `metadata.annotations` follows the same pattern. Always serializing (even when empty) simplifies client code — no need to check for field presence.

## Risks / Trade-offs

**[Risk] Large annotation payloads impact performance** → **Mitigation**: 256KB total limit prevents extreme cases. Monitor annotation sizes in production. Consider adding metrics for annotation payload sizes.

**[Risk] JSON serialization overhead on every read/write** → **Mitigation**: Annotations are small in practice (typically <1KB). The overhead is negligible compared to network I/O. If it becomes a problem, we can optimize later (e.g., cache serialized form).

**[Trade-off] No annotation queries limits future flexibility** → **Mitigation**: This is by design. If query needs emerge, we can add a separate indexed field or migrate to a table later. Starting simple avoids over-engineering.

**[Trade-off] 256KB limit may be too restrictive for some use cases** → **Mitigation**: Matches Kubernetes convention. If specific use cases need more, we can increase the limit or make it configurable per-schema.

**[Risk] SQLite schema migration required** → **Mitigation**: `ALTER TABLE objects ADD COLUMN annotations TEXT` is non-blocking. Existing rows get `NULL`, which deserializes to empty map via `#[serde(default)]`. No data migration needed.

## Migration Plan

**Deployment:**
1. Add `annotations TEXT` column to SQLite schema (idempotent `CREATE TABLE IF NOT EXISTS` handles new deployments)
2. For existing databases: `ALTER TABLE objects ADD COLUMN annotations TEXT` (non-blocking)
3. Deploy new binary — existing objects work seamlessly via `#[serde(default)]`

**Rollback:**
1. Revert to previous binary — annotations column is ignored
2. No data loss — annotations are optional, existing code doesn't depend on them

**Testing:**
1. Integration tests verify create/update/get/list with annotations
2. Verify backward compatibility: objects without annotations work correctly
3. Verify SQLite migration: existing objects gain empty annotations map

## Open Questions

None — all design decisions are resolved. The council reached unanimous consensus on validation strategy, storage approach, and scope.
