## MODIFIED Requirements

### Requirement: Meta-schema includes optional statusSchema property
The meta-schema JSON SHALL include an optional `statusSchema` property of type `"object"`. The `unevaluatedProperties: false` constraint SHALL continue to apply, so only `targetGroup`, `targetVersion`, `targetKind`, `jsonSchema`, and `statusSchema` are allowed.

#### Scenario: Schema registration with statusSchema passes meta-schema validation
- **WHEN** a Schema registration payload includes `statusSchema` as a valid JSON Schema object
- **THEN** meta-schema validation passes

#### Scenario: Schema registration without statusSchema passes meta-schema validation
- **WHEN** a Schema registration payload does not include `statusSchema`
- **THEN** meta-schema validation passes (it is optional)

#### Scenario: Schema registration with invalid statusSchema type fails
- **WHEN** a Schema registration payload includes `statusSchema` as a non-object type (e.g., string)
- **THEN** meta-schema validation fails