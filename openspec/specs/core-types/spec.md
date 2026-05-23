## Purpose

Define the core domain types that represent resources, objects, schemas, pagination, and watch events. These types serve as the foundation for all storage, service, and handler layers.

## Requirements

### Requirement: ResourceKey uniquely identifies a resource kind
The system SHALL define a `ResourceKey` struct with `group`, `version`, and `kind` fields that implements `Hash`, `Eq`, `Clone`, `Serialize`, and `Deserialize`.

#### Scenario: Key equality and hashing
- **WHEN** two `ResourceKey` values have identical `group`, `version`, and `kind`
- **THEN** they SHALL be equal and produce the same hash

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
The system SHALL define a `SystemMetadata` struct containing `resource_version: u64`, `created_at: DateTime<Utc>`, and `updated_at: DateTime<Utc>`. This struct SHALL derive `Debug`, `Clone`, `Serialize`, and `Deserialize` with `#[serde(rename_all = "camelCase")]`. `SystemMetadata` represents the portion of object metadata that the server controls; clients read these values but do not set them on create (they may echo `resourceVersion` on update for optimistic concurrency).

#### Scenario: SystemMetadata is part of StoredObject
- **WHEN** a `StoredObject` is constructed by the store
- **THEN** it contains a `SystemMetadata` with `resource_version`, `created_at`, and `updated_at`

#### Scenario: SystemMetadata serializes with camelCase
- **WHEN** a `SystemMetadata` is serialized
- **THEN** the JSON field names are `resourceVersion`, `createdAt`, `updatedAt`

### Requirement: UserData wraps raw JSON for extensibility
The system SHALL define a `UserData` named struct containing a single `value: serde_json::Value` field.

#### Scenario: Handler receives user JSON
- **WHEN** a handler deserializes a request body
- **THEN** the payload SHALL be wrapped in `UserData { value: ... }` before passing to the service layer

### Requirement: Schema represented as StoredObject convention
Schemas SHALL be represented as `StoredObject` with `kind="Schema"` in group `"kapi.io"`, not as a separate `Schema` struct. The `StoredObject.data` field SHALL hold a JSON Schema value for validation.

#### Scenario: Schema struct removed
- **WHEN** the codebase is compiled
- **THEN** `src/schema/types.rs` does not exist
- **AND** no `Schema` struct is defined outside of `StoredObject`

#### Scenario: Schema registration stores the raw JSON Schema
- **WHEN** a schema is registered
- **THEN** a `StoredObject` with `kind="Schema"` stores the raw JSON Schema value in its `data` field for later validation

### Requirement: Schema module scope
`src/schema/mod.rs` SHALL only declare `pub mod meta_schema`.

#### Scenario: Schema module contains only meta_schema
- **WHEN** the schema module is compiled
- **THEN** it contains only `meta_schema.rs`
- **AND** `schema/types.rs`, `schema/service.rs`, `schema/handler.rs` do not exist

### Requirement: ValidationError carries structured validation failures
The system SHALL define a `ValidationError` struct with `path: String` and `message: String`, located in `src/object/types.rs`.

#### Scenario: Mapping jsonschema output
- **WHEN** the `jsonschema` crate reports validation failures
- **THEN** each failure SHALL be mapped to `ValidationError { path, message }`

#### Scenario: ValidationError accessible from object module
- **WHEN** `error.rs` imports `ValidationError`
- **THEN** it imports from `crate::object::types::ValidationError`

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
All public types defined for core types SHALL derive `Debug` and `Clone`. Types that cross API boundaries SHALL additionally derive `Serialize` and `Deserialize`. `ObjectMeta` and `SystemMetadata` SHALL each derive `Debug`, `Clone`, `Serialize`, and `Deserialize` with `#[serde(rename_all = "camelCase")]`.

#### Scenario: Serialization roundtrip
- **WHEN** a `StoredObject` is serialized to JSON and back
- **THEN** the resulting value SHALL equal the original
