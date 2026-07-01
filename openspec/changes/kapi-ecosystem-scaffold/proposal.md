## Why

The kapi project is evolving from a single server crate into an ecosystem of tools (CLI, client library, controller-runtime SDK). Currently, all code lives in the root `src/` directory with the server as the only package. This structure doesn't scale for multiple crates and creates asymmetry — the server is special while future tools would be workspace members. Restructuring now establishes a clean foundation before building the CLI and controller-runtime, avoiding painful refactors later.

## What Changes

- **Move server to `kapi-server/`**: Relocate `src/` to `kapi-server/src/`, making the server a peer workspace member instead of the root package
- **Move tests into `kapi-server/tests/`**: Relocate root `tests/` to `kapi-server/tests/`, co-locating server integration tests with the server crate
- **Convert root to pure workspace manifest**: Remove `[package]` and `[dependencies]` from root `Cargo.toml`, keep only `[workspace]` configuration
- **Extract `kapi-core` crate**: Create a lightweight shared types crate containing `StoredObject`, `WatchEvent`, `ResourceKey`, selectors, and other types needed by both server and future client/controller crates. Introduce `CoreError` to replace `AppError` in parsing logic (thin adapter in server)
- **Create placeholder crates**: Add `kapi-client`, `kapi-cli`, and `kapi-controller` as empty workspace members with minimal `Cargo.toml` and stub source files
- **Update test dependencies**: Change `kapi-server/tests/Cargo.toml` to depend on `kapi-server` instead of root `kapi`

## Capabilities

### New Capabilities
- `workspace-restructure`: Reorganize project into symmetric workspace with server, core, and placeholder crates
- `kapi-core-extraction`: Extract shared types into lightweight crate with minimal dependencies (serde, chrono, thiserror)

### Modified Capabilities
- `core-types`: Types move from server crate to `kapi-core` crate; `FieldSelector::parse()` and `LabelSelector::parse()` return `CoreError` instead of `AppError`
- `library-entry-points`: Server's public API re-exports types from `kapi-core` instead of defining them locally

## Impact

- **Code**: All server source files move from `src/` to `kapi-server/src/`. Integration tests move from `tests/` to `kapi-server/tests/`. Type definitions in `src/object/types.rs` and `src/store/mod.rs` (ResourceKey) move to `kapi-core/src/`
- **Dependencies**: Server gains `kapi-core = { path = "../kapi-core" }` dependency. `kapi-core` has minimal deps: `serde`, `serde_json`, `chrono`, `thiserror`
- **APIs**: No public API changes for server users — types are re-exported from `kapi-core`. Internal `AppError` gains `From<CoreError>` adapter
- **Tests**: `kapi-server/tests/Cargo.toml` dependency changes from `kapi` to `kapi-server`. All existing tests continue to pass
- **Build**: Workspace now has 6 members (kapi-core, kapi-server, kapi-server/tests, kapi-client, kapi-cli, kapi-controller)

## Non-goals

- Implementing CLI commands (placeholder only)
- Implementing HTTP client methods (placeholder only)
- Implementing controller-runtime logic (placeholder only)
- Adding `resource_version` to `ListResponse` or watch resume logic (deferred to controller-runtime phase)
- Publishing crates to crates.io (internal workspace only)

## Future Work

- Implement `kapi-client` HTTP client library (reqwest-based wrappers for CRUD, watch, schema, status)
- Implement `kapi-cli` with full command coverage (schema CRUD, object CRUD, watch, status)
- Implement `kapi-controller` controller-runtime SDK (Informer, WorkQueue, Controller trait)
- Add `resource_version` field to `ListResponse` and implement watch resume with ring buffer replay (prerequisite for correct Informer behavior)
