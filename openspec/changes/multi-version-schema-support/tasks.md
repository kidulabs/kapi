## 1. Helper function

- [ ] 1.1 Add `pub fn schema_cache_key(kind: &str, group: &str, version: &str) -> String` to `src/schema/mod.rs` that returns `format!("{}.{}.{}", kind, group, version)`. Include a brief doc comment and a unit test for the format.

## 2. Schema name generation

- [ ] 2.1 Update `extract_schema_name` in `src/object/handler.rs` to read `targetVersion` and return `schema_cache_key(target_kind, target_group, target_version)`. Return `None` if any of the three fields is missing or not a string, so the existing `InvalidSchema` error path still fires.

## 3. SchemaRegistry cache keys

- [ ] 3.1 Update `get_validator` in `src/schema/registry.rs` (around line 103) so the cache key uses `schema_cache_key(key.kind, key.group, key.version)` instead of `format!("{}.{}", key.kind, key.group)`. The `schema_name` used for the store lookup at line 113 already derives from `cache_key` — verify this still works with the longer key.
- [ ] 3.2 Update `get_status_validator` in `src/schema/registry.rs` to use `schema_cache_key(key.kind, key.group, key.version)` for BOTH the cache key (line 166) and the store lookup name (line 175). This is the critical gap council flagged: the two values were built independently and the store lookup would have stayed broken.
- [ ] 3.3 Update the status cache key format to `"{k}.{g}.{v}.status"` so it remains distinguishable from the spec cache key.

## 4. OpenAPI component naming

- [ ] 4.1 Update `build_openapi_spec` in `src/openapi/paths.rs` (line 60) to build `schema_name = format!("{}.{}.{}", target_kind, target_group, target_version)` and pass it to `component_name()`. The existing `component_name` helper splits on `.` and PascalCases each segment, so no transform changes are needed.
- [ ] 4.2 Update the `description` text strings in `src/openapi/paths.rs` (around lines 192 and 219) that reference the old `Widget.example.io` example to the new `Widget.example.io.v1` form.

## 5. Test fixture updates

- [ ] 5.1 Replace literal `"Widget.example.io"` strings with `"Widget.example.io.v1"` in test fixtures across `src/schema/registry.rs`, `src/object/service.rs`, `src/object/handler.rs`, `src/object/schema_service.rs`, and the integration tests under `tests/`. Use a consistent find-and-replace — there are ~51 occurrences.
- [ ] 5.2 Update docstring examples and test doc-comments that reference the old format in the same files.

## 6. Multi-version regression test

- [ ] 6.1 Add a new test in `src/object/service.rs` (or a new `src/schema/registry.rs` test) that proves the central invariant: two Schemas with the same `(targetKind, targetGroup)` but different `targetVersion` register successfully, cache independently under distinct keys, validate independently (a payload satisfying v1 but not v2 is accepted at v1 and rejected at v2), and evict independently (deleting v1 leaves v2's cache entries intact). Cover both spec and status validators.
- [ ] 6.2 Add a per-version deletion test in `src/object/schema_service.rs` that registers `example.io/v1/Widget` and `example.io/v2/Widget` schemas, creates an object at v2, then deletes the v1 schema — confirm v1 deletion succeeds (no `SchemaHasObjects` because the existing object is at v2) and v2's cache entry is untouched.

## 7. Build verification

- [ ] 7.1 Run `cargo check` and confirm no compile errors.
- [ ] 7.2 Run `cargo clippy --all-targets -- -D warnings` and resolve any new lints.
- [ ] 7.3 Run the full test suite (`cargo test --workspace`) and confirm all tests pass, including the new multi-version regression test.

## 8. Documentation updates

- [ ] 8.1 Update `docs/api-reference.md` line 53 (the `metadata.name` example in the response shape) and line 93 (the prose "e.g. `Widget.example.io`") to use the new `Widget.example.io.v1` form. Add a brief note that the name includes the version.
- [ ] 8.2 Update `docs/storage.md` line 217 (the `SchemaRegistry` cache key description) to state the key format is `{targetKind}.{targetGroup}.{targetVersion}` and that two versions of the same kind occupy independent entries.
- [ ] 8.3 Update `openspec/specs/schema-name-generation/spec.md`, `openspec/specs/schema-registry/spec.md`, `openspec/specs/schema-service/spec.md`, and `openspec/specs/openapi-spec/spec.md` to reflect the versioned format — these are the source-of-truth spec files, not the delta files in this change. (The deltas in this change already describe the new behavior; after archive the deltas are merged into the base specs.)

## 9. Roadmap

- [ ] 9.1 Add a new entry to `roadmap.md` under "Future Work" titled "Version conversion webhooks" with a short description covering the conversion-hook exploration (translating objects between registered versions of the same kind). This is the natural follow-up that council and the user both deferred to a future change.

## 10. Commit and changelog

- [ ] 10.1 Commit the change with a `feat!:` conventional-commit prefix (breaking change to `Schema.metadata.name` format). The commit body SHALL call out the breaking change prominently and instruct operators to re-register any existing Schemas.
- [ ] 10.2 Verify `openspec status --change "multi-version-schema-support"` reports the change is complete and apply-ready.
