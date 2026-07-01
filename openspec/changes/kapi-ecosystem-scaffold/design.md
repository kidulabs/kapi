## Context

The kapi project currently has a single server crate at the workspace root with `src/` containing all server code. The `tests/` directory is a separate workspace member for integration tests at the root level. As the project evolves to include a CLI, HTTP client library, and controller-runtime SDK, the current structure creates asymmetry — the server is the root package while future tools would be workspace members.

The server defines core types (`StoredObject`, `WatchEvent`, `ResourceKey`, selectors) that will be needed by both the server and future client/controller crates. Currently these types live in the server crate which pulls in heavy dependencies (axum, rusqlite, tower, jsonschema). A client shouldn't need these dependencies.

## Goals / Non-Goals

**Goals:**
- Establish a symmetric workspace structure where all crates are peers
- Extract shared types into a lightweight `kapi-core` crate with minimal dependencies
- Create placeholder crates for `kapi-client`, `kapi-cli`, and `kapi-controller` to establish the ecosystem structure
- Maintain backward compatibility — existing server users see no API changes
- Ensure all existing tests pass after restructuring

**Non-Goals:**
- Implementing CLI commands (placeholder only)
- Implementing HTTP client methods (placeholder only)
- Implementing controller-runtime logic (placeholder only)
- Adding `resource_version` to `ListResponse` or watch resume logic
- Publishing crates to crates.io

## Decisions

### Decision 1: Move server to `kapi-server/` subdirectory

**Choice**: Relocate `src/` to `kapi-server/src/`, making the server a workspace member instead of the root package.

**Rationale**: 
- Symmetric workspace structure — all crates are peers
- Clear separation between workspace manifest (root) and server implementation
- Matches Rust ecosystem conventions (e.g., `kube-rs` has `kube-core`, `kube-client`, etc.)
- Future tools (CLI, controller) are naturally workspace members

**Alternatives considered**:
- Keep server at root, add tools as subdirectories: Creates asymmetry, server is "special"
- Separate repos for each crate: Adds cross-repo coordination overhead for a solo dev project

### Decision 2: Extract `kapi-core` with shared types

**Choice**: Create a lightweight `kapi-core` crate containing `ResourceKey`, `StoredObject`, `ObjectMeta`, `SystemMetadata`, `WatchEvent`, `WatchEventType`, `WatchFilter`, `FieldSelector`, `LabelSelector`, `ListOptions`, `ListResponse`, `ContinueToken`, `SchemaData`, `ValidationError`. Introduce `CoreError` enum for parsing errors.

**Rationale**:
- Client and controller crates need these types but shouldn't pull in axum/rusqlite
- `kapi-core` has minimal deps: `serde`, `serde_json`, `chrono`, `thiserror`
- Server re-exports types from `kapi-core`, maintaining backward compatibility
- `CoreError` keeps axum's `IntoResponse` out of core; server has thin `From<CoreError> for AppError` adapter

**Alternatives considered**:
- Re-export types from server crate: Forces client to pull heavy deps (rusqlite bundled C compilation!)
- Duplicate types in client crate: High drift risk, maintenance burden
- Keep parsing logic in server: Client can't construct selectors for filtering

### Decision 3: Placeholder crates with minimal structure

**Choice**: Create `kapi-client`, `kapi-cli`, `kapi-controller` as workspace members with empty `lib.rs` / `main.rs` and minimal `Cargo.toml` (just name, version, edition, path deps).

**Rationale**:
- Establishes the ecosystem structure without implementation
- Future explorations can fill in the placeholders
- Workspace `cargo check` validates the structure
- Low risk — no behavior changes

**Alternatives considered**:
- Implement CLI/client/controller now: Out of scope for this change, better explored separately
- Don't create placeholders yet: Misses the opportunity to establish structure, will need to revisit

### Decision 4: `CoreError` adapter pattern

**Choice**: Define `CoreError` in `kapi-core` with variants `InvalidFieldSelector(String)` and `InvalidLabelSelector(String)`. Server implements `From<CoreError> for AppError`.

**Rationale**:
- Keeps axum dependency out of `kapi-core`
- Server maintains its error handling strategy
- Thin adapter (~10 lines) is acceptable overhead
- Client can use `CoreError` directly or map to its own error type

**Alternatives considered**:
- Use `String` errors in core: Loses type safety
- Define full error hierarchy in core: Over-engineering for current needs
- Keep `AppError` in core: Pulls in axum dependency

### Decision 5: Move tests into `kapi-server/tests/`

**Choice**: Relocate root `tests/` to `kapi-server/tests/`, co-locating server integration tests with the server crate.

**Rationale**:
- Tests are server-specific — they test the server's API endpoints and behavior
- Co-location makes the server crate self-contained (source + tests)
- Clearer ownership — server tests belong with the server, not at the workspace root
- Matches Rust conventions where tests live alongside the code they test

**Alternatives considered**:
- Keep tests at root: Creates ambiguity about what the tests are for, especially as more crates are added
- Create separate test crates per workspace member: Over-engineering for current needs; can be done later if client/controller need their own test crates

## Risks / Trade-offs

**[Risk] Breaking existing imports** → Mitigation: Server re-exports all types from `kapi-core` with `pub use kapi_core::*;`. Existing code using `kapi::object::types::StoredObject` continues to work.

**[Risk] Circular dependencies** → Mitigation: Dependency graph is strictly layered: `kapi-core` ← `kapi-server`, `kapi-core` ← `kapi-client` ← `kapi-cli`/`kapi-controller`. No cycles.

**[Risk] Build times increase with more workspace members** → Mitigation: Workspace caching helps. Placeholder crates have zero deps, minimal build impact. If it becomes an issue, can use `cargo build -p kapi-server` to build specific packages.

**[Trade-off] More `Cargo.toml` files to maintain** → Acceptable for the clarity and modularity gained. Each crate has clear ownership and dependencies.

**[Trade-off] `CoreError` adapter adds indirection** → Minimal overhead (~10 lines). Worth it to keep core lightweight.
