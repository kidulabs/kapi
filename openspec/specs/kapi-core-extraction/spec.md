### Requirement: kapi-core crate exists with minimal dependencies
The `kapi-core` crate SHALL exist at `kapi-core/` with its own `Cargo.toml`. It SHALL depend only on `serde`, `serde_json`, `chrono`, and `thiserror`. It SHALL NOT depend on `axum`, `rusqlite`, `tower`, `jsonschema`, `dashmap`, or any other server-specific crate.

#### Scenario: kapi-core Cargo.toml has minimal deps
- **WHEN** inspecting `kapi-core/Cargo.toml`
- **THEN** the `[dependencies]` section SHALL contain only `serde`, `serde_json`, `chrono`, and `thiserror`
- **AND** it SHALL NOT contain `axum`, `rusqlite`, `tower`, `jsonschema`, or `dashmap`

### Requirement: Core types defined in kapi-core
The `kapi-core` crate SHALL define the following types: `ResourceKey`, `StoredObject`, `ObjectMeta`, `SystemMetadata`, `WatchEvent`, `WatchEventType`, `WatchFilter`, `FieldSelector`, `LabelSelector`, `LabelRequirement`, `ListOptions`, `ListResponse`, `ContinueToken`, `SchemaData`, `ValidationError`. These types SHALL be moved from the server crate.

#### Scenario: ResourceKey in kapi-core
- **WHEN** inspecting `kapi-core/src/`
- **THEN** `ResourceKey` SHALL be defined in `kapi-core` (e.g., `kapi-core/src/key.rs` or similar)

#### Scenario: StoredObject and related types in kapi-core
- **WHEN** inspecting `kapi-core/src/`
- **THEN** `StoredObject`, `ObjectMeta`, `SystemMetadata`, `SchemaData`, `ValidationError` SHALL be defined in `kapi-core`

#### Scenario: Watch types in kapi-core
- **WHEN** inspecting `kapi-core/src/`
- **THEN** `WatchEvent`, `WatchEventType`, `WatchFilter`, `FieldSelector`, `LabelSelector`, `LabelRequirement` SHALL be defined in `kapi-core`

#### Scenario: List types in kapi-core
- **WHEN** inspecting `kapi-core/src/`
- **THEN** `ListOptions`, `ListResponse`, `ContinueToken` SHALL be defined in `kapi-core`

### Requirement: FieldSelector and LabelSelector parse errors
The `FieldSelector::parse()` and `LabelSelector::parse()` methods SHALL return `Result<_, ApiError>` instead of `Result<_, AppError>`. Parse errors SHALL be represented as `ApiError::InvalidRequest` with appropriate `what` fields.

#### Scenario: FieldSelector::parse returns ApiError
- **WHEN** `FieldSelector::parse()` is called with invalid input
- **THEN** it SHALL return `Err(ApiError::InvalidRequest { what: "field selector", message })`

#### Scenario: LabelSelector::parse returns ApiError
- **WHEN** `LabelSelector::parse()` is called with invalid input
- **THEN** it SHALL return `Err(ApiError::InvalidRequest { what: "label selector", message })`

**Note**: The `CoreError` type has been removed. Parse errors now use `ApiError::InvalidRequest` directly, eliminating the need for `From<CoreError> for AppError` conversion.

### Requirement: Server re-exports core types
The `kapi-server` crate SHALL re-export all public types from `kapi-core` to maintain backward compatibility. Existing code using `kapi::object::types::StoredObject` SHALL continue to work.

#### Scenario: Server re-exports StoredObject
- **WHEN** code imports `kapi_server::StoredObject` (or equivalent re-export path)
- **THEN** it SHALL resolve to `kapi_core::StoredObject`

#### Scenario: Server re-exports ResourceKey
- **WHEN** code imports `kapi_server::ResourceKey` (or equivalent re-export path)
- **THEN** it SHALL resolve to `kapi_core::ResourceKey`

### Requirement: kapi-core compiles independently
The `kapi-core` crate SHALL compile successfully with `cargo check -p kapi-core` without requiring any server dependencies.

#### Scenario: kapi-core compiles standalone
- **WHEN** running `cargo check -p kapi-core`
- **THEN** compilation SHALL succeed with no errors

#### Scenario: kapi-core tests pass
- **WHEN** running `cargo test -p kapi-core`
- **THEN** all unit tests in `kapi-core` SHALL pass (if any exist)
