## MODIFIED Requirements

### Requirement: Core types derive standard traits
All public types defined in `kapi-core` SHALL derive `Debug` and `Clone`. Types that cross API boundaries SHALL additionally derive `Serialize` and `Deserialize`. `ObjectMeta` and `SystemMetadata` SHALL each derive `Debug`, `Clone`, `Serialize`, and `Deserialize` with `#[serde(rename_all = "camelCase")]`.

#### Scenario: Serialization roundtrip
- **WHEN** a `StoredObject` is serialized to JSON and back
- **THEN** the resulting value SHALL equal the original

**Note**: Types are now defined in `kapi-core` crate instead of `kapi-server`. The trait derivation requirements remain the same; only the location changes.

### Requirement: InvalidFieldSelector error variant
The system SHALL define an `InvalidFieldSelector(String)` variant in `AppError` that returns HTTP 400 Bad Request with the error message. The server SHALL implement `From<CoreError> for AppError` to convert `CoreError::InvalidFieldSelector(msg)` to `AppError::InvalidFieldSelector(msg)`.

#### Scenario: InvalidFieldSelector returns 400
- **WHEN** an `AppError::InvalidFieldSelector(msg)` is returned from a handler
- **THEN** the HTTP response SHALL be 400 Bad Request with the error message

#### Scenario: CoreError converts to InvalidFieldSelector
- **WHEN** a `CoreError::InvalidFieldSelector(msg)` is converted to `AppError`
- **THEN** the result SHALL be `AppError::InvalidFieldSelector(msg)`

**Note**: `FieldSelector::parse()` now returns `CoreError` instead of `AppError`. The server adapts via `From` trait.
