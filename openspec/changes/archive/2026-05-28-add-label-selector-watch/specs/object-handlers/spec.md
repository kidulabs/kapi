## MODIFIED Requirements

### Requirement: ListQuery accepts labelSelector
The `ListQuery` struct SHALL include a `label_selector: Option<String>` field bound to the `labelSelector` query parameter.

#### Scenario: Watch request with labelSelector
- **WHEN** a GET request is received with `?watch=true&labelSelector=app=nginx`
- **THEN** the handler SHALL parse `labelSelector` and create a `WatchFilter::LabelSelector`

#### Scenario: Watch request with both selectors
- **WHEN** a GET request is received with `?watch=true&fieldSelector=metadata.name=foo&labelSelector=app=nginx`
- **THEN** the handler SHALL parse both and create separate filters (combination is Phase 3)

#### Scenario: List request with labelSelector
- **WHEN** a GET request is received with `?labelSelector=app=nginx` (no watch)
- **THEN** the handler SHALL return 400 (labelSelector on list is Phase 3)

### Requirement: Parse label selector in handler
The handler SHALL implement `parse_label_selector(raw: &str) -> Result<WatchFilter, AppError>` to parse the `labelSelector` query parameter.

#### Scenario: Parse valid label selector
- **WHEN** `parse_label_selector("app=nginx,env=prod")` is called
- **THEN** it SHALL return `Ok(WatchFilter::LabelSelector(...))` with the parsed requirements

#### Scenario: Parse invalid label selector
- **WHEN** `parse_label_selector("invalid selector")` is called
- **THEN** it SHALL return `Err(AppError::InvalidLabelSelector(...))` with a descriptive message

#### Scenario: Parse empty label selector
- **WHEN** `parse_label_selector("")` is called
- **THEN** it SHALL return `Ok(WatchFilter::LabelSelector(...))` with empty requirements (matches all)
