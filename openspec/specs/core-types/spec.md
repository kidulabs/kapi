## Purpose

Define the core domain types that represent resources, objects, schemas, pagination, and watch events. These types serve as the foundation for all storage, service, and handler layers.

## Requirements

### Requirement: ResourceKey uniquely identifies a resource kind
The system SHALL define a `ResourceKey` struct with `group`, `version`, and `kind` fields that implements `Hash`, `Eq`, `Clone`, `Serialize`, and `Deserialize`.

#### Scenario: Key equality and hashing
- **WHEN** two `ResourceKey` values have identical `group`, `version`, and `kind`
- **THEN** they SHALL be equal and produce the same hash

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

### Requirement: ObjectMetadata groups server-managed lifecycle fields
The system SHALL define an `ObjectMetadata` struct containing `name: String`, `resource_version: u64`, `created_at: DateTime<Utc>`, and `updated_at: DateTime<Utc>`. This struct SHALL be used as the `metadata` field of `StoredObject`. The struct SHALL derive `Debug`, `Clone`, `Serialize`, and `Deserialize` with `#[serde(rename_all = "camelCase")]`.

#### Scenario: ObjectMetadata is part of StoredObject
- **WHEN** a `StoredObject` is constructed
- **THEN** it contains an `ObjectMetadata` with `name`, `resource_version`, `created_at`, and `updated_at`

#### Scenario: ObjectMetadata serializes correctly
- **WHEN** `ObjectMetadata` is serialized
- **THEN** the JSON output uses camelCase field names

### Requirement: UserData wraps raw JSON for extensibility
The system SHALL define a `UserData` named struct containing a single `value: serde_json::Value` field.

#### Scenario: Handler receives user JSON
- **WHEN** a handler deserializes a request body
- **THEN** the payload SHALL be wrapped in `UserData { value: ... }` before passing to the service layer

### Requirement: Schema represents a registered JSON Schema
The system SHALL define a `Schema` struct containing `key: ResourceKey`, `json_schema: serde_json::Value`, and `created_at: DateTime<Utc>`.

#### Scenario: Schema registration stores the raw JSON Schema
- **WHEN** a schema is registered via POST /schemas
- **THEN** the `json_schema` field SHALL hold the raw JSON Schema value for later validation

### Requirement: ValidationError carries structured validation failures
The system SHALL define a `ValidationError` struct with `path: String` and `message: String`.

#### Scenario: Mapping jsonschema output
- **WHEN** the `jsonschema` crate reports validation failures
- **THEN** each failure SHALL be mapped to `ValidationError { path, message }`

### Requirement: ListOptions and ListResponse support pagination
The system SHALL define `ListOptions` with `limit: Option<usize>` and `continue_token: Option<ContinueToken>`, and `ListResponse` with `items: Vec<StoredObject>` and `continue_token: Option<ContinueToken>`.

#### Scenario: Unpaginated list
- **WHEN** `ListOptions.limit` is `None`
- **THEN** the storage layer SHALL return all matching items

#### Scenario: Paginated list with continuation
- **WHEN** `ListOptions.continue_token` is provided
- **THEN** the storage layer SHALL resume listing from the encoded offset

### Requirement: ContinueToken is an opaque string newtype
The system SHALL define `ContinueToken(pub String)` to prevent accidental mixing of raw strings with pagination tokens.

#### Scenario: Token construction
- **WHEN** the storage layer creates a continuation token
- **THEN** it SHALL be wrapped in `ContinueToken` before returning to the client

### Requirement: WatchEvent supports real-time change notifications
The system SHALL define `WatchEventType` as an enum with `Added`, `Modified`, and `Deleted` variants, and `WatchEvent` as a struct with `event_type: WatchEventType` and `object: StoredObject`.

#### Scenario: Watch stream receives events
- **WHEN** an object is created, updated, or deleted
- **THEN** watchers SHALL receive a `WatchEvent` with the corresponding `WatchEventType` and the affected `StoredObject`

### Requirement: Core types derive standard traits
All public types defined in P1 SHALL derive `Debug` and `Clone`. Types that cross API boundaries SHALL additionally derive `Serialize` and `Deserialize`. `ObjectMetadata` SHALL derive `Debug`, `Clone`, `Serialize`, and `Deserialize` with `#[serde(rename_all = "camelCase")]`.

#### Scenario: Serialization roundtrip
- **WHEN** a `StoredObject` is serialized to JSON and back
- **THEN** the resulting value SHALL equal the original

#### Scenario: ObjectMetadata serializes with camelCase
- **WHEN** an `ObjectMetadata` is serialized to JSON
- **THEN** the field names are `name`, `resourceVersion`, `createdAt`, `updatedAt`
