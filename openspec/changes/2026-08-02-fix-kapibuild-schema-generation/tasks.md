# Tasks: Fix kapibuild schema generation for crates.io installs

## Implementation Tasks

- [ ] **Task 1: Update `write_helper_cargo_toml()`**
  - Remove the `kapi-core = { path = "..." }` line from the generated Cargo.toml
  - Remove the `workspace_root: &Path` parameter (no longer needed)
  - Update function signature and callers

- [ ] **Task 2: Update `generate_wrapper_code()`**
  - Remove `pub metadata: ObjectMeta` field from the wrapper struct
  - Remove the `key()` method (dead code in helper)
  - Keep only `schema_data()` method

- [ ] **Task 3: Update `write_helper_main_rs()`**
  - Remove `use kapi_core::{ObjectMeta, ResourceKey}` import from generated module code
  - Update the wrapper code generation to not include metadata field or key() method

- [ ] **Task 4: Update `create_helper_project()`**
  - Remove the `workspace_root: &Path` parameter
  - Update call to `write_helper_cargo_toml()` to not pass workspace_root

- [ ] **Task 5: Delete `workspace_root()` function**
  - Remove the entire function (lines 107-112 in generate.rs)
  - Remove the call in `cmd_api_generate()` that captures `let ws_root = workspace_root()`

- [ ] **Task 6: Update `cmd_api_generate()`**
  - Remove `let ws_root = workspace_root()` line
  - Update call to `create_helper_project()` to not pass `ws_root`

- [ ] **Task 7: Update tests**
  - Update `test_generate_wrapper_code_without_status` — remove assertions about metadata field and key() method
  - Update `test_generate_wrapper_code_with_status` — same updates
  - Add test verifying helper Cargo.toml does NOT contain kapi-core

## Verification Tasks

- [ ] **Task 8: Manual test with local build**
  - Run `cargo build -p kapibuild`
  - Run `kapibuild api generate` against a test project
  - Verify schemas are generated correctly
  - Verify the temp helper's Cargo.toml does not reference kapi-core

- [ ] **Task 9: Manual test with crates.io-style install**
  - Simulate crates.io install scenario (remove workspace context)
  - Verify `kapibuild api generate` works without workspace root

## Out of Scope

- Schema registration (`register_schema()`) — separate change
- Changes to `generate_type_file()` — types/ files unchanged
- Changes to schema output format
