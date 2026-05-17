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
- **WHEN** a Schema is registered for kind `Widget` in group `example.io`
- **AND** `GET /openapi` is called
- **THEN** the spec contains paths for `/apis/example.io/v1/Widget` and `/apis/example.io/v1/Widget/{name}`
- **AND** the spec contains component schemas `WidgetExampleIo`, `WidgetExampleIoStoredObject`, and `WidgetExampleIoListResponse`

#### Scenario: Spec reflects current state at request time
- **WHEN** a Schema is registered after a previous `/openapi` call
- **AND** `GET /openapi` is called again
- **THEN** the new response includes the newly registered kind's paths and components

### Requirement: Static component schemas cover kapi built-in types
The generated OpenAPI spec SHALL include component schemas for: `ResourceKey`, `ObjectMetadata`, `UserData`, `StoredObject`, `ListResponse`, `WatchEvent`, `WatchEventType`, `ValidationError`, `AppError`, and `SchemaData`.

#### Scenario: StoredObject component matches wire format
- **WHEN** the spec is generated
- **THEN** the `StoredObject` component has properties `key` (ref ResourceKey), `metadata` (ref ObjectMetadata), and `data` (ref UserData)

#### Scenario: AppError component covers all variants
- **WHEN** the spec is generated
- **THEN** the `AppError` component represents the error response shape with `error`, `code`, and `details` fields

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

#### Scenario: POST path uses kind-specific data schema
- **WHEN** the spec is generated for a registered kind
- **THEN** the POST request body references the kind's data component (e.g., `WidgetExampleIo`) via `allOf` with `metadata.name`

#### Scenario: GET/PUT/DELETE responses use kind-specific StoredObject schema
- **WHEN** the spec is generated for a registered kind
- **THEN** the 200 responses reference the kind's StoredObject component (e.g., `WidgetExampleIoStoredObject`)

### Requirement: Dynamic component schemas embed user's jsonSchema
For each registered Schema, the system SHALL generate a component schema from the Schema's `jsonSchema` field. This component represents the user's data shape and is referenced by the kind's `StoredObject` component's `data` property.

#### Scenario: User schema properties appear in component
- **WHEN** a Schema is registered with `jsonSchema: { "type": "object", "properties": { "color": { "type": "string" }, "size": { "type": "integer" } } }`
- **THEN** the generated component (e.g., `WidgetExampleIo`) has `type: "object"` with `properties` containing `color` and `size`

#### Scenario: Kind-specific StoredObject references kind-specific data
- **WHEN** the spec is generated for a registered kind
- **THEN** `{Kind}{Group}StoredObject` has a `data` property with `$ref` pointing to `#/components/schemas/{Kind}{Group}`

### Requirement: Component names follow PascalCase dot-split convention
Component names SHALL be derived from the schema name (format: `{Kind}.{group}`) by splitting on dots, PascalCasing each segment, and concatenating. Example: `"Widget.other.io"` → `"WidgetOtherIo"`.

#### Scenario: Single-dot schema name
- **WHEN** schema name is `"Widget.example.io"`
- **THEN** component name is `"WidgetExampleIo"`

#### Scenario: Multi-segment group name
- **WHEN** schema name is `"Deployment.apps.v1"`
- **THEN** component name is `"DeploymentAppsV1"`

#### Scenario: No collision between same kind different groups
- **WHEN** schemas `"Widget.example.io"` and `"Widget.other.io"` are both registered
- **THEN** component names are `"WidgetExampleIo"` and `"WidgetOtherIo"` respectively

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

### Requirement: Response codes documented for all operations
All dynamic paths SHALL document appropriate HTTP response codes:
- POST: 201 (Created), 404 (NotFound for unregistered kind), 409 (Conflict for duplicate), 422 (SchemaValidation)
- GET (item): 200 (OK), 404 (NotFound)
- PUT: 200 (OK), 404 (NotFound), 409 (Conflict for version mismatch), 422 (SchemaValidation)
- DELETE: 200 (OK), 404 (NotFound), 409 (SchemaHasObjects for Schema deletion)
- GET (list): 200 (OK)

#### Scenario: POST documents error responses
- **WHEN** the spec is generated for a dynamic kind
- **THEN** the POST operation documents 404, 409, and 422 response schemas referencing `AppError`

### Requirement: GET /swagger-ui/ serves HTML page (optional)
If implemented, the system SHALL provide a `GET /swagger-ui/` endpoint that returns an HTML page loading Swagger UI from a CDN and configured to fetch the spec from `/openapi`.

#### Scenario: Swagger UI page loads
- **WHEN** `GET /swagger-ui/` is requested in a browser
- **THEN** an HTML page is returned that loads Swagger UI and points to `/openapi`

