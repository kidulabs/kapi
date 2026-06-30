## MODIFIED Requirements

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
