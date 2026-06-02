## MODIFIED Requirements

### Requirement: StoredObject represents a persisted custom object
The system SHALL define a `StoredObject` struct containing `key: ResourceKey`, `metadata: ObjectMeta`, `system: SystemMetadata`, `spec: SpecData`, and `status: Option<SpecData>`. The `status` field SHALL be `None` for kinds without a `statusSchema` and `Some(SpecData)` for kinds with one. `StoredObject` SHALL derive `Debug`, `Clone`, `Serialize`, and `Deserialize`.

#### Scenario: StoredObject serializes with status field
- **WHEN** a `StoredObject` with `status: Some(SpecData { value: {"phase": "Running"} })` is serialized to JSON
- **THEN** the JSON contains top-level keys `key`, `metadata`, `system`, `spec`, and `status`
- **AND** `status` contains `{"value": {"phase": "Running"}}`

#### Scenario: StoredObject serializes with null status
- **WHEN** a `StoredObject` with `status: None` is serialized to JSON
- **THEN** the JSON contains top-level keys `key`, `metadata`, `system`, `spec`, and `status`
- **AND** `status` is `null`

#### Scenario: StoredObject deserializes with status
- **WHEN** JSON with keys `key`, `metadata`, `system`, `spec`, and `status` is deserialized
- **THEN** the resulting `StoredObject` has `status` populated as `Some(SpecData)`

#### Scenario: StoredObject deserializes with null status
- **WHEN** JSON with `status: null` is deserialized
- **THEN** the resulting `StoredObject` has `status` as `None`

### Requirement: SchemaData includes optional statusSchema
The system SHALL define `SchemaData` with fields `target_group: String`, `target_version: String`, `target_kind: String`, `json_schema: serde_json::Value`, and `status_schema: Option<serde_json::Value>`. The `status_schema` field SHALL use `#[serde(rename_all = "camelCase")]` serialization, producing `statusSchema` in JSON.

#### Scenario: SchemaData with statusSchema serializes correctly
- **WHEN** a `SchemaData` with `status_schema: Some({...})` is serialized
- **THEN** the JSON contains `"statusSchema": {...}` alongside `targetGroup`, `targetVersion`, `targetKind`, and `jsonSchema`

#### Scenario: SchemaData without statusSchema serializes correctly
- **WHEN** a `SchemaData` with `status_schema: None` is serialized
- **THEN** the JSON does not contain a `statusSchema` key (or it is `null`)

### Requirement: WatchEventType includes StatusModified
The system SHALL define `WatchEventType` as an enum with `Added`, `Modified`, `Deleted`, and `StatusModified` variants.

#### Scenario: StatusModified variant exists
- **WHEN** the `WatchEventType` enum is compiled
- **THEN** it SHALL have variants `Added`, `Modified`, `Deleted`, and `StatusModified`