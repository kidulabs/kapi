## Context

The P1 and P2 phases are implemented with a flat `StoredObject` and an `ObjectStore` trait that takes `expected_version` as a separate parameter. The service, handler, and route layers are not yet written. This refactor changes the type shape and trait signatures before the upper layers are built, minimizing rework.

## Decisions

### 1. ObjectMetadata groups lifecycle fields

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectMetadata {
    pub name: String,
    pub resource_version: u64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
```

`name` moves from top-level into metadata because it is an instance identifier, not type identity. `key` stays top-level because it answers "what kind of thing is this?" — that's identity, not metadata.

### 2. StoredObject becomes a three-part structure

```rust
pub struct StoredObject {
    pub key: ResourceKey,          // identity
    pub metadata: ObjectMetadata,  // lifecycle
    pub data: UserData,            // domain
}
```

This makes the separation of concerns explicit. The handler maps between wire JSON and this internal shape. The store works with `StoredObject` directly.

### 3. update(StoredObject) — OCC from embedded version

The store implementation peeks at `object.metadata.resource_version` and compares it against the current stored version. The client does not need to understand this field — it just echoes back the metadata it received from the previous GET.

```rust
// Inside InMemoryStore::update
let expected = object.metadata.resource_version;
if guard.metadata.resource_version != expected {
    return Err(AppError::Conflict { expected, actual: guard.metadata.resource_version });
}
```

### 4. delete(key, name) — unconditional

No version parameter. Deletes are idempotent by nature. If conditional delete is needed later, it can be added without breaking the existing unconditional path (e.g., as a separate method or query param at the handler level).

### 5. Wire format uses camelCase

`#[serde(rename_all = "camelCase")]` on `ObjectMetadata` (and `ResourceKey`) ensures the JSON wire format uses `resourceVersion`, `createdAt`, `updatedAt`. This matches K8s conventions and is standard for JSON APIs.

### 6. User schemas validate data only

The meta-schema and user-registered schemas describe only the `data` portion. Metadata fields are injected by the server on create/update and are never part of schema validation. This means:

- Schema registration does not need to declare metadata fields
- Users cannot accidentally omit or misdeclare metadata in their schemas
- The server owns the full object contract; users own their domain data

## Migration Path

Since service/handler/route layers are not yet implemented, the migration is confined to:

1. `src/object/types.rs` — add `ObjectMetadata`, restructure `StoredObject`
2. `src/store/mod.rs` — update trait signatures
3. `src/store/memory.rs` — rewrite implementation and tests

No downstream code needs updating because nothing calls these APIs yet.

## Risks

- **Test rewrite**: All 16 existing tests in `memory.rs` must be rewritten. This is mechanical but requires care to preserve test coverage.
- **Wire format lock-in**: Once clients exist, changing the JSON shape becomes a breaking change. The camelCase convention and metadata structure should be stable.
