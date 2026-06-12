## MODIFIED Requirements

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

### Requirement: Dynamic component schemas embed user's jsonSchema
For each registered Schema, the system SHALL generate a component schema from the Schema's `jsonSchema` field. This component represents the user's data shape and is referenced by the kind's `StoredObject` component's `spec` property. The kind-specific spec component SHALL be the user's specSchema directly, with no `value` wrapper or envelope.

#### Scenario: User schema properties appear in component
- **WHEN** a Schema is registered with `jsonSchema: { "type": "object", "properties": { "color": { "type": "string" }, "size": { "type": "integer" } } }`
- **THEN** the generated component (e.g., `WidgetExampleIo`) has `type: "object"` with `properties` containing `color` and `size` as top-level properties (not nested under a `value` key)

#### Scenario: Kind-specific StoredObject references kind-specific spec
- **WHEN** the spec is generated for a registered kind
- **THEN** `{Kind}{Group}StoredObject` has a `spec` property with `$ref` pointing to `#/components/schemas/{Kind}{Group}`

#### Scenario: Swagger UI displays user fields without indirection
- **WHEN** a user views a per-kind schema (e.g. `WidgetExampleIo`) in Swagger UI
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

## REMOVED Requirements

### Requirement: OpenAPI SpecData component
**Reason**: The `SpecData` component is being deleted as part of removing the `SpecData` envelope type. The `spec` and `status` fields on `StoredObject` are now unconstrained JSON values, with no wrapper component needed.
**Migration**: Remove the `SpecData` component from the OpenAPI document. Replace `$ref: "#/components/schemas/SpecData"` references in the `StoredObject` component with unconstrained JSON values (e.g., `{ "description": "..." }` or `{}`).

### Requirement: Per-kind spec component wraps user schema in value envelope
**Reason**: The per-kind spec component (e.g. `WidgetExampleIo`) currently wraps the user's specSchema in `{ value: <userSchema> }`. This wraps the user-facing API in a `value` indirection that doesn't exist on the wire and is being removed.
**Migration**: The `build_kind_spec_component` function SHALL return the user's specSchema directly as the component value, without a `value` wrapper. The kind-specific component name (e.g. `WidgetExampleIo`) and the `$ref` chain (`{Kind}{Group}StoredObject.spec → {Kind}{Group}`) are preserved.
