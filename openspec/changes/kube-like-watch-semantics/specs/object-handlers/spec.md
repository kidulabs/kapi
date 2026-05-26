## MODIFIED Requirements

### Requirement: List handler supports both list and watch modes
The list handler SHALL check for `?watch=true` query parameter. If present, it SHALL parse the `fieldSelector` query parameter into a `WatchFilter`, subscribe to the event bus with the filter, and return an SSE stream. If `fieldSelector` is not present, `WatchFilter::All` SHALL be used. If `fieldSelector` is present on a non-watch request, the handler SHALL return 400 Bad Request with `InvalidFieldSelector` error.

#### Scenario: List returns JSON
- **WHEN** GET `/apis/example.io/v1/Widget` without `?watch=true`
- **THEN** the response is 200 OK with `ListResponse` as JSON

#### Scenario: Watch returns SSE stream
- **WHEN** GET `/apis/example.io/v1/Widget?watch=true`
- **THEN** the response is an SSE stream of `WatchEvent` objects

#### Scenario: Watch with fieldSelector filters by name
- **WHEN** GET `/apis/example.io/v1/Widget?watch=true&fieldSelector=metadata.name=my-widget`
- **THEN** the SSE stream only delivers events for objects with `metadata.name == "my-widget"`

#### Scenario: Watch without fieldSelector returns all events
- **WHEN** GET `/apis/example.io/v1/Widget?watch=true`
- **THEN** the SSE stream delivers all events for the Widget kind

#### Scenario: fieldSelector on non-watch request returns 400
- **WHEN** GET `/apis/example.io/v1/Widget?fieldSelector=metadata.name=my-widget` (without `?watch=true`)
- **THEN** the response is 400 Bad Request with `InvalidFieldSelector` error

#### Scenario: Invalid fieldSelector returns 400
- **WHEN** GET `/apis/example.io/v1/Widget?watch=true&fieldSelector=metadata.namespace=default`
- **THEN** the response is 400 Bad Request with `InvalidFieldSelector` error indicating the field is not supported

#### Scenario: Malformed fieldSelector returns 400
- **WHEN** GET `/apis/example.io/v1/Widget?watch=true&fieldSelector=invalid-format`
- **THEN** the response is 400 Bad Request with `InvalidFieldSelector` error indicating the format is invalid

#### Scenario: Watch events have correct SSE format
- **WHEN** an object is created while a watch is active
- **THEN** the SSE stream receives an event with `event: message` and the `WatchEvent` JSON as data

### Requirement: ListQuery includes field_selector
The `ListQuery` struct in `src/object/handler.rs` SHALL include a `field_selector: Option<String>` field with `#[serde(rename = "fieldSelector")]`.

#### Scenario: fieldSelector query parameter deserialized
- **WHEN** a request includes `?fieldSelector=metadata.name=my-widget`
- **THEN** `ListQuery.field_selector` SHALL be `Some("metadata.name=my-widget".to_string())`

#### Scenario: No fieldSelector in request
- **WHEN** a request does not include `fieldSelector`
- **THEN** `ListQuery.field_selector` SHALL be `None`