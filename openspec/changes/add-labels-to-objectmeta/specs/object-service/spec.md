## MODIFIED Requirements

### Requirement: Label validation on create
`ObjectService::create()` SHALL validate labels before persistence. Validation SHALL check key format, value format, and length limits according to Kubernetes label semantics. Validation SHALL occur after schema validation of the data payload but before store persistence.

#### Scenario: Create with valid labels
- **WHEN** `create()` is called with valid labels
- **THEN** validation SHALL pass and the object SHALL be persisted with labels

#### Scenario: Create with invalid label key
- **WHEN** `create()` is called with a label key that violates format rules
- **THEN** an `AppError::InvalidLabel` error SHALL be returned with a descriptive message

#### Scenario: Create with invalid label value
- **WHEN** `create()` is called with a label value that violates format rules
- **THEN** an `AppError::InvalidLabel` error SHALL be returned with a descriptive message

#### Scenario: Create with too many labels
- **WHEN** `create()` is called with labels exceeding a reasonable count limit
- **THEN** the system SHALL accept them (no count limit enforced in this phase)

### Requirement: Label validation on update
`ObjectService::update()` SHALL validate labels on the incoming `StoredObject` before persistence, using the same rules as create.

#### Scenario: Update with valid labels
- **WHEN** `update()` is called with valid labels
- **THEN** validation SHALL pass and the object SHALL be persisted with updated labels

#### Scenario: Update with invalid labels
- **WHEN** `update()` is called with invalid labels
- **THEN** an `AppError::InvalidLabel` error SHALL be returned and no persistence SHALL occur

### Requirement: Label validation function
A `validate_labels()` function SHALL validate a `HashMap<String, String>` against Kubernetes label semantics. It SHALL return `Result<(), AppError>` with descriptive error messages identifying the offending key or value.

#### Scenario: Validate empty labels map
- **WHEN** `validate_labels()` is called with an empty `HashMap`
- **THEN** validation SHALL pass

#### Scenario: Validate key with prefix
- **WHEN** `validate_labels()` is called with key `app.kubernetes.io/name`
- **THEN** validation SHALL check prefix format (DNS subdomain, max 253 chars) and name format (max 256 chars, valid characters)

#### Scenario: Validate empty value
- **WHEN** `validate_labels()` is called with a label whose value is an empty string
- **THEN** validation SHALL pass (empty values are allowed)
