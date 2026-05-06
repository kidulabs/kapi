## 1. Dependency Cleanup

- [ ] 1.1 Remove `uuid` dependency from `Cargo.toml`
- [ ] 1.2 Verify `cargo build` still succeeds after removal

## 2. ResourceKey Completion

- [ ] 2.1 Add `Serialize` and `Deserialize` derives to `ResourceKey` in `src/store/mod.rs`
- [ ] 2.2 Re-export `ResourceKey` from `src/object/types.rs` via `pub use crate::store::ResourceKey;`

## 3. Error Types and Mapping

- [ ] 3.1 Define `AppError` enum in `src/error.rs` with variants: `NotFound { what, identifier }`, `Conflict { expected, actual }`, `SchemaValidation(Vec<ValidationError>)`, `Internal(#[from] anyhow::Error)`
- [ ] 3.2 Derive `thiserror::Error` and `Debug` for `AppError`
- [ ] 3.3 Implement `axum::response::IntoResponse` for `AppError` mapping: `NotFound` → 404, `Conflict` → 409, `SchemaValidation` → 422, `Internal` → 500
- [ ] 3.4 Ensure error response JSON body uses the rich format: `{ "error", "code", "details" }`
- [ ] 3.5 Ensure `Internal` details are `null` (do not leak internal error details)

## 4. Object Types

- [ ] 4.1 Define `UserData` named struct with `value: serde_json::Value` in `src/object/types.rs`
- [ ] 4.2 Define `StoredObject` in `src/object/types.rs` with fields: `key`, `name`, `data: UserData`, `version: u64`, `created_at`, `updated_at`
- [ ] 4.3 Define `ContinueToken` newtype `pub struct ContinueToken(pub String)` in `src/object/types.rs`
- [ ] 4.4 Define `ListOptions` with `limit: Option<usize>` and `continue_token: Option<ContinueToken>`
- [ ] 4.5 Define `ListResponse` with `items: Vec<StoredObject>` and `continue_token: Option<ContinueToken>`
- [ ] 4.6 Define `WatchEventType` enum with `Added`, `Modified`, `Deleted` — derive `Copy`, `Clone`, `Debug`, `Serialize`, `Deserialize`
- [ ] 4.7 Define `WatchEvent` with `event_type: WatchEventType` and `object: StoredObject`
- [ ] 4.8 Add required derives to all object types: `Debug`, `Clone`, `Serialize`, `Deserialize`

## 5. Schema Types

- [ ] 5.1 Define `Schema` in `src/schema/types.rs` with `key: ResourceKey`, `json_schema: serde_json::Value`, `created_at: DateTime<Utc>`
- [ ] 5.2 Define `ValidationError` in `src/schema/types.rs` with `path: String` and `message: String`
- [ ] 5.3 Add required derives to schema types: `Debug`, `Clone`, `Serialize`, `Deserialize`

## 6. Type Integration

- [ ] 6.1 Ensure `src/object/types.rs` compiles with all types and `ResourceKey` re-export
- [ ] 6.2 Ensure `src/schema/types.rs` compiles with `Schema`, `ValidationError`, and `use crate::store::ResourceKey`
- [ ] 6.3 Ensure `src/error.rs` compiles with `AppError` and `use crate::schema::types::ValidationError`
- [ ] 6.4 Ensure `src/lib.rs` exports are correct (no new exports needed yet, but verify modules compile)

## 7. Backlog Alignment

- [ ] 7.1 Update `roadmap.md` P1 section: change T8 to indicate `ResourceKey` is completed/enhanced in `src/store/mod.rs` (not created in `object/types.rs`)
- [ ] 7.2 Update `roadmap.md` P1 section: revise T6/T7 to reflect `NotFound` carries `{ what, identifier }`
- [ ] 7.3 Update `roadmap.md` P1 section: revise T6/T7 to reflect `SchemaValidation` uses `Vec<ValidationError>`
- [ ] 7.4 Update `roadmap.md` P1 section: add `UserData`, `ContinueToken`, and `ValidationError` as new types introduced in P1
- [ ] 7.5 Update `roadmap.md` dependencies section: remove `uuid` from the dependency list

## 8. Verification

- [ ] 8.1 Run `cargo build` and confirm no warnings or errors
- [ ] 8.2 Run `cargo test` and confirm baseline test passes
- [ ] 8.3 Run `cargo doc --no-deps` and confirm documentation generates without errors
- [ ] 8.4 Verify `cargo clippy` passes (if available)
