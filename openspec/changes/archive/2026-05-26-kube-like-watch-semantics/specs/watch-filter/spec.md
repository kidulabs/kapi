## ADDED Requirements

### Requirement: WatchFilter enum defines watch event filtering
The system SHALL define a `WatchFilter` enum in `src/object/types.rs` with variants `All` and `FieldSelector(FieldSelector)`. `WatchFilter` SHALL derive `Debug` and `Clone`.

#### Scenario: WatchFilter::All matches all events
- **WHEN** `WatchFilter::All.matches(&event)` is called for any `WatchEvent`
- **THEN** the result SHALL be `true`

#### Scenario: WatchFilter::FieldSelector with NameEquals matches by name
- **WHEN** `WatchFilter::FieldSelector(FieldSelector::NameEquals("my-widget".into())).matches(&event)` is called
- **AND** `event.object.metadata.name == "my-widget"`
- **THEN** the result SHALL be `true`

#### Scenario: WatchFilter::FieldSelector with NameEquals rejects non-matching name
- **WHEN** `WatchFilter::FieldSelector(FieldSelector::NameEquals("my-widget".into())).matches(&event)` is called
- **AND** `event.object.metadata.name != "my-widget"`
- **THEN** the result SHALL be `false`

### Requirement: FieldSelector enum defines field-based filtering
The system SHALL define a `FieldSelector` enum in `src/object/types.rs` with variant `NameEquals(String)`. `FieldSelector` SHALL derive `Debug` and `Clone`.

#### Scenario: FieldSelector::NameEquals matches exact name
- **WHEN** `FieldSelector::NameEquals("test".into())` is used to match a `WatchEvent` with `object.metadata.name == "test"`
- **THEN** the match SHALL return `true`

#### Scenario: FieldSelector::NameEquals rejects different name
- **WHEN** `FieldSelector::NameEquals("test".into())` is used to match a `WatchEvent` with `object.metadata.name == "other"`
- **THEN** the match SHALL return `false`

### Requirement: parse_field_selector converts query string to WatchFilter
The system SHALL provide a `parse_field_selector(raw: &str) -> Result<WatchFilter, AppError>` function in `src/object/handler.rs` that parses the `fieldSelector` query parameter value.

#### Scenario: Valid metadata.name field selector
- **WHEN** `parse_field_selector("metadata.name=my-widget")` is called
- **THEN** the result SHALL be `Ok(WatchFilter::FieldSelector(FieldSelector::NameEquals("my-widget".into())))`

#### Scenario: Unsupported field returns InvalidFieldSelector error
- **WHEN** `parse_field_selector("metadata.namespace=default")` is called
- **THEN** the result SHALL be `Err(AppError::InvalidFieldSelector(_))` with a message indicating the field is not supported

#### Scenario: Malformed field selector returns InvalidFieldSelector error
- **WHEN** `parse_field_selector("invalid-format")` is called
- **THEN** the result SHALL be `Err(AppError::InvalidFieldSelector(_))` with a message indicating the format is invalid

### Requirement: InvalidFieldSelector error variant
The system SHALL define an `InvalidFieldSelector(String)` variant in `AppError` that returns HTTP 400 Bad Request.

#### Scenario: InvalidFieldSelector returns 400
- **WHEN** an `AppError::InvalidFieldSelector(msg)` is returned from a handler
- **THEN** the HTTP response SHALL be 400 Bad Request with the error message