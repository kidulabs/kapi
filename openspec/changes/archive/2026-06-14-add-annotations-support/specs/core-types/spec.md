## MODIFIED Requirements

### Requirement: ObjectMeta groups user-controlled metadata fields
`ObjectMeta` SHALL contain a `name` field of type `String`, a `labels` field of type `HashMap<String, String>`, and an `annotations` field of type `HashMap<String, String>`. All fields SHALL use `camelCase` serialization via `#[serde(rename_all = "camelCase")]`. The `annotations` field SHALL use `#[serde(default)]` to default to an empty map when absent.

#### Scenario: ObjectMeta serialization with labels and annotations
- **WHEN** an `ObjectMeta` with `name: "my-widget"`, `labels: {"app": "nginx"}`, and `annotations: {"description": "my widget"}` is serialized
- **THEN** the JSON output SHALL be `{"name": "my-widget", "labels": {"app": "nginx"}, "annotations": {"description": "my widget"}}`

#### Scenario: ObjectMeta serialization without labels or annotations
- **WHEN** an `ObjectMeta` with `name: "my-widget"` and empty labels and annotations is serialized
- **THEN** the JSON output SHALL be `{"name": "my-widget", "labels": {}, "annotations": {}}`

#### Scenario: ObjectMeta deserialization with labels and annotations
- **WHEN** JSON `{"name": "my-widget", "labels": {"env": "prod"}, "annotations": {"owner": "team"}}` is deserialized into `ObjectMeta`
- **THEN** the resulting struct SHALL have `name = "my-widget"`, `labels = {"env": "prod"}`, and `annotations = {"owner": "team"}`

#### Scenario: ObjectMeta deserialization without labels or annotations fields
- **WHEN** JSON `{"name": "my-widget"}` is deserialized into `ObjectMeta`
- **THEN** the resulting struct SHALL have `name = "my-widget"`, `labels` as an empty `HashMap`, and `annotations` as an empty `HashMap`

#### Scenario: ObjectMeta deserialization with only labels
- **WHEN** JSON `{"name": "my-widget", "labels": {"app": "nginx"}}` is deserialized into `ObjectMeta`
- **THEN** the resulting struct SHALL have `name = "my-widget"`, `labels = {"app": "nginx"}`, and `annotations` as an empty `HashMap`

#### Scenario: ObjectMeta deserialization with only annotations
- **WHEN** JSON `{"name": "my-widget", "annotations": {"description": "test"}}` is deserialized into `ObjectMeta`
- **THEN** the resulting struct SHALL have `name = "my-widget"`, `labels` as an empty `HashMap`, and `annotations = {"description": "test"}`
