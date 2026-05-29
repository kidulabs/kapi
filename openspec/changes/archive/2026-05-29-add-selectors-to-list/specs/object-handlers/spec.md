## MODIFIED Requirements

### Requirement: List handler accepts fieldSelector
The list handler SHALL accept `fieldSelector` on non-watch requests and pass it to `ListOptions`. The previous 400 error SHALL be removed.

#### Scenario: List with fieldSelector
- **WHEN** a GET request is received with `?fieldSelector=metadata.name=foo` (no watch)
- **THEN** the handler SHALL parse the selector and include it in `ListOptions`

#### Scenario: List with invalid fieldSelector
- **WHEN** a GET request is received with `?fieldSelector=invalid` (malformed)
- **THEN** the handler SHALL return `AppError::InvalidFieldSelector` with HTTP 400

### Requirement: List handler accepts labelSelector
The list handler SHALL accept `labelSelector` on non-watch requests and pass it to `ListOptions`.

#### Scenario: List with labelSelector
- **WHEN** a GET request is received with `?labelSelector=app=nginx` (no watch)
- **THEN** the handler SHALL parse the selector and include it in `ListOptions`

#### Scenario: List with invalid labelSelector
- **WHEN** a GET request is received with `?labelSelector=invalid` (malformed)
- **THEN** the handler SHALL return `AppError::InvalidLabelSelector` with HTTP 400

### Requirement: List handler accepts both selectors
The list handler SHALL accept both `fieldSelector` and `labelSelector` on the same request.

#### Scenario: List with both selectors
- **WHEN** a GET request is received with `?fieldSelector=metadata.name=foo&labelSelector=app=nginx` (no watch)
- **THEN** the handler SHALL parse both selectors and include them in `ListOptions`

### Requirement: Watch handler combines selectors with And
When both `fieldSelector` and `labelSelector` are present on a watch request, the handler SHALL combine them with `WatchFilter::And`.

#### Scenario: Watch with both selectors
- **WHEN** a GET request is received with `?watch=true&fieldSelector=metadata.name=foo&labelSelector=app=nginx`
- **THEN** the handler SHALL create `WatchFilter::And(Box::new(FieldSelector(...)), Box::new(LabelSelector(...)))`

#### Scenario: Watch with only field selector
- **WHEN** a GET request is received with `?watch=true&fieldSelector=metadata.name=foo`
- **THEN** the handler SHALL create `WatchFilter::FieldSelector(...)` (no And wrapper)

#### Scenario: Watch with only label selector
- **WHEN** a GET request is received with `?watch=true&labelSelector=app=nginx`
- **THEN** the handler SHALL create `WatchFilter::LabelSelector(...)` (no And wrapper)

#### Scenario: Watch with no selectors
- **WHEN** a GET request is received with `?watch=true` (no selectors)
- **THEN** the handler SHALL create `WatchFilter::All`
