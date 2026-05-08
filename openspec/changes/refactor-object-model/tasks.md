## 1. Add ObjectMetadata type

- [x] 1.1 Add `ObjectMetadata` struct in `src/object/types.rs` with fields `name: String`, `resource_version: u64`, `created_at: DateTime<Utc>`, `updated_at: DateTime<Utc>`
- [x] 1.2 Derive `Debug`, `Clone`, `Serialize`, `Deserialize` on `ObjectMetadata` with `#[serde(rename_all = "camelCase")]`

## 2. Refactor StoredObject

- [x] 2.1 Replace flat `name`, `resource_version`, `created_at`, `updated_at` fields with `metadata: ObjectMetadata` in `StoredObject`
- [x] 2.2 Update all references to `StoredObject` fields throughout `src/store/memory.rs` to use `metadata.*` accessors

## 3. Update ObjectStore trait

- [x] 3.1 Change `update` signature from `update(&self, key: &ResourceKey, name: &str, data: Value, expected_version: u64)` to `update(&self, object: StoredObject)`
- [x] 3.2 Change `delete` signature from `delete(&self, key: &ResourceKey, name: &str, expected_version: Option<u64>)` to `delete(&self, key: &ResourceKey, name: &str)`

## 4. Rewrite InMemoryStore implementation

- [x] 4.1 Rewrite `update` to extract `expected_version` from `object.metadata.resource_version`, perform OCC check, apply `object.data`, bump version, touch `updated_at`
- [x] 4.2 Rewrite `delete` to remove optional version check — unconditional removal
- [x] 4.3 Update `create` to construct `ObjectMetadata` instead of flat fields
- [x] 4.4 Update `get`, `list` to work with new `StoredObject` structure (field access changes)

## 5. Rewrite tests

- [x] 5.1 Rewrite `create_get_round_trip` test for new types
- [x] 5.2 Rewrite `create_duplicate_conflict` test
- [x] 5.3 Rewrite `get_missing_not_found` test
- [x] 5.4 Rewrite `list_sorted_by_name` test
- [x] 5.5 Rewrite `list_with_limit_and_continue_token` test
- [x] 5.6 Rewrite `list_continue_token_resumes` test
- [x] 5.7 Rewrite `update_correct_version_succeeds` test — use `StoredObject` with embedded version
- [x] 5.8 Rewrite `update_wrong_version_conflict` test — use `StoredObject` with stale version
- [x] 5.9 Rewrite `update_missing_not_found` test
- [x] 5.10 Rewrite `delete_returns_object_and_get_not_found` test — unconditional delete
- [x] 5.11 Rewrite `delete_none_version_succeeds` test — remove version param, simplify to unconditional delete
- [x] 5.12 Remove `delete_wrong_version_conflict_and_object_remains` test — no longer applicable (delete is unconditional)
- [x] 5.13 Remove `delete_matching_version_succeeds` test — merged into unconditional delete test
- [x] 5.14 Rewrite `delete_missing_not_found` test
- [x] 5.15 Rewrite `list_empty_key` test

## 6. Verify

- [x] 6.1 Run `cargo test` — all tests pass with no warnings
- [x] 6.2 Run `cargo build` — no compilation errors
