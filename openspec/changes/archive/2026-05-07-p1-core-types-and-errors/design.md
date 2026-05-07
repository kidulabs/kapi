## Context

P0 established the project scaffold: `Cargo.toml`, module tree, empty `.rs` files, and a minimal Axum server on port 8080. The only type defined so far is `ResourceKey` in `src/store/mod.rs` (with `Debug, Clone, Hash, Eq, PartialEq` but missing `Serialize, Deserialize`). All other type definitions (`AppError`, `StoredObject`, `Schema`, `WatchEvent`, etc.) and the error response mapping are deferred to P1. P2 (Storage Traits) and P3-P9 depend on these foundations, so P1 is on the critical path.

## Goals / Non-Goals

**Goals:**

- Define a complete, coherent set of core domain types that P2-P9 can consume without further modification
- Establish `AppError` as the single application-wide error enum with `thiserror` derives and Axum `IntoResponse` mapping
- Design a rich, structured JSON error body that gives clients a stable `code` to switch on plus human-readable messages
- Remove the unused `uuid` dependency to keep `Cargo.toml` honest
- Align `roadmap.md` with the evolved P1 scope discovered during exploration

**Non-Goals:**

- Implementing storage traits (`SchemaStore`, `ObjectStore`) — P2
- Implementing `EventBus` — P3
- Implementing handlers, services, or routes — P4-P7
- Implementing OpenAPI derives — P8
- Integration tests — P9
- Adding new crate dependencies beyond what P0 already declared

## Decisions

### D1: `ResourceKey` stays in `src/store/mod.rs`

P0 placed `ResourceKey` in `store/mod.rs` to avoid circular dependencies between `schema` and `object`. P1 adds `Serialize` and `Deserialize` but keeps the location.

**Rationale:** Both `SchemaStore` and `ObjectStore` depend on it; `store` is the natural shared parent. Moving it to `object/types.rs` would force `schema` to import from `object`, which is architecturally awkward.

**Alternative considered:** Create a `common` or `types` module at crate root. Rejected as premature — `ResourceKey` is the only truly shared type in P1.

### D2: `AppError::NotFound` carries structured context

```rust
NotFound { what: String, identifier: String }
```

**Rationale:** A bare `NotFound` is ambiguous in logs and error responses. `what` + `identifier` produces clear messages like `"schema 'example.io/v1/Widget' not found"` and maps cleanly to the rich JSON error body.

**Alternative considered:** `NotFound(String)` as a single message. Rejected because structured fields are easier to format consistently in `IntoResponse`.

### D3: `AppError::Internal` wraps `anyhow::Error` with `#[from]`

```rust
Internal(#[from] anyhow::Error)
```

**Rationale:** This is the Rust ecosystem standard for "catch-all" internal errors. The `#[from]` impl lets any `anyhow::Result` convert automatically with `?`. The trade-off is that `AppError` becomes non-`PartialEq`, but `matches!` in tests is idiomatic and acceptable.

**Alternative considered:** Store a `String` inside `Internal` to preserve `PartialEq`. Rejected because it loses error chains and `anyhow` backtraces/context.

### D4: `SchemaValidation` uses `Vec<ValidationError>` with structured fields

```rust
struct ValidationError { path: String, message: String }
```

**Rationale:** `jsonschema` produces structured validation output (JSON pointers + messages). Mapping directly to `Vec<String>` would throw away useful metadata. Structured errors produce better client-facing messages and are easier to test.

**Alternative considered:** `Vec<String>` only. Rejected because it forces string parsing to extract field paths.

### D5: Rich JSON error body with `code` + `details`

```json
{
  "error": "human readable summary",
  "code": "NotFound",
  "details": { "what": "schema", "identifier": "example.io/v1/Widget" }
}
```

**Rationale:** Clients need a stable machine-readable `code` to switch on, while humans need a readable `error`. `details` carries variant-specific structured data. This is roughly Kubernetes-style but simpler.

**Alternative considered:** Flatten everything into top-level fields. Rejected because it makes the schema inconsistent across error variants.

### D6: `UserData` is a named struct wrapping `serde_json::Value`

```rust
struct UserData { value: serde_json::Value }
```

**Rationale:** Raw `serde_json::Value` is simple but not extensible. A named struct gives us room to add metadata fields later (e.g., system-managed annotations alongside user JSON) without changing every call site.

**Alternative considered:** Tuple newtype `UserData(pub serde_json::Value)`. Rejected because named fields are easier to extend and self-documenting.

### D7: `ContinueToken` is a simple string newtype

```rust
struct ContinueToken(pub String)
```

**Rationale:** Pagination tokens need to be opaque to clients but meaningful to the server. A newtype prevents accidentally passing arbitrary strings. The internal format is deferred to P2 (in-memory store can use `"offset:N"` or similar).

### D8: Remove `uuid` dependency now

**Rationale:** `uuid` was included in P0 but is not referenced in P1-P5 types. Keeping it is speculative technical debt. If a future phase needs it, we add it back with a concrete use case.

## Risks / Trade-offs

- **[Non-PartialEq `AppError`]** → `Internal(anyhow::Error)` prevents `PartialEq`. Mitigation: use `matches!` in tests; this is idiomatic in the Rust ecosystem.
- **[ResourceKey location divergence from roadmap]** → Future readers may expect it in `object/types.rs`. Mitigation: update `roadmap.md` as part of this change.
- **[UserData ergonomics]** → Every handler will construct `UserData { value: json }`. Mitigation: add a `From<serde_json::Value>` impl if construction becomes noisy.
- **[ContinueToken format not finalized]** → P2 may need to change the encoding. Mitigation: it's a newtype — the public API stays stable even if the internal format changes.

## Migration Plan

This is a greenfield type-definition change — no runtime migration needed. Rollback is `git revert`.

## Open Questions

- Should `WatchEventType` derive `Copy`? It has no data and is small; `Copy` would remove clone overhead in async streams. Decision: yes, derive both `Copy` and `Clone`.
- Should `object/types.rs` re-export `ResourceKey` for convenience? Decision: yes, `pub use crate::store::ResourceKey;` so consumers of object types don't need a separate `store` import.
