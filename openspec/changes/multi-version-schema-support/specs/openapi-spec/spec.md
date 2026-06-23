## MODIFIED Requirements

### Requirement: GET /openapi returns dynamically generated OpenAPI 3.0.3 JSON
The system SHALL provide a `GET /openapi` endpoint that returns an OpenAPI 3.0.3 specification as `application/json`. The spec SHALL be generated on every request by listing all Schema objects from the store and building the document from scratch. Each registered Schema produces a unique set of component schemas derived from its versioned name.

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
- **WHEN** the spec is generated for a registered kind at `example.io/v1/Widget`
- **THEN** the POST request body references the kind's versioned spec component (`WidgetExampleIoV1`) via `allOf` with `metadata.name`

#### Scenario: GET/PUT/DELETE responses use kind-specific StoredObject schema
- **WHEN** the spec is generated for a registered kind at `example.io/v1/Widget`
- **THEN** the 200 responses reference the kind's versioned StoredObject component (`WidgetExampleIoV1StoredObject`)

### Requirement: Dynamic component schemas embed user's specSchema
For each registered Schema, the system SHALL generate a component schema from the Schema's `specSchema` field. This component represents the user's data shape and is referenced by the kind's `StoredObject` component's `spec` property. The kind-specific spec component SHALL be the user's specSchema directly, with no `value` wrapper or envelope.

#### Scenario: User schema properties appear in component
- **WHEN** a Schema is registered with `specSchema: { "type": "object", "properties": { "color": { "type": "string" }, "size": { "type": "integer" } } }` at `example.io/v1/Widget`
- **THEN** the generated component `WidgetExampleIoV1` has `type: "object"` with `properties` containing `color` and `size` as top-level properties (not nested under a `value` key)

#### Scenario: Kind-specific StoredObject references kind-specific spec
- **WHEN** the spec is generated for `example.io/v1/Widget`
- **THEN** `WidgetExampleIoV1StoredObject` has a `spec` property with `$ref` pointing to `#/components/schemas/WidgetExampleIoV1`

#### Scenario: Two versions can have different spec shapes
- **WHEN** the v1 Schema's `specSchema` is `{ "type": "object", "properties": { "color": { "type": "string" } } }` and the v2 Schema's `specSchema` is `{ "type": "object", "properties": { "weight": { "type": "number" } } }`
- **THEN** `WidgetExampleIoV1` exposes `color` and `WidgetExampleIoV2` exposes `weight`
- **AND** neither component contains the other version's properties

#### Scenario: Swagger UI displays user fields without indirection
- **WHEN** a user views a per-kind schema (e.g. `WidgetExampleIoV1`) in Swagger UI
- **THEN** the schema SHALL expand to show the user's fields (`color`, `size`) at the top level
- **AND** the user SHALL NOT have to click through a `value` wrapper to see the fields

### Requirement: Component names follow PascalCase dot-split convention with version
Component names SHALL be derived from the schema name (format: `{Kind}.{group}.{version}`) by splitting on dots, PascalCasing each segment, and concatenating. Example: `"Widget.other.io.v1"` → `"WidgetOtherIoV1"`. The `version` segment flows through the same PascalCase transform as `kind` and `group` segments (e.g., `"v1alpha1"` → `"V1alpha1"`).

#### Scenario: Single-dot schema name with version
- **WHEN** schema name is `"Widget.example.io.v1"`
- **THEN** component name is `"WidgetExampleIoV1"`

#### Scenario: Multi-segment group name with version
- **WHEN** schema name is `"Deployment.apps.v1"`
- **THEN** component name is `"DeploymentAppsV1"`

#### Scenario: Same kind and group, different versions produce distinct component names
- **WHEN** schemas `"Widget.example.io.v1"` and `"Widget.example.io.v2"` are both registered
- **THEN** component names are `"WidgetExampleIoV1"` and `"WidgetExampleIoV2"` respectively

#### Scenario: No collision between same kind different groups
- **WHEN** schemas `"Widget.example.io.v1"` and `"Widget.other.io.v1"` are both registered
- **THEN** component names are `"WidgetExampleIoV1"` and `"WidgetOtherIoV1"` respectively
