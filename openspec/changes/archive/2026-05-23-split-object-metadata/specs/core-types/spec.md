## MODIFIED Requirements

### Requirement: StoredObject represents a persisted custom object
The system SHALL define a `StoredObject` struct containing `key: ResourceKey`, `metadata: ObjectMeta`, `system: SystemMetadata`, and `data: UserData`. The `ObjectMeta` struct SHALL contain `name: String` and derive `Debug`, `Clone`, `Serialize`, and `Deserialize` with `#[serde(rename_all = "camelCase")]`. The `SystemMetadata` struct SHALL contain `resource_version: u64`, `created_at: DateTime<Utc>`, and `updated_at: DateTime<Utc>` and derive `Debug`, `Clone`, `Serialize`, and `Deserialize` with `#[serde(rename_all = "camelCase")]`. `StoredObject` SHALL derive `Debug`, `Clone`, `Serialize`, and `Deserialize`.

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
- **THEN** the JSON contains top-level keys `key`, `metadata`, `system`, and `data`
- **AND** `metadata` contains `name`
- **AND** `system` contains `resourceVersion`, `createdAt`, `updatedAt`

#### Scenario: StoredObject deserializes from JSON
- **WHEN** JSON with keys `key`, `metadata`, `system`, and `data` is deserialized
- **THEN** the resulting `StoredObject` has `metadata.name`, `system.resource_version`, `system.created_at`, and `system.updated_at` populated

### Requirement: ObjectMeta groups user-controlled metadata fields
The system SHALL define an `ObjectMeta` struct containing `name: String`. This struct SHALL derive `Debug`, `Clone`, `Serialize`, and `Deserialize` with `#[serde(rename_all = "camelCase")`. `ObjectMeta` represents the portion of object metadata that the client controls.

#### Scenario: ObjectMeta is part of StoredObject
- **WHEN** a `StoredObject` is constructed
- **THEN** it contains an `ObjectMeta` with `name`

#### Scenario: ObjectMeta serializes correctly
- **WHEN** an `ObjectMeta` is serialized
- **THEN** the JSON output is `{ "name": "..." }`

### Requirement: SystemMetadata groups server-managed lifecycle fields
The system SHALL define a `SystemMetadata` struct containing `resource_version: u64`, `created_at: DateTime<Utc>`, and `updated_at: DateTime<Utc>`. This struct SHALL derive `Debug`, `Clone`, `Serialize`, and `Deserialize` with `#[serde(rename_all = "camelCase")`. `SystemMetadata` represents the portion of object metadata that the server controls; clients read these values but do not set them on create (they may echo `resourceVersion` on update for optimistic concurrency).

#### Scenario: SystemMetadata is part of StoredObject
- **WHEN** a `StoredObject` is constructed by the store
- **THEN** it contains a `SystemMetadata` with `resource_version`, `created_at`, and `updated_at`

#### Scenario: SystemMetadata serializes with camelCase
- **WHEN** a `SystemMetadata` is serialized
- **THEN** the JSON field names are `resourceVersion`, `createdAt`, `updatedAt`

### Requirement: Core types derive standard traits
All public types defined for core types SHALL derive `Debug` and `Clone`. Types that cross API boundaries SHALL additionally derive `Serialize` and `Deserialize`. `ObjectMeta` and `SystemMetadata` SHALL each derive `Debug`, `Clone`, `Serialize`, and `Deserialize` with `#[serde(rename_all = "camelCase")]`.

#### Scenario: Serialization roundtrip
- **WHEN** a `StoredObject` is serialized to JSON and back
- **THEN** the resulting value SHALL equal the original

## REMOVED Requirements

### Requirement: ObjectMetadata groups server-managed lifecycle fields
**Reason**: Replaced by two separate structs: `ObjectMeta` (user-controlled) and `SystemMetadata` (server-controlled). This split establishes a clear boundary between client-owned and server-owned metadata.
**Migration**: Code referencing `ObjectMetadata` should use `ObjectMeta` for user-controlled fields and `SystemMetadata` for server-controlled fields. Access paths change: `obj.metadata.name` remains `obj.metadata.name`; `obj.metadata.resource_version` becomes `obj.system.resource_version`.