## MODIFIED Requirements

### Requirement: OpenAPI spec documents fieldSelector on list
The generated OpenAPI 3.0.3 spec SHALL include the `fieldSelector` query parameter on the list endpoint (not just watch). The parameter SHALL be typed as `string` and marked as optional.

#### Scenario: fieldSelector parameter on list endpoint
- **WHEN** the OpenAPI spec is generated
- **THEN** the list endpoint SHALL include a `fieldSelector` query parameter of type `string`

#### Scenario: fieldSelector parameter description updated
- **WHEN** the OpenAPI spec is generated
- **THEN** the `fieldSelector` parameter description SHALL indicate it is valid on both list and watch requests

### Requirement: OpenAPI spec documents labelSelector on list
The generated OpenAPI 3.0.3 spec SHALL include the `labelSelector` query parameter on the list endpoint (not just watch).

#### Scenario: labelSelector parameter on list endpoint
- **WHEN** the OpenAPI spec is generated
- **THEN** the list endpoint SHALL include a `labelSelector` query parameter of type `string`

### Requirement: Swagger UI reflects selectors on list
The Swagger UI SHALL display both `fieldSelector` and `labelSelector` query parameters on the list endpoint.

#### Scenario: Swagger UI shows fieldSelector on list
- **WHEN** a user views the list endpoint in Swagger UI
- **THEN** the parameter list SHALL include `fieldSelector` as an optional string input

#### Scenario: Swagger UI shows labelSelector on list
- **WHEN** a user views the list endpoint in Swagger UI
- **THEN** the parameter list SHALL include `labelSelector` as an optional string input
