## Purpose

Dynamic OpenAPI 3.0.3 specification generation at request time, including static components/paths for Schema CRUD and dynamic per-kind paths/schemas from registered schemas. Module restructured from single file to directory (`src/openapi/`).
## Requirements
### Requirement: GET /openapi returns dynamically generated OpenAPI 3.0.3 JSON
The system SHALL provide a `GET /openapi` endpoint that returns an OpenAPI 3.0.3 specification as `application/json`. The spec SHALL be generated on every request by listing all Schema objects from the store and building the document from scratch.

#### Scenario: Empty registry returns spec with static components only
- **WHEN** `GET /openapi` is called with no registered schemas
- **THEN** the response is HTTP 200 with valid OpenAPI 3.0.3 JSON
- **AND** the spec contains static components (StoredObject, ObjectMetadata, ResourceKey, AppError, etc.)
- **AND** the spec contains static paths for Schema CRUD
- **AND** the spec contains no dynamic per-kind paths

#### Scenario: Registered schemas produce dynamic paths and components
- **WHEN** a Schema is registered for `example.io/v1/Widget`
- **AND** `GET /openapi` is called
- **THEN** the spec contains paths for `/apis/example.io/v1/Widget` and `/apis/example.io/v1/Widget/{name}`
- **AND** the spec contains component schemas `WidgetExampleIoV1`, `WidgetExampleIoV1StoredObject`, and `WidgetExampleIoV1ListResponse`

#### Scenario: Two versions of the same kind produce distinct component sets
- **WHEN** Schemas are registered for `example.io/v1/Widget` and `example.io/v2/Widget`
- **AND** `GET /openapi` is called
- **THEN** the spec contains component schemas `WidgetExampleIoV1`, `WidgetExampleIoV1StoredObject`, `WidgetExampleIoV1ListResponse` AND `WidgetExampleIoV2`, `WidgetExampleIoV2StoredObject`, `WidgetExampleIoV2ListResponse`
- **AND** both URL paths are documented (`/apis/example.io/v1/Widget` and `/apis/example.io/v2/Widget`)
- **AND** no component name collides between the two versions

#### Scenario: Spec reflects current state at request time
- **WHEN** a Schema is registered after a previous `/openapi` call
- **AND** `GET /openapi` is called again
- **THEN** the new response includes the newly registered kind's paths and components

### Requirement: Static component schemas cover kapi built-in types
The generated OpenAPI spec SHALL include component schemas for: `ResourceKey`, `ObjectMeta`, `SystemMetadata`, `StoredObject`, `ListResponse`, `WatchEvent`, `WatchEventType`, `ValidationError`, `AppError`, and `SchemaData`. The `ObjectMeta` component SHALL include a `labels` property of type `object` with `additionalProperties: { type: string }`. The `StoredObject` component SHALL declare `spec` and `status` as unconstrained JSON values (no `SpecData` component, no `value` wrapper).

#### Scenario: StoredObject component matches wire format
- **WHEN** the spec is generated
- **THEN** the `StoredObject` component has properties `key` (ref ResourceKey), `metadata` (ref ObjectMeta), `system` (ref SystemMetadata), `spec` (unconstrained JSON), and `status` (unconstrained JSON, nullable)
- **AND** the `ObjectMeta` component has properties `name` (type string, required) and `labels` (type object, additionalProperties: string)
- **AND** the `SystemMetadata` component has properties `resourceVersion` (type integer, format int64, required), `createdAt` (type string, format date-time, required), and `updatedAt` (type string, format date-time, required)
- **AND** there is no `SpecData` component in the spec

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

### Requirement: Static paths cover Schema CRUD operations
The generated OpenAPI spec SHALL include paths for:
- `GET /apis/kapi.io/v1/Schema` — list all schemas
- `POST /apis/kapi.io/v1/Schema` — register a new schema
- `GET /apis/kapi.io/v1/Schema/{name}` — get a specific schema
- `DELETE /apis/kapi.io/v1/Schema/{name}` — delete a schema

#### Scenario: Schema POST path has correct request body
- **WHEN** the spec is generated
- **THEN** `POST /apis/kapi.io/v1/Schema` has a request body schema referencing `SchemaData` with `metadata.name`

#### Scenario: Schema GET item path has correct response
- **WHEN** the spec is generated
- **THEN** `GET /apis/kapi.io/v1/Schema/{name}` has a 200 response referencing `StoredObject`

