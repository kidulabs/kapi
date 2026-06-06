## Context

The `ObjectStore` trait currently has a single monotonic counter: `resource_version`, which bumps on every update (spec, metadata, status). This is correct for watch ordering and CAS, but insufficient for controllers that need to know whether the **desired state** (spec) actually changed.

The roadmap already lists `generation` as Future Work. This change implements it.

## Goals / Non-Goals

**Goals:**
- Add `generation: u64` to `SystemMetadata`
- Generation starts at 1 on CREATE
- Generation bumps only when spec changes in `update()`
- Generation does NOT bump on `update_status()` or metadata-only updates
- Both stores implement this; integration test verifies compliance
- Trait documents the contract explicitly

**Non-Goals:**
- Metadata-only update endpoint (separate change)
- Annotations support (separate change, follows this one)
- `generation` exposed as a queryable/filterable field

## Decisions

### D1: Generation lives in `SystemMetadata`, not a separate field

`generation` is a server-maintained counter, like `resource_version`. It belongs in `SystemMetadata` alongside it. This keeps all server-controlled versioning in one place.

### D2: Store layer handles generation bump, not service

The store already reads the old object for the CAS check on `resource_version`. The spec comparison is free — it's one more field check in the same read. This avoids:
- An extra store read in the service layer
- Race conditions between get and update
- Duplicating logic across store implementations

The `ObjectStore` trait documents the contract: *"update() bumps generation iff spec differs."*

### D3: Spec comparison uses `serde_json::Value` structural equality

```rust
if old.spec.value != new.spec.value {
    generation += 1;
}
```

This is simple, correct, and handles all cases. False positives (structurally different but semantically equivalent JSON) are harmless — the controller reconciles unnecessarily. False negatives (missing a real change) are bad, and structural equality avoids them.

### D4: `update_status()` does NOT bump generation

Status is not desired state. Controllers care about spec drift, not status drift. The existing `update_status()` path already bypasses the normal `update()` flow, so generation naturally stays.

### D5: Generation is documented in the trait, enforced by integration tests

The `ObjectStore` trait doc comment specifies the generation contract. The integration test suite includes a test that:
1. Creates an object (gen=1)
2. Updates with same spec, different labels (gen stays 1)
3. Updates with different spec (gen=2)
4. Updates status (gen stays 2)
5. Updates with same spec, different labels again (gen stays 2)

This test runs against both InMemoryStore and SQLiteStore, catching any store that doesn't implement the contract.

## Risks / Trade-offs

| Risk | Mitigation |
|------|------------|
| Existing serialized objects lack `generation` field | `#[serde(default)]` on `generation` — deserializes as 0, but all new objects get 1. Migration: next update sets it to 1. |
| SQLite schema needs new column | `ALTER TABLE objects ADD COLUMN generation INTEGER NOT NULL DEFAULT 1` in init_schema. Existing databases get the column with default 1. |
| Spec comparison cost for large JSON payloads | Acceptable for v1. If needed later, could use a hash of spec for O(1) comparison. |
