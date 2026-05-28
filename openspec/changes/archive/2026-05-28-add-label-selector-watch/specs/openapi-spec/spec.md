## MODIFIED Requirements

### Requirement: OpenAPI spec documents labelSelector parameter
The generated OpenAPI 3.0.3 spec SHALL include the `labelSelector` query parameter on the list/watch endpoint. The parameter SHALL be typed as `string` and marked as optional.

#### Scenario: labelSelector parameter in OpenAPI spec
- **WHEN** the OpenAPI spec is generated
- **THEN** the list/watch endpoint SHALL include a `labelSelector` query parameter of type `string`

#### Scenario: labelSelector parameter description
- **WHEN** the OpenAPI spec is generated
- **THEN** the `labelSelector` parameter SHALL have a description explaining the supported syntax (equality, inequality, existence, non-existence, AND)

### Requirement: Swagger UI reflects labelSelector parameter
The Swagger UI SHALL display the `labelSelector` query parameter on the list/watch endpoint.

#### Scenario: Swagger UI shows labelSelector input
- **WHEN** a user views the list/watch endpoint in Swagger UI
- **THEN** the parameter list SHALL include `labelSelector` as an optional string input