### Requirement: Dynamic per-kind paths are generated from registered schemas
For each registered Schema object, the system SHALL generate paths under `/apis/{group}/{version}/{kind}` and `/apis/{group}/{version}/{kind}/{name}` matching the Schema's `targetGroup`, `targetVersion`, and `targetKind`.

#### Scenario: Widget kind generates full CRUD paths
- **WHEN** a Schema is registered with `targetGroup: "example.io"`, `targetVersion: "v1"`, `targetKind: "Widget"`
- **THEN** the spec contains:
  - `GET /apis/example.io/v1/Widget` (list, with `?watch=true` query parameter)
  - `POST /apis/example.io/v1/Widget` (create, with `metadata.name` in request body)
  - `GET /apis/example.io/v1/Widget/{name}` (get)
  - `PUT /apis/example.io/v1/Widget/{name}` (update)
  - `DELETE /apis/example.io/v1/Widget/{name}` (delete)

#### Scenario: Two versions generate distinct path sets
- **WHEN** Schemas are registered for `example.io/v1/Widget` and `example.io/v2/Widget`
- **THEN** the spec contains a full CRUD path set for each version
- **AND** the two path sets do not overlap

#### Scenario: POST path uses kind-specific spec schema
- **WHEN** the spec is generated for a registered kind
- **THEN** the POST request body references the kind's versioned spec component (`WidgetExampleIoV1`) via `allOf` with `metadata.name`

#### Scenario: GET/PUT/DELETE responses use kind-specific StoredObject schema
- **WHEN** the spec is generated for a registered kind
- **THEN** the 200 responses reference the kind's versioned StoredObject component (`WidgetExampleIoV1StoredObject`)

### Requirement: Dynamic component schemas embed user's specSchema
For each registered Schema, the system SHALL generate a component schema from the Schema's `specSchema` field. This component represents the user's data shape and is referenced by the kind's `StoredObject` component's `spec` property. The kind-specific spec component SHALL be the user's specSchema directly, with no `value` wrapper or envelope.

#### Scenario: User schema properties appear in component
- **WHEN** a Schema is registered with `specSchema: { "type": "object", "properties": { "color": { "type": "string" }, "size": { "type": "integer" } } }`
- **THEN** the generated component (`WidgetExampleIoV1`) has `type: "object"` with `properties` containing `color` and `size` as top-level properties (not nested under a `value` key)

#### Scenario: Kind-specific StoredObject references kind-specific spec
- **WHEN** the spec is generated for a registered kind
- **THEN** `{Kind}{Group}StoredObject` has a `spec` property with `$ref` pointing to `#/components/schemas/{Kind}{Group}`

#### Scenario: Swagger UI displays user fields without indirection
- **WHEN** a user views a per-kind schema (e.g. `WidgetExampleIoV1`) in Swagger UI
- **THEN** the schema SHALL expand to show the user's fields (`color`, `size`) at the top level
- **AND** the user SHALL NOT have to click through a `value` wrapper to see the fields

### Requirement: Static and dynamic schemas are coherent across create and response
The OpenAPI schemas for the create-request body, the GET-item response, the GET-status response, and the PUT-status request body SHALL all use the same shape for user data: the user's specSchema (or statusSchema) directly, with no `value` wrapper. There SHALL be no asymmetry between request shapes and response shapes.

#### Scenario: Create request and GET response have matching shapes
- **WHEN** a kind is registered with a specSchema
- **THEN** the create request schema (which uses `build_create_request_schema`) and the GET response schema (which references the kind-specific `StoredObject`) SHALL both expose the user's specSchema fields at the same level — no `value` wrapper in either

#### Scenario: PUT /status and GET /status have matching shapes
- **WHEN** a kind is registered with a statusSchema
- **THEN** the PUT /status request body and the GET /status response body SHALL both expose the status JSON directly, with no `value` wrapper

### Requirement: Component names follow PascalCase dot-split convention
Component names SHALL be derived from the schema name (format: `{Kind}.{group}.{version}`) by splitting on dots, PascalCasing each segment, and concatenating. Example: `"Widget.example.io.v1"` → `"WidgetExampleIoV1"`.

#### Scenario: Single-dot schema name with version
- **WHEN** schema name is `"Widget.example.io.v1"`
- **THEN** component name is `"WidgetExampleIoV1"`

#### Scenario: Multi-segment group name with version
- **WHEN** schema name is `"Deployment.apps.v1"`
- **THEN** component name is `"DeploymentAppsV1"`

#### Scenario: No collision between same kind different groups or versions
- **WHEN** schemas `"Widget.example.io.v1"` and `"Widget.other.io.v1"` are both registered
- **THEN** component names are `"WidgetExampleIoV1"` and `"WidgetOtherIoV1"` respectively

### Requirement: Path parameters are documented in OpenAPI

Dynamic paths SHALL document only the `name` path parameter on item paths (`/apis/{group}/{version}/{kind}/{name}`). The `group`, `version`, and `kind` are **baked into the URL path** and are NOT path parameters in the OpenAPI spec. This follows the roadmap design where GVK is resolved at route registration time, not at request time.

#### Scenario: Item path has only name parameter
- **WHEN** the spec is generated for a dynamic item path
- **THEN** the path parameters include only `name` (type `string`, required)
- **AND** the path parameters do NOT include `group`, `version`, or `kind`

#### Scenario: Collection path has no path parameters
- **WHEN** the spec is generated for a dynamic collection path
- **THEN** the path has no path parameters
- **AND** the `?watch=true` query parameter is documented on the GET operation

### Requirement: Watch query parameter documented on list endpoint
The `GET` list endpoint for each kind SHALL document the `?watch=true` query parameter as an optional boolean. When `watch=true`, the response is an SSE stream of `WatchEvent` objects.

#### Scenario: Watch parameter appears in spec
- **WHEN** the spec is generated for a dynamic kind
- **THEN** the GET list path has a query parameter `watch` of type `boolean`, not required

### Requirement: fieldSelector and labelSelector query parameters documented on list endpoint
The generated OpenAPI 3.0.3 spec SHALL include the `fieldSelector` and `labelSelector` query parameters on the list endpoint (not just on watch). Both parameters SHALL be typed as `string` and marked as optional.

#### Scenario: fieldSelector parameter in OpenAPI spec
- **WHEN** the OpenAPI spec is generated
- **THEN** the list endpoint SHALL include a `fieldSelector` query parameter of type `string`

#### Scenario: labelSelector parameter in OpenAPI spec
- **WHEN** the OpenAPI spec is generated
- **THEN** the list endpoint SHALL include a `labelSelector` query parameter of type `string`

#### Scenario: fieldSelector parameter description
- **WHEN** the OpenAPI spec is generated
- **THEN** the `fieldSelector` parameter description SHALL indicate it is valid on both list and watch requests

#### Scenario: labelSelector parameter description
- **WHEN** the OpenAPI spec is generated
- **THEN** the `labelSelector` parameter SHALL have a description explaining the supported syntax (equality, inequality, existence, non-existence, AND)

### Requirement: Swagger UI reflects selectors on list
The Swagger UI SHALL display both `fieldSelector` and `labelSelector` query parameters on the list endpoint.

#### Scenario: Swagger UI shows fieldSelector input
- **WHEN** a user views the list endpoint in Swagger UI
- **THEN** the parameter list SHALL include `fieldSelector` as an optional string input

#### Scenario: Swagger UI shows labelSelector input
- **WHEN** a user views the list endpoint in Swagger UI
- **THEN** the parameter list SHALL include `labelSelector` as an optional string input

### Requirement: Response codes documented for all operations
All dynamic paths SHALL document appropriate HTTP response codes:
- POST: 201 (Created), 404 (NotFound for unregistered kind), 409 (Conflict for version mismatch), 409 (AlreadyExists for duplicate), 422 (SchemaValidation)
- GET (item): 200 (OK), 404 (NotFound)
- PUT: 200 (OK), 404 (NotFound), 409 (Conflict for version mismatch), 422 (SchemaValidation)
- DELETE: 200 (OK), 404 (NotFound), 409 (SchemaHasObjects for Schema deletion)
- GET (list): 200 (OK)

#### Scenario: POST documents error responses
- **WHEN** the spec is generated for a dynamic kind
- **THEN** the POST operation documents 404, 409 (Conflict), 409 (AlreadyExists), and 422 response schemas referencing `AppError`

#### Scenario: POST documents AlreadyExists response
- **WHEN** the spec is generated for a dynamic kind
- **THEN** the POST operation includes a 409 response with `code: "AlreadyExists"` and `details` containing `kind` and `name` fields

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

