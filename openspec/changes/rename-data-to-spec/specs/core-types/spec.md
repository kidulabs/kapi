## MODIFIED Requirements

### Requirement: StoredObject represents a persisted custom object
The system SHALL define a `StoredObject` struct containing `key: ResourceKey`, `metadata: ObjectMeta`, `system: SystemMetadata`, and `spec: SpecData`. The `ObjectMeta` struct SHALL contain `name: String` and derive `Debug`, `Clone`, `Serialize`, and `Deserialize` with `#[serde(rename_all = "camelCase")]`. The `SystemMetadata` struct SHALL contain `resource_version: u64`, `created_at: DateTime<Utc>`, and `updated_at: DateTime<Utc>` and derive `Debug`, `Clone`, `Serialize`, and `Deserialize` with `#[serde(rename_all = "camelCase")]`. `StoredObject` SHALL derive `Debug`, `Clone`, `Serialize`, and `Deserialize`.

#### Scenario: Object carries versioning timestamps
- **WHEN** an object is created or updated
- **THEN** `system.created_at` and `system.updated_at` SHALL be populated by the storage layer

#### Scenario: Resource version for optimistic concurrency
- **WHEN** an object is created or updated
- **THEN** `system.resource_version` SHALL be updated by the storage layer
- **WHEN** an update is performed with a stale `system.resource_version`
- **THEN** the storage layer SHALL reject the update, returning a `Conflict` error

#### Scenario: StoredObject serializes with correct field grouping
- **WHEN** a `StoredObject` is serialized to JSON
- **THEN** the JSON contains top-level keys `key`, `metadata`, `system`, and `spec`
- **AND** `metadata` contains `name`
- **AND** `system` contains `resourceVersion`, `createdAt`, `updatedAt`

#### Scenario: StoredObject deserializes from JSON
- **WHEN** JSON with keys `key`, `metadata`, `system`, and `spec` is deserialized
- **THEN** the resulting `StoredObject` has `metadata.name`, `system.resource_version`, `system.created_at`, and `system.updated_at` populated

### Requirement: SpecData wraps raw JSON for extensibility
The system SHALL define a `SpecData` named struct containing a single `value: serde_json::Value` field. This replaces the previous `UserData` type.

#### Scenario: Handler receives user JSON
- **WHEN** a handler deserializes a request body
- **THEN** the payload SHALL be wrapped in `SpecData { value: ... }` before passing to the service layer

### Requirement: Schema represented as StoredObject convention
Schemas SHALL be represented as `StoredObject` with `kind="Schema"` in group `"kapi.io"`, not as a separate `Schema` struct. The `StoredObject.spec` field SHALL hold a JSON Schema value for validation.

#### Scenario: Schema struct removed
- **WHEN** the codebase is compiled
- **THEN** `src/schema/types.rs` does not exist
- **AND** no `Schema` struct is defined outside of `StoredObject`

#### Scenario: Schema registration stores the raw JSON Schema
- **WHEN** a schema is registered
- **THEN** a `StoredObject` with `kind="Schema"` stores the raw JSON Schema value in its `spec` field for later validation

## RENAMED Requirements

### FROM: UserData wraps raw JSON for extensibility
### TO: SpecData wraps raw JSON for extensibility