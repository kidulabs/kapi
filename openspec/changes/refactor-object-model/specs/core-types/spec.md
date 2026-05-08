## MODIFIED Requirements

### Requirement: StoredObject represents a persisted custom object
The system SHALL define a `StoredObject` struct containing `key: ResourceKey`, `metadata: ObjectMetadata`, and `data: UserData`. The `ObjectMetadata` struct SHALL contain `name: String`, `resource_version: u64`, `created_at: DateTime<Utc>`, and `updated_at: DateTime<Utc>`. Both `StoredObject` and `ObjectMetadata` SHALL derive `Serialize` and `Deserialize` with `#[serde(rename_all = "camelCase")]` on `ObjectMetadata` for wire format compatibility.

#### Scenario: Object carries versioning timestamps
- **WHEN** an object is created or updated
- **THEN** `metadata.created_at` and `metadata.updated_at` SHALL be populated by the storage layer

#### Scenario: Resource version for optimistic concurrency
- **WHEN** an object is created or updated
- **THEN** `metadata.resource_version` SHALL be updated by the storage layer
- **WHEN** an update is performed with a stale `metadata.resource_version`
- **THEN** the storage layer SHALL reject the update, returning a `Conflict` error

#### Scenario: StoredObject serializes with camelCase metadata
- **WHEN** a `StoredObject` is serialized to JSON
- **THEN** the metadata fields appear as `resourceVersion`, `createdAt`, `updatedAt`

### Requirement: Core types derive standard traits
All public types defined in P1 SHALL derive `Debug` and `Clone`. Types that cross API boundaries SHALL additionally derive `Serialize` and `Deserialize`. `ObjectMetadata` SHALL derive `Debug`, `Clone`, `Serialize`, and `Deserialize` with `#[serde(rename_all = "camelCase")]`.

#### Scenario: Serialization roundtrip
- **WHEN** a `StoredObject` is serialized to JSON and back
- **THEN** the resulting value SHALL equal the original

#### Scenario: ObjectMetadata serializes with camelCase
- **WHEN** an `ObjectMetadata` is serialized to JSON
- **THEN** the field names are `name`, `resourceVersion`, `createdAt`, `updatedAt`

## ADDED Requirements

### Requirement: ObjectMetadata groups server-managed lifecycle fields
The system SHALL define an `ObjectMetadata` struct containing `name: String`, `resource_version: u64`, `created_at: DateTime<Utc>`, and `updated_at: DateTime<Utc>`. This struct SHALL be used as the `metadata` field of `StoredObject`. The struct SHALL derive `Debug`, `Clone`, `Serialize`, and `Deserialize` with `#[serde(rename_all = "camelCase")]`.

#### Scenario: ObjectMetadata is part of StoredObject
- **WHEN** a `StoredObject` is constructed
- **THEN** it contains an `ObjectMetadata` with `name`, `resource_version`, `created_at`, and `updated_at`

#### Scenario: ObjectMetadata serializes correctly
- **WHEN** `ObjectMetadata` is serialized
- **THEN** the JSON output uses camelCase field names
