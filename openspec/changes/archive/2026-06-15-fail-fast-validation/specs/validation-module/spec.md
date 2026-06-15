## ADDED Requirements

### Requirement: Validation module provides stateless format validation
The system SHALL provide a `src/validation/` module containing pure functions for validating label and annotation format. These functions SHALL have no dependencies on the store, event bus, schema registry, or any I/O. They SHALL be callable from any layer (handler, service, tests).

#### Scenario: Validation module is importable
- **WHEN** a module imports `crate::validation::validate_labels`
- **THEN** the function SHALL be available without constructing an `ObjectService` or accessing any stateful dependency

### Requirement: Label key validation uses precompiled regex
The `validate_label_key` function SHALL use `std::sync::LazyLock<Regex>` statics for the prefix DNS subdomain pattern and the label name pattern. Regex compilation SHALL occur exactly once per process lifetime, not per function call.

#### Scenario: Regex compiled once
- **WHEN** `validate_label_key` is called multiple times across multiple requests
- **THEN** the regex patterns SHALL be compiled only on the first call and reused for all subsequent calls

### Requirement: Label value validation uses precompiled regex
The `validate_label_value` function SHALL use a `std::sync::LazyLock<Regex>` static for the value pattern. Regex compilation SHALL occur exactly once per process lifetime.

#### Scenario: Value regex compiled once
- **WHEN** `validate_label_value` is called multiple times
- **THEN** the regex pattern SHALL be compiled only on the first call

### Requirement: Validation functions preserve existing error behavior
The validation functions in the `validation/` module SHALL return the same `AppError` variants (`InvalidLabel`, `InvalidAnnotation`) with the same error messages as the current implementations in `object/service.rs`. No error behavior SHALL change.

#### Scenario: Invalid label key returns same error
- **WHEN** `validate_label_key` is called with an empty string
- **THEN** the error SHALL be `AppError::InvalidLabel("label key must not be empty")`

#### Scenario: Invalid annotation key returns same error
- **WHEN** `validate_annotation_key` is called with a key exceeding 256 characters
- **THEN** the error SHALL be `AppError::InvalidAnnotation` with a message indicating the key exceeds the maximum length

### Requirement: Validation module location
All format validation functions for labels and annotations SHALL be defined in `src/validation/mod.rs`. The module SHALL be declared in `src/lib.rs`.

#### Scenario: Module is accessible
- **WHEN** the project is built
- **THEN** `src/validation/mod.rs` SHALL exist and contain `validate_labels`, `validate_annotations`, `validate_label_key`, `validate_label_value`, and `validate_annotation_key`
