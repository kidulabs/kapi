## MODIFIED Requirements

### Requirement: OpenAPI StoredObject component uses spec field
The OpenAPI `StoredObject` component schema SHALL use `"spec"` as the JSON key for the user-defined data field, referencing `#/components/schemas/SpecData`. The `required` array SHALL include `"spec"` instead of `"data"`.

#### Scenario: StoredObject component has spec field
- **WHEN** the OpenAPI spec is generated
- **THEN** the `StoredObject` component contains `"spec": { "$ref": "#/components/schemas/SpecData" }`
- **AND** the `required` array includes `"spec"`

### Requirement: OpenAPI SpecData component replaces UserData
The OpenAPI component previously named `UserData` SHALL be renamed to `SpecData`. It SHALL contain a single `value` property of type `object` with `additionalProperties: true`.

#### Scenario: SpecData component in OpenAPI spec
- **WHEN** the OpenAPI spec is generated
- **THEN** a `SpecData` component exists with `"value": { "type": "object", "additionalProperties": true }`

### Requirement: Dynamic kind components use spec field
The `build_kind_spec_component` function (previously `build_kind_data_component`) SHALL generate OpenAPI component schemas for registered kinds that reference the kind-specific spec schema. The `build_kind_stored_object_component` function SHALL reference the kind-specific spec component via the `"spec"` key.

#### Scenario: Kind-specific StoredObject component references spec
- **WHEN** a kind "Widget" is registered with group "example.io"
- **THEN** the generated `WidgetExampleIoStoredObject` component contains `"spec": { "$ref": "#/components/schemas/WidgetExampleIo" }`
- **AND** the `required` array includes `"spec"`

### Requirement: Dynamic kind paths use spec in request schemas
The `build_kind_paths` function SHALL generate create request schemas that combine `metadata` with the kind-specific spec schema. The kind-specific spec component SHALL be named using the pattern `{Kind}{Group}SpecData` (previously `{Kind}{Group}Data`).

#### Scenario: Create request schema for registered kind
- **WHEN** a kind "Widget" is registered
- **THEN** the create request schema combines `metadata` properties with the Widget spec schema