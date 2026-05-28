## MODIFIED Requirements

### Requirement: Static component schemas cover kapi built-in types
The generated OpenAPI spec SHALL include component schemas for: `ResourceKey`, `ObjectMeta`, `SystemMetadata`, `UserData`, `StoredObject`, `ListResponse`, `WatchEvent`, `WatchEventType`, `ValidationError`, `AppError`, and `SchemaData`. The `ObjectMeta` component SHALL include a `labels` property of type `object` with `additionalProperties: { type: string }`.

#### Scenario: StoredObject component matches wire format
- **WHEN** the spec is generated
- **THEN** the `StoredObject` component has properties `key` (ref ResourceKey), `metadata` (ref ObjectMeta), `system` (ref SystemMetadata), and `data` (ref UserData)
- **AND** the `ObjectMeta` component has properties `name` (type string, required) and `labels` (type object, additionalProperties: string)
- **AND** the `SystemMetadata` component has properties `resourceVersion` (type integer, format int64, required), `createdAt` (type string, format date-time, required), and `updatedAt` (type string, format date-time, required)

#### Scenario: AppError component covers all variants
- **WHEN** the spec is generated
- **THEN** the `AppError` component represents the error response shape with `error`, `code`, and `details` fields

#### Scenario: Dynamic StoredObject references ObjectMeta and SystemMetadata
- **WHEN** the spec is generated for a registered kind
- **THEN** `{Kind}{Group}StoredObject` has a `metadata` property referencing `ObjectMeta` and a `system` property referencing `SystemMetadata`

#### Scenario: ObjectMeta schema includes labels
- **WHEN** the OpenAPI spec is generated
- **THEN** the `ObjectMeta` schema SHALL include a `labels` property of type `object` with `additionalProperties: { type: string }`

#### Scenario: Labels field in API responses
- **WHEN** any API endpoint returns an object with `ObjectMeta`
- **THEN** the response schema SHALL include the `labels` field in the metadata section

### Requirement: GET /swagger-ui/ serves HTML page (optional)
If implemented, the system SHALL provide a `GET /swagger-ui/` endpoint that returns an HTML page loading Swagger UI from a CDN and configured to fetch the spec from `/openapi`. The Swagger UI SHALL display the `labels` field in request and response schemas for all endpoints that involve objects.

#### Scenario: Swagger UI page loads
- **WHEN** `GET /swagger-ui/` is requested in a browser
- **THEN** an HTML page is returned that loads Swagger UI and points to `/openapi`

#### Scenario: Swagger UI shows labels in create request
- **WHEN** a user views the create endpoint in Swagger UI
- **THEN** the request body schema SHALL include `metadata.labels` as an optional object field

#### Scenario: Swagger UI shows labels in response
- **WHEN** a user views any endpoint response in Swagger UI
- **THEN** the response schema SHALL include `metadata.labels` in the metadata section
