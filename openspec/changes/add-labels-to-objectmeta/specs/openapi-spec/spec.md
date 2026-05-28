## MODIFIED Requirements

### Requirement: OpenAPI spec documents labels field
The generated OpenAPI 3.0.3 spec SHALL include the `labels` field in the `ObjectMeta` schema definition. The field SHALL be typed as an object with `additionalProperties` of type `string`.

#### Scenario: ObjectMeta schema in OpenAPI spec
- **WHEN** the OpenAPI spec is generated
- **THEN** the `ObjectMeta` schema SHALL include a `labels` property of type `object` with `additionalProperties: { type: string }`

#### Scenario: Labels field in API responses
- **WHEN** any API endpoint returns an object with `ObjectMeta`
- **THEN** the response schema SHALL include the `labels` field in the metadata section

### Requirement: Swagger UI reflects labels field
The Swagger UI SHALL display the `labels` field in request and response schemas for all endpoints that involve objects.

#### Scenario: Swagger UI shows labels in create request
- **WHEN** a user views the create endpoint in Swagger UI
- **THEN** the request body schema SHALL include `metadata.labels` as an optional object field

#### Scenario: Swagger UI shows labels in response
- **WHEN** a user views any endpoint response in Swagger UI
- **THEN** the response schema SHALL include `metadata.labels` in the metadata section
