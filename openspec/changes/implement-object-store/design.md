## Context

The project has completed P0 (scaffold) and P1 (core types + error handling). `ResourceKey`, `StoredObject`, `ListOptions`, `ListResponse`, `WatchEvent`, and `AppError` are all defined. The `src/store/mod.rs` file contains only `ResourceKey` and a `pub mod memory`. The `src/store/memory.rs` file is a `// TODO` stub.

The roadmap architecture calls for a single `ObjectStore` trait that handles all objects including schemas. Storage is pluggable via this trait; v1 is in-memory using `DashMap`.

## Goals / Non-Goals

**Goals:**
- Define a clean `ObjectStore` async trait that serves as the sole storage contract
- Implement `InMemoryStore` as the v1 backend using `DashMap` and `AtomicU64`
- Support optimistic concurrency control (version mismatch → 409)
- Support pagination in list (sort, limit, continue tokens)
- Provide full unit test coverage for all operations

**Non-Goals:**
- Persistent storage backends (SQLite, Postgres) — deferred to later phases
- Multi-node clustering or consensus
- Transaction isolation beyond per-key atomicity
- Schema-specific operations (schemas are just objects with kind="Schema")

## Decisions

### D1: Trait takes `serde_json::Value`, not `UserData`

The `ObjectStore` trait methods accept `Value` for the `data` parameter. The wrapping to `UserData` happens inside the implementation. This keeps callers (handlers, services) ignorant of the `UserData` wrapper type — it's an implementation detail of how objects are stored, not a concern of the storage contract.

Alternative considered: take `UserData` directly. Rejected because it leaks the internal wrapper into every caller and couples the trait to a specific type structure.

### D2: Map value is `StoredObject` directly (no `ObjectEntry` alias)

The `DashMap` stores `(ResourceKey, String) → StoredObject`. The roadmap notation mentioned `ObjectEntry` but this is just `StoredObject`. The duplication of `key` and `name` (also present in the map key tuple) is harmless and simplifies returning `StoredObject` without reconstruction.

### D3: `delete` returns the deleted object

The `delete` method returns `Result<StoredObject, AppError>`, cloning the object before removing it from the map. This enables the service layer to publish `Deleted` watch events with full object data and confirm to callers what was deleted.

### D4: Continue tokens are base64-encoded object names

Pagination in `list` sorts entries by name, skips past the continue token's decoded name, takes up to `limit` items, and encodes the last returned name as the next continue token. The base64 encoding provides opacity without requiring a separate token registry.

### D5: `AtomicU64` with `Relaxed` ordering for version counter

The version counter uses `fetch_add(1, Ordering::Relaxed)`. Strict ordering is unnecessary because versions only need to be unique and monotonically increasing — they don't coordinate with other memory operations. `DashMap` provides the per-key synchronization; the counter is fire-and-forget.

### D6: `update` requires exact version match; `delete` makes it optional

`update(key, name, data, expected_version)` always checks that the stored object's `resource_version` matches `expected_version`, returning `Conflict` on mismatch. `delete(key, name, expected_version)` accepts `Option<u64>` — if `Some`, it checks; if `None`, it deletes unconditionally. This matches the roadmap design where delete's version check is optional.

## Risks / Trade-offs

| Risk | Mitigation |
|------|------------|
| `DashMap` shard collisions under high contention | Acceptable for v1 (in-memory dev store). Persistent backends will have their own concurrency models. |
| `Relaxed` ordering could theoretically produce non-monotonic versions on exotic architectures | x86 and ARM guarantee store visibility for atomic operations. If targeting unusual architectures, switch to `SeqCst`. |
| Continue token encodes the object name (opaque but not encrypted) | Tokens are short-lived and only meaningful within the same list call sequence. No security implication. |
| `delete` clones the full `StoredObject` before removal | Objects are small (JSON payload + metadata). Clone cost is negligible compared to the benefit of returning the deleted object. |
