## Why

P0 established the project scaffold — module tree, dependencies, and a minimal server. Before any storage traits, services, or handlers can be implemented, the project needs its foundational types and error definitions. Without these, P2 (Storage Traits) and all subsequent phases are blocked. This change defines the domain types, error enum, and error response format that the rest of the system will build upon.

## What Changes

- Define `AppError` in `src/error.rs` with four variants: `NotFound`, `Conflict`, `SchemaValidation`, `Internal` — using `thiserror::Error` for ergonomic error handling
- Implement `IntoResponse` for `AppError` producing a richer JSON error body: `{ "error", "code", "details" }`
- Complete `ResourceKey` in `src/store/mod.rs` by adding `Serialize` and `Deserialize` derives (it currently only has `Debug, Clone, Hash, Eq, PartialEq`)
- Define all core object types in `src/object/types.rs`: `StoredObject`, `UserData`, `ListOptions`, `ListResponse`, `ContinueToken`, `WatchEvent`, `WatchEventType`
- Define schema types in `src/schema/types.rs`: `Schema`, `ValidationError`
- Remove unused `uuid` dependency from `Cargo.toml`
- **Backlog alignment**: Update roadmap.md to reflect that `ResourceKey` lives in `store/mod.rs` (not `object/types.rs`), that `NotFound` carries context, that `SchemaValidation` uses structured `ValidationError` objects, and that `UserData` and `ContinueToken` are new types introduced in P1

## Capabilities

### New Capabilities
- `error-handling`: Application-wide error enum, `thiserror` integration, and Axum `IntoResponse` mapping with structured JSON error bodies
- `core-types`: Shared domain types including `ResourceKey`, `StoredObject`, `Schema`, `UserData`, `ContinueToken`, `WatchEvent`, and `ValidationError`

### Modified Capabilities
<!-- No existing specs to modify -->

## Impact

- **Code**: Populates `src/error.rs`, `src/object/types.rs`, `src/schema/types.rs`; updates `src/store/mod.rs` and `Cargo.toml`
- **Dependencies**: Removes `uuid` (dead weight); all other deps from P0 remain
- **Build**: Must remain green — `cargo build` and `cargo test` pass after changes
- **API**: No API surface changes yet (no routes); these are purely internal types that P2-P5 will consume
- **Backlog**: Roadmap.md needs sync to match the evolved P1 scope
