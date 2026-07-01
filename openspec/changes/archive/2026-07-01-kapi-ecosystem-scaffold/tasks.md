## 1. Workspace Restructure

- [x] 1.1 Create `kapi-server/` directory structure: `mkdir -p kapi-server/src`
- [x] 1.2 Move all server source files from `src/` to `kapi-server/src/`: `mv src/* kapi-server/src/`
- [x] 1.3 Create `kapi-server/Cargo.toml` by copying root's `[package]` and `[dependencies]` sections, update name to `kapi-server`
- [x] 1.4 Update root `Cargo.toml` to pure workspace manifest: remove `[package]`, `[dependencies]`, `[dev-dependencies]`; keep only `[workspace]` with `members = ["kapi-core", "kapi-server", "kapi-server/tests", "kapi-client", "kapi-cli", "kapi-controller"]`
- [x] 1.5 Remove empty `src/` directory
- [x] 1.6 Verify `cargo check -p kapi-server` compiles successfully
- [x] 1.7 Run `cargo test --workspace` to ensure all tests pass after restructure

## 2. Extract kapi-core Crate

- [x] 2.1 Create `kapi-core/` directory structure: `mkdir -p kapi-core/src`
- [x] 2.2 Create `kapi-core/Cargo.toml` with minimal dependencies: `serde = { version = "1", features = ["derive"] }`, `serde_json = "1"`, `chrono = { version = "0.4", features = ["serde"] }`, `thiserror = "2"`
- [x] 2.3 Move `ResourceKey` from `kapi-server/src/store/mod.rs` to `kapi-core/src/key.rs`
- [x] 2.4 Move core types from `kapi-server/src/object/types.rs` to `kapi-core/src/`: `StoredObject`, `ObjectMeta`, `SystemMetadata`, `SchemaData`, `ValidationError`, `WatchEvent`, `WatchEventType`, `WatchFilter`, `FieldSelector`, `LabelSelector`, `LabelRequirement`, `ListOptions`, `ListResponse`, `ContinueToken`
- [x] 2.5 Create `kapi-core/src/error.rs` with `CoreError` enum: `InvalidFieldSelector(String)`, `InvalidLabelSelector(String)`
- [x] 2.6 Update `FieldSelector::parse()` to return `Result<WatchFilter, CoreError>` instead of `Result<WatchFilter, AppError>`
- [x] 2.7 Update `LabelSelector::parse()` to return `Result<WatchFilter, CoreError>` instead of `Result<WatchError, AppError>`
- [x] 2.8 Create `kapi-core/src/lib.rs` with `pub mod` declarations and re-exports for all public types
- [x] 2.9 Verify `cargo check -p kapi-core` compiles successfully
- [x] 2.10 Run `cargo test -p kapi-core` to ensure unit tests pass (if any)

## 3. Update kapi-server to Use kapi-core

- [x] 3.1 Add `kapi-core = { path = "../kapi-core" }` to `kapi-server/Cargo.toml` dependencies
- [x] 3.2 Implement `From<CoreError> for AppError` in `kapi-server/src/error.rs` to convert `CoreError::InvalidFieldSelector` and `CoreError::InvalidLabelSelector`
- [x] 3.3 Update `kapi-server/src/lib.rs` to re-export core types: `pub use kapi_core::{ResourceKey, StoredObject, ObjectMeta, SystemMetadata, WatchEvent, WatchEventType, WatchFilter, FieldSelector, LabelSelector, LabelRequirement, ListOptions, ListResponse, ContinueToken, SchemaData, ValidationError, CoreError};`
- [x] 3.4 Remove type definitions from `kapi-server/src/object/types.rs` (now in kapi-core), keep only server-specific types if any
- [x] 3.5 Remove `ResourceKey` definition from `kapi-server/src/store/mod.rs` (now in kapi-core)
- [x] 3.6 Update all internal imports in kapi-server to use `crate::` paths or `kapi_core::` as appropriate
- [x] 3.7 Verify `cargo check -p kapi-server` compiles successfully
- [x] 3.8 Run `cargo test --workspace` to ensure all tests pass

