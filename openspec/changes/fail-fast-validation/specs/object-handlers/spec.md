## ADDED Requirements

### Requirement: Create handler validates label format eagerly
The create handler SHALL call `validate_labels(&labels)` immediately after extracting labels from the request body, before invoking `ObjectService::create`. If validation fails, the handler SHALL return the error without calling the service.

#### Scenario: Create with invalid label key rejected at handler
- **WHEN** a POST request is received with `metadata.labels: {"invalid key!": "value"}`
- **THEN** the handler SHALL return `AppError::InvalidLabel` without invoking the service

#### Scenario: Create with label value exceeding length rejected at handler
- **WHEN** a POST request is received with a label value exceeding 256 characters
- **THEN** the handler SHALL return `AppError::InvalidLabel` without invoking the service

### Requirement: Create handler validates annotation format eagerly
The create handler SHALL call `validate_annotations(&annotations)` immediately after extracting annotations from the request body, before invoking `ObjectService::create`. If validation fails, the handler SHALL return the error without calling the service.

#### Scenario: Create with annotation key exceeding length rejected at handler
- **WHEN** a POST request is received with an annotation key exceeding 256 characters
- **THEN** the handler SHALL return `AppError::InvalidAnnotation` without invoking the service

#### Scenario: Create with annotations exceeding total size rejected at handler
- **WHEN** a POST request is received with annotations whose total serialized size exceeds 256KB
- **THEN** the handler SHALL return `AppError::InvalidAnnotation` without invoking the service

### Requirement: Update handler validates label format eagerly
The update handler SHALL call `validate_labels(&body.metadata.labels)` after the URL/body consistency check, before invoking `ObjectService::update`. If validation fails, the handler SHALL return the error without calling the service.

#### Scenario: Update with invalid label key rejected at handler
- **WHEN** a PUT request is received with `metadata.labels: {"invalid key!": "value"}`
- **THEN** the handler SHALL return `AppError::InvalidLabel` without invoking the service

#### Scenario: Update with valid labels passes through
- **WHEN** a PUT request is received with valid labels
- **THEN** the handler SHALL pass the labels to the service for processing

### Requirement: Update handler validates annotation format eagerly
The update handler SHALL call `validate_annotations(&body.metadata.annotations)` after the URL/body consistency check, before invoking `ObjectService::update`. If validation fails, the handler SHALL return the error without calling the service.

#### Scenario: Update with annotations exceeding total size rejected at handler
- **WHEN** a PUT request is received with annotations whose total serialized size exceeds 256KB
- **THEN** the handler SHALL return `AppError::InvalidAnnotation` without invoking the service

### Requirement: Handler principle updated
The module documentation in `src/object/handler.rs` SHALL state: "Handlers validate input format and deserialization constraints. They never access the store, event bus, or schema registry, and never contain conditional mutation logic."

#### Scenario: Handler module doc reflects new principle
- **WHEN** the handler module documentation is read
- **THEN** it SHALL describe format validation as a handler responsibility and state access as prohibited
