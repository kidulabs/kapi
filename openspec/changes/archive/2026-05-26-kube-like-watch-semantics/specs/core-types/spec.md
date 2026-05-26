## ADDED Requirements

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