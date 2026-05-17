## Purpose

Dynamic OpenAPI 3.0.3 specification generation at request time, including static components/paths for Schema CRUD and dynamic per-kind paths/schemas from registered schemas. Module restructured from single file to directory (`src/openapi/`).

## MODIFIED Requirements

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
