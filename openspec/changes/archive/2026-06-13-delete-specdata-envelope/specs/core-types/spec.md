## MODIFIED Requirements

### Requirement: StoredObject represents a persisted custom object
The system SHALL define a `StoredObject` struct containing `key: ResourceKey`, `metadata: ObjectMeta`, `system: SystemMetadata`, `spec: serde_json::Value`, and `status: Option<serde_json::Value>`. The `status` field SHALL be `None` for kinds without a `statusSchema` and `Some(Value)` for kinds with one. The `ObjectMeta` struct SHALL contain `name: String` and derive `Debug`, `Clone`, `Serialize`, and `Deserialize` with `#[serde(rename_all = "camelCase")]`. The `SystemMetadata` struct SHALL contain `resource_version: u64`, `created_at: DateTime<Utc>`, and `updated_at: DateTime<Utc>` and derive `Debug`, `Clone`, `Serialize`, and `Deserialize` with `#[serde(rename_all = "camelCase")]`. `StoredObject` SHALL derive `Debug`, `Clone`, `Serialize`, and `Deserialize`. The `spec` and `status` fields SHALL be the user-supplied JSON directly, with no wrapper envelope.

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
- **THEN** the JSON contains top-level keys `key`, `metadata`, `system`, `spec`, and `status`
- **AND** `metadata` contains `name`
- **AND** `system` contains `resourceVersion`, `createdAt`, `updatedAt`
- **AND** `spec` contains the user-supplied spec JSON directly (no `value` wrapper)
- **AND** `status` contains the user-supplied status JSON directly (no `value` wrapper) or `null`

#### Scenario: StoredObject serializes with status field
- **WHEN** a `StoredObject` with `status: Some(Value::Object({"phase": "Running"}))` is serialized to JSON
- **THEN** the JSON contains top-level keys `key`, `metadata`, `system`, `spec`, and `status`
- **AND** `status` contains `{"phase": "Running"}` (the inner value directly, no wrapper)

#### Scenario: StoredObject serializes with null status
- **WHEN** a `StoredObject` with `status: None` is serialized to JSON
- **THEN** the JSON contains top-level keys `key`, `metadata`, `system`, `spec`, and `status`
- **AND** `status` is `null`

#### Scenario: StoredObject deserializes from JSON
- **WHEN** JSON with keys `key`, `metadata`, `system`, and `spec` is deserialized
- **THEN** the resulting `StoredObject` has `metadata.name`, `system.resource_version`, `system.created_at`, and `system.updated_at` populated
- **AND** `spec` is the JSON value at the `spec` key directly (no `value` wrapper expected or required)

#### Scenario: StoredObject deserializes with status
- **WHEN** JSON with keys `key`, `metadata`, `system`, `spec`, and `status` is deserialized
- **THEN** the resulting `StoredObject` has `status` populated as `Some(Value)` containing the JSON at the `status` key directly

#### Scenario: StoredObject deserializes with null status
- **WHEN** JSON with `status: null` is deserialized
- **THEN** the resulting `StoredObject` has `status` as `None`

### Requirement: SpecData wraps raw JSON for extensibility
**Reason**: The `SpecData` envelope is being deleted. The wrapper contributed no methods, no extension fields, and created a wire-format asymmetry between request bodies (unwrapped) and response bodies (wrapped). The `spec` and `status` fields on `StoredObject` are now `serde_json::Value` directly.
**Migration**: Replace any code that constructs `SpecData { value: x }` with the value `x` directly. Replace any code that reads `.spec.value` or `.status.unwrap().value` with `.spec` or `.status.unwrap()`. The wire format on responses changes from `{"value": {...}}` to `{...}` for both `spec` and `status` fields.

## REMOVED Requirements

### Requirement: SpecData wraps raw JSON for extensibility
**Reason**: The `SpecData` envelope is being deleted. There is no longer a wrapper around `serde_json::Value` for spec/status payloads.
**Migration**: All sites that used `SpecData` now use `serde_json::Value` directly. All sites that used `SpecData { value: x }` now use `x` directly. All sites that used `.spec.value` or `.status.unwrap().value` now use `.spec` or `.status.unwrap()`.
