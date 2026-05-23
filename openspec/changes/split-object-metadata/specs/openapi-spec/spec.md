## MODIFIED Requirements

### Requirement: Static component schemas cover kapi built-in types
The generated OpenAPI spec SHALL include component schemas for: `ResourceKey`, `ObjectMeta`, `SystemMetadata`, `UserData`, `StoredObject`, `ListResponse`, `WatchEvent`, `WatchEventType`, `ValidationError`, `AppError`, and `SchemaData`.

#### Scenario: StoredObject component matches wire format
- **WHEN** the spec is generated
- **THEN** the `StoredObject` component has properties `key` (ref ResourceKey), `metadata` (ref ObjectMeta), `system` (ref SystemMetadata), and `data` (ref UserData)
- **AND** the `ObjectMeta` component has properties `name` (type string, required)
- **AND** the `SystemMetadata` component has properties `resourceVersion` (type integer, format int64, required), `createdAt` (type string, format date-time, required), and `updatedAt` (type string, format date-time, required)

#### Scenario: AppError component covers all variants
- **WHEN** the spec is generated
- **THEN** the `AppError` component represents the error response shape with `error`, `code`, and `details` fields

#### Scenario: Dynamic StoredObject references ObjectMeta and SystemMetadata
- **WHEN** the spec is generated for a registered kind
- **THEN** `{Kind}{Group}StoredObject` has a `metadata` property referencing `ObjectMeta` and a `system` property referencing `SystemMetadata`