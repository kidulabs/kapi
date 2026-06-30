## Purpose

Define the core domain types that represent resources, objects, schemas, pagination, and watch events. These types serve as the foundation for all storage, service, and handler layers.
## Requirements
### Requirement: ResourceKey uniquely identifies a resource kind
The system SHALL define a `ResourceKey` struct with `group`, `version`, and `kind` fields that implements `Hash`, `Eq`, `Clone`, `Serialize`, and `Deserialize`.

#### Scenario: Key equality and hashing
- **WHEN** two `ResourceKey` values have identical `group`, `version`, and `kind`
- **THEN** they SHALL be equal and produce the same hash

### Requirement: StoredObject represents a persisted custom object
The system SHALL define a `StoredObject` struct containing `key: ResourceKey`, `metadata: ObjectMeta`, `system: SystemMetadata`, `spec: serde_json::Value`, and `status: Option<serde_json::Value>`. The `status` field SHALL be `None` for kinds without a `statusSchema` and `Some(Value)` for kinds with one. The `ObjectMeta` struct SHALL contain `name: String`, `namespace: Option<String>`, `labels: HashMap<String, String>`, `annotations: HashMap<String, String>`, and `finalizers: Vec<String>`. The `namespace` field SHALL use `#[serde(skip_serializing_if = "Option::is_none")]`. The `SystemMetadata` struct SHALL contain `resource_version: u64`, `generation: u64`, `created_at: DateTime<Utc>`, `updated_at: DateTime<Utc>`, and `deletion_timestamp: Option<DateTime<Utc>>`. All structs SHALL derive `Debug`, `Clone`, `Serialize`, and `Deserialize` with `#[serde(rename_all = "camelCase")]`.

#### Scenario: StoredObject serializes with namespace
- **WHEN** a `StoredObject` with `metadata.namespace = Some("production")` is serialized
- **THEN** the JSON contains `"namespace": "production"` in the metadata

#### Scenario: StoredObject serializes without namespace (cluster-scoped)
- **WHEN** a `StoredObject` with `metadata.namespace = None` is serialized
- **THEN** the JSON does NOT contain a `namespace` key in the metadata

#### Scenario: StoredObject deserializes with namespace
- **WHEN** JSON with `"namespace": "production"` in metadata is deserialized
- **THEN** the resulting `StoredObject` has `metadata.namespace = Some("production")`

#### Scenario: StoredObject deserializes without namespace
- **WHEN** JSON without `namespace` in metadata is deserialized
- **THEN** the resulting `StoredObject` has `metadata.namespace = None`

### Requirement: ObjectMeta groups user-controlled metadata fields
`ObjectMeta` SHALL contain a `name` field of type `String`, a `namespace` field of type `Option<String>`, a `labels` field of type `HashMap<String, String>`, an `annotations` field of type `HashMap<String, String>`, and a `finalizers` field of type `Vec<String>`. All fields SHALL use `camelCase` serialization. The `namespace` field SHALL use `#[serde(skip_serializing_if = "Option::is_none")]`. The `annotations` and `finalizers` fields SHALL use `#[serde(default)]`.

#### Scenario: ObjectMeta serialization with namespace
- **WHEN** an `ObjectMeta` with `name: "my-widget"`, `namespace: Some("production")`, `labels: {"app": "nginx"}` is serialized
- **THEN** the JSON output SHALL include `"namespace": "production"`

#### Scenario: ObjectMeta serialization without namespace
- **WHEN** an `ObjectMeta` with `name: "my-widget"`, `namespace: None` is serialized
- **THEN** the JSON output SHALL NOT include a `namespace` key

#### Scenario: ObjectMeta deserialization with namespace
- **WHEN** JSON `{"name": "my-widget", "namespace": "production"}` is deserialized
- **THEN** the resulting struct SHALL have `namespace = Some("production")`

#### Scenario: ObjectMeta deserialization without namespace
- **WHEN** JSON `{"name": "my-widget"}` is deserialized
- **THEN** the resulting struct SHALL have `namespace = None`

### Requirement: SystemMetadata groups server-managed lifecycle fields
The system SHALL define a `SystemMetadata` struct containing `resource_version: u64`, `created_at: DateTime<Utc>`, and `updated_at: DateTime<Utc>`. This struct SHALL derive `Debug`, `Clone`, `Serialize`, and `Deserialize` with `#[serde(rename_all = "camelCase")]`. `SystemMetadata` represents the portion of object metadata that the server controls; clients read these values but do not set them on create (they may echo `resourceVersion` on update for optimistic concurrency).

#### Scenario: SystemMetadata is part of StoredObject
- **WHEN** a `StoredObject` is constructed by the store
- **THEN** it contains a `SystemMetadata` with `resource_version`, `created_at`, and `updated_at`

#### Scenario: SystemMetadata serializes with camelCase
- **WHEN** a `SystemMetadata` is serialized
- **THEN** the JSON field names are `resourceVersion`, `createdAt`, `updatedAt`

### Requirement: SchemaData includes optional statusSchema
The system SHALL define `SchemaData` with fields `target_group: String`, `target_version: String`, `target_kind: String`, `json_schema: serde_json::Value`, and `status_schema: Option<serde_json::Value>`. The `status_schema` field SHALL use `#[serde(rename_all = "camelCase")]` serialization, producing `statusSchema` in JSON.

#### Scenario: SchemaData with statusSchema serializes correctly
- **WHEN** a `SchemaData` with `status_schema: Some({...})` is serialized
- **THEN** the JSON contains `"statusSchema": {...}` alongside `targetGroup`, `targetVersion`, `targetKind`, and `jsonSchema`

#### Scenario: SchemaData without statusSchema serializes correctly
- **WHEN** a `SchemaData` with `status_schema: None` is serialized
- **THEN** the JSON does not contain a `statusSchema` key (or it is `null`)

### Requirement: Schema represented as StoredObject convention
Schemas SHALL be represented as `StoredObject` with `kind="Schema"` in group `"kapi.io"`, not as a separate `Schema` struct. The `StoredObject.spec` field SHALL hold a JSON Schema value for validation.

#### Scenario: Schema struct removed
- **WHEN** the codebase is compiled
- **THEN** `src/schema/types.rs` does not exist
- **AND** no `Schema` struct is defined outside of `StoredObject`

#### Scenario: Schema registration stores the raw JSON Schema
- **WHEN** a schema is registered
- **THEN** a `StoredObject` with `kind="Schema"` stores the raw JSON Schema value in its `spec` field for later validation

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

### Requirement: ContinueToken encodes namespace and name
The `ContinueToken` SHALL encode both `namespace: Option<String>` and `name: String` to support cross-namespace pagination. The token format SHALL be a base64-encoded JSON object `{"namespace": "...", "name": "..."}` where namespace is `null` for cluster-scoped objects.

#### Scenario: ContinueToken with namespace
- **WHEN** a continue token is created for namespace "production" and name "foo"
- **THEN** the encoded token SHALL contain `{"namespace": "production", "name": "foo"}`

#### Scenario: ContinueToken without namespace (cluster-scoped)
- **WHEN** a continue token is created for namespace `None` and name "foo"
- **THEN** the encoded token SHALL contain `{"namespace": null, "name": "foo"}`

#### Scenario: ContinueToken roundtrip
- **WHEN** a ContinueToken is encoded and then decoded
- **THEN** the namespace and name SHALL be preserved

### Requirement: WatchEvent supports real-time change notifications
The system SHALL define `WatchEventType` as an enum with `Added`, `Modified`, `Deleted`, and `StatusModified` variants, and `WatchEvent` as a struct with `event_type: WatchEventType` and `object: StoredObject`.

#### Scenario: Watch stream receives events
- **WHEN** an object is created, updated, deleted, or has its status updated
- **THEN** watchers SHALL receive a `WatchEvent` with the corresponding `WatchEventType` and the affected `StoredObject`

#### Scenario: StatusModified variant exists
- **WHEN** the `WatchEventType` enum is compiled
- **THEN** it SHALL have variants `Added`, `Modified`, `Deleted`, and `StatusModified`

### Requirement: WatchFilter and FieldSelector types for watch event filtering
The system SHALL define `WatchFilter` and `FieldSelector` enums in `src/object/types.rs`. `WatchFilter` SHALL have variants `All` and `FieldSelector(FieldSelector)`. `FieldSelector` SHALL have variant `NameEquals(String)`. Both SHALL derive `Debug` and `Clone`. `WatchFilter` SHALL implement a `matches(&self, event: &WatchEvent) -> bool` method.

#### Scenario: WatchFilter::All matches any event
- **WHEN** `WatchFilter::All.matches(&event)` is called for any `WatchEvent`
- **THEN** the result SHALL be `true`

#### Scenario: WatchFilter::FieldSelector with NameEquals matches by name
- **WHEN** `WatchFilter::FieldSelector(FieldSelector::NameEquals("test".into())).matches(&event)` is called
- **AND** `event.object.metadata.name == "test"`
- **THEN** the result SHALL be `true`

#### Scenario: WatchFilter::FieldSelector with NameEquals rejects non-matching name
- **WHEN** `WatchFilter::FieldSelector(FieldSelector::NameEquals("test".into())).matches(&event)` is called
- **AND** `event.object.metadata.name != "test"`
- **THEN** the result SHALL be `false`

### Requirement: InvalidFieldSelector error variant
The system SHALL define an `InvalidFieldSelector(String)` variant in `AppError` that returns HTTP 400 Bad Request with the error message.

#### Scenario: InvalidFieldSelector returns 400
- **WHEN** an `AppError::InvalidFieldSelector(msg)` is returned from a handler
- **THEN** the HTTP response SHALL be 400 Bad Request with the error message

### Requirement: Core types derive standard traits
All public types defined for core types SHALL derive `Debug` and `Clone`. Types that cross API boundaries SHALL additionally derive `Serialize` and `Deserialize`. `ObjectMeta` and `SystemMetadata` SHALL each derive `Debug`, `Clone`, `Serialize`, and `Deserialize` with `#[serde(rename_all = "camelCase")]`.

#### Scenario: Serialization roundtrip
- **WHEN** a `StoredObject` is serialized to JSON and back
- **THEN** the resulting value SHALL equal the original

