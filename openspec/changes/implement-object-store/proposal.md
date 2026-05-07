## Why

The roadmap defines P2 as the storage layer — the `ObjectStore` trait and its in-memory implementation using `DashMap`. This is the foundation for all CRUD operations. Without it, services and handlers have nothing to persist or retrieve objects against. The types and error handling are already in place (P0-P1 complete); the next step is the storage abstraction and its first concrete implementation.

## What Changes

- Define the `ObjectStore` async trait with `create`, `get`, `list`, `update`, and `delete` methods
- Implement `InMemoryStore` backed by `DashMap<(ResourceKey, String), StoredObject>`
- Add a global monotonic `AtomicU64` version counter for optimistic concurrency
- Implement pagination in `list` (sort by name, continue tokens, limit)
- Implement optimistic concurrency checks in `update` (version mismatch → 409) and `delete` (optional version check)
- Write unit tests covering all operations

## Capabilities

### New Capabilities

- `object-store`: The `ObjectStore` async trait defining the storage contract (create, get, list, update, delete) and the `InMemoryStore` implementation with DashMap, atomic versioning, pagination, and optimistic concurrency
- `store-tests`: Unit test coverage for the storage layer verifying create/get/list/update/delete semantics including conflict detection and pagination

### Modified Capabilities

<!-- No existing specs change their requirements. Core types and error handling are already defined and will be consumed, not modified. -->

## Impact

- `src/store/mod.rs` — adds `ObjectStore` trait definition
- `src/store/memory.rs` — replaces `// TODO` with `InMemoryStore` implementation
- `Cargo.toml` — no new dependencies (all already present)
- `roadmap.md` — P2 tasks T13-T18 move from unchecked to complete
