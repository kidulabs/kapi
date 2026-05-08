## 1. Add ObjectMetadata type

- [ ] 1.1 Add `ObjectMetadata` struct in `src/object/types.rs` with fields `name: String`, `resource_version: u64`, `created_at: DateTime<Utc>`, `updated_at: DateTime<Utc>`
- [ ] 1.2 Derive `Debug`, `Clone`, `Serialize`, `Deserialize` on `ObjectMetadata` with `#[serde(rename_all = "camelCase")]`

## 2. Refactor StoredObject

- [ ] 2.1 Replace flat `name`, `resource_version`, `created_at`, `updated_at` fields with `metadata: ObjectMetadata` in `StoredObject`
- [ ] 2.2 Update all references to `StoredObject` fields throughout `src/store/memory.rs` to use `metadata.*` accessors

## 3. Update ObjectStore trait

- [ ] 3.1 Change `update` signature from `update(&self, key: &ResourceKey, name: &str, data: Value, expected_version: u64)` to `update(&self, object: StoredObject)`
- [ ] 3.2 Change `delete` signature from `delete(&self, key: &ResourceKey, name: &str, expected_version: Option<u64>)` to `delete(&self, key: &ResourceKey, name: &str)`

## 4. Rewrite InMemoryStore implementation

- [ ] 4.1 Rewrite `update` to extract `expected_version` from `object.metadata.resource_version`, perform OCC check, apply `object.data`, bump version, touch `updated_at`
- [ ] 4.2 Rewrite `delete` to remove optional version check — unconditional removal
- [ ] 4.3 Update `create` to construct `ObjectMetadata` instead of flat fields
- [ ] 4.4 Update `get`, `list` to work with new `StoredObject` structure (field access changes)

## 5. Rewrite tests

- [ ] 5.1 Rewrite `create_get_round_trip` test for new types
- [ ] 5.2 Rewrite `create_duplicate_conflict` test
- [ ] 5.3 Rewrite `get_missing_not_found` test
- [ ] 5.4 Rewrite `list_sorted_by_name` test
- [ ] 5.5 Rewrite `list_with_limit_and_continue_token` test
- [ ] 5.6 Rewrite `list_continue_token_resumes` test
- [ ] 5.7 Rewrite `update_correct_version_succeeds` test — use `StoredObject` with embedded version
- [ ] 5.8 Rewrite `update_wrong_version_conflict` test — use `StoredObject` with stale version
- [ ] 5.9 Rewrite `update_missing_not_found` test
- [ ] 5.10 Rewrite `delete_returns_object_and_get_not_found` test — unconditional delete
- [ ] 5.11 Rewrite `delete_none_version_succeeds` test — remove version param, simplify to unconditional delete
- [ ] 5.12 Remove `delete_wrong_version_conflict_and_object_remains` test — no longer applicable (delete is unconditional)
- [ ] 5.13 Remove `delete_matching_version_succeeds` test — merged into unconditional delete test
- [ ] 5.14 Rewrite `delete_missing_not_found` test
- [ ] 5.15 Rewrite `list_empty_key` test

## 6. Verify

- [ ] 6.1 Run `cargo test` — all tests pass with no warnings
- [ ] 6.2 Run `cargo build` — no compilation errors
