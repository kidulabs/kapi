## MODIFIED Requirements

### Requirement: ObjectMeta structure
`ObjectMeta` SHALL contain a `name` field of type `String` and a `labels` field of type `HashMap<String, String>`. Both fields SHALL use `camelCase` serialization via `#[serde(rename_all = "camelCase")]`.

#### Scenario: ObjectMeta serialization with labels
- **WHEN** an `ObjectMeta` with `name: "my-widget"` and `labels: {"app": "nginx"}` is serialized
- **THEN** the JSON output SHALL be `{"name": "my-widget", "labels": {"app": "nginx"}}`

#### Scenario: ObjectMeta serialization without labels
- **WHEN** an `ObjectMeta` with `name: "my-widget"` and empty labels is serialized
- **THEN** the JSON output SHALL be `{"name": "my-widget", "labels": {}}`

#### Scenario: ObjectMeta deserialization with labels
- **WHEN** JSON `{"name": "my-widget", "labels": {"env": "prod"}}` is deserialized into `ObjectMeta`
- **THEN** the resulting struct SHALL have `name = "my-widget"` and `labels = {"env": "prod"}`

#### Scenario: ObjectMeta deserialization without labels field
- **WHEN** JSON `{"name": "my-widget"}` is deserialized into `ObjectMeta`
- **THEN** the resulting struct SHALL have `name = "my-widget"` and `labels` as an empty `HashMap`