## 4. Create Placeholder Crates

- [x] 4.1 Create `kapi-client/` directory structure: `mkdir -p kapi-client/src`
- [x] 4.2 Create `kapi-client/Cargo.toml` with `name = "kapi-client"`, `version = "0.1.0"`, `edition = "2024"`, and `kapi-core = { path = "../kapi-core" }` dependency
- [x] 4.3 Create `kapi-client/src/lib.rs` with a comment: `// TODO: Implement HTTP client library`
- [x] 4.4 Create `kapi-cli/` directory structure: `mkdir -p kapi-cli/src`
- [x] 4.5 Create `kapi-cli/Cargo.toml` with `name = "kapi-cli"`, `version = "0.1.0"`, `edition = "2024"`, and `kapi-client = { path = "../kapi-client" }` dependency
- [x] 4.6 Create `kapi-cli/src/main.rs` with a stub `fn main() { println!("kapi CLI - not yet implemented"); }`
- [x] 4.7 Create `kapi-controller/` directory structure: `mkdir -p kapi-controller/src`
- [x] 4.8 Create `kapi-controller/Cargo.toml` with `name = "kapi-controller"`, `version = "0.1.0"`, `edition = "2024"`, and `kapi-client = { path = "../kapi-client" }` dependency
- [x] 4.9 Create `kapi-controller/src/lib.rs` with a comment: `// TODO: Implement controller-runtime SDK`
- [x] 4.10 Verify `cargo check --workspace` compiles successfully

## 5. Move Tests into kapi-server/tests

- [x] 5.1 Create `kapi-server/tests/` directory structure: `mkdir -p kapi-server/tests`
- [x] 5.2 Move all test files from root `tests/` to `kapi-server/tests/`: `mv tests/* kapi-server/tests/`
- [x] 5.3 Update `kapi-server/tests/Cargo.toml` to depend on `kapi-server` instead of root `kapi`: change `kapi = { path = ".." }` to `kapi-server = { path = ".." }`
- [x] 5.4 Update all imports in `kapi-server/tests/src/` from `kapi::` to `kapi_server::` (or use renamed import if needed)
- [x] 5.5 Remove empty root `tests/` directory
- [x] 5.6 Run `cargo test --workspace` to ensure all integration tests pass

## 6. Verification and Cleanup

- [x] 6.1 Run `cargo clippy --workspace --all-targets -- -D warnings` to ensure no clippy warnings
- [x] 6.2 Run `cargo fmt --all -- --check` to ensure formatting is correct
- [x] 6.3 Run `cargo build --workspace` to ensure full build succeeds
- [x] 6.4 Verify workspace structure: `ls -la` should show `kapi-core/`, `kapi-server/`, `kapi-client/`, `kapi-cli/`, `kapi-controller/`
- [x] 6.5 Verify no `src/` directory exists at root
- [x] 6.6 Verify no `tests/` directory exists at root (tests are now in `kapi-server/tests/`)
- [x] 6.7 Check that `cargo tree -p kapi-core` shows only serde, serde_json, chrono, thiserror dependencies

## 7. Documentation and Roadmap

- [x] 7.1 Check `docs/` directory for any architecture or structure documentation that needs updating
- [x] 7.2 Update `AGENTS.md` if it references the old `src/` structure or workspace layout
- [x] 7.3 Check `roadmap.md` and add items for: "Implement kapi-client HTTP client library", "Implement kapi-cli with full command coverage", "Implement kapi-controller controller-runtime SDK", "Add resource_version to ListResponse and implement watch resume"
- [x] 7.4 Update any README or project documentation to reflect the new workspace structure

## 8. Final Validation

- [x] 8.1 Run full test suite: `cargo test --workspace`
- [x] 8.2 Verify all workspace members are recognized: `cargo metadata --no-deps --format-version 1 | jq '.packages[].name'`
- [x] 8.3 Confirm no breaking changes to public API: types are re-exported from kapi-server
- [x] 8.4 Document the change in a commit message following conventional commits format
