## Purpose

Define the Schema scope field that determines whether resources are cluster-scoped or namespace-scoped, and how scope drives URL structure and validation.

## Requirements

### Requirement: Schema defines scope field
The system SHALL support a `scope` field in Schema definitions. The `scope` field SHALL be a string with two valid values: `"Namespaced"` and `"Cluster"`. The default value SHALL be `"Namespaced"` when the field is absent. The `scope` field SHALL be serialized as `scope` in JSON (no rename).

#### Scenario: Schema with explicit scope
- **WHEN** a Schema is created with `scope: "Cluster"`
- **THEN** the Schema SHALL be stored with `scope = "Cluster"`
- **AND** objects of this kind SHALL be cluster-scoped

#### Scenario: Schema without scope defaults to Namespaced
- **WHEN** a Schema is created without a `scope` field
- **THEN** the Schema SHALL be treated as `scope = "Namespaced"`
- **AND** objects of this kind SHALL require a namespace

#### Scenario: Schema with invalid scope value
- **WHEN** a Schema is created with `scope: "Invalid"`
- **THEN** the system SHALL reject the Schema with an appropriate error

### Requirement: Schema scope determines URL structure
The system SHALL use the Schema `scope` field to determine the URL structure for resources of that kind. Cluster-scoped resources SHALL use `/apis/{group}/{version}/{kind}[/{name}]`. Namespaced resources SHALL use `/apis/{group}/{version}/namespaces/{namespace}/{kind}[/{name}]` for namespace-scoped operations and `/apis/{group}/{version}/{kind}` for cross-namespace list.

#### Scenario: Cluster-scoped kind uses flat URL
- **WHEN** a kind has `scope: "Cluster"`
- **THEN** CRUD operations SHALL use `/apis/{group}/{version}/{kind}[/{name}]`

#### Scenario: Namespaced kind uses namespace URL
- **WHEN** a kind has `scope: "Namespaced"`
- **THEN** namespace-scoped CRUD operations SHALL use `/apis/{group}/{version}/namespaces/{namespace}/{kind}[/{name}]`

#### Scenario: Namespaced kind supports cross-namespace list
- **WHEN** a kind has `scope: "Namespaced"`
- **THEN** a GET to `/apis/{group}/{version}/{kind}` SHALL return objects from all namespaces

### Requirement: Service validates scope vs URL pattern
The service layer SHALL validate that the URL pattern matches the Schema scope. If a cluster-scoped kind is accessed with a namespace in the URL, the service SHALL reject the request. If a namespaced kind requires a namespace for get/update/delete but none is provided, the service SHALL reject the request.

#### Scenario: Cluster-scoped kind with namespace in URL
- **WHEN** a request is made to `/apis/{group}/{version}/namespaces/{ns}/{kind}` for a cluster-scoped kind
- **THEN** the service SHALL reject the request with an appropriate error

#### Scenario: Namespaced kind get without namespace
- **WHEN** a GET request is made to `/apis/{group}/{version}/{kind}/{name}` for a namespaced kind
- **THEN** the service SHALL reject the request (namespace required)

#### Scenario: Namespaced kind create without namespace defaults to "default"
- **WHEN** a POST request is made to `/apis/{group}/{version}/{kind}` for a namespaced kind
- **THEN** the object SHALL be created in the `"default"` namespace

### Requirement: Schema scope is included in SchemaData
The `SchemaData` struct SHALL include a `scope` field of type `String`. The field SHALL default to `"Namespaced"` when absent during deserialization. The field SHALL be serialized as `scope` in JSON.

#### Scenario: SchemaData serialization with scope
- **WHEN** a `SchemaData` with `scope: "Cluster"` is serialized
- **THEN** the JSON SHALL contain `"scope": "Cluster"`

#### Scenario: SchemaData deserialization without scope
- **WHEN** a `SchemaData` JSON without `scope` field is deserialized
- **THEN** the resulting struct SHALL have `scope = "Namespaced"`

### Requirement: Schema scope is cached in SchemaRegistry
The `SchemaRegistry` SHALL cache the scope alongside the compiled validator. The `get_validator` method SHALL return both the validator and the scope. Alternatively, a separate `get_scope` method SHALL provide scope lookup.

#### Scenario: Scope lookup returns cached scope
- **WHEN** `get_scope(key)` is called for a registered kind
- **THEN** the scope SHALL be returned without store access (if cached)

#### Scenario: Scope lookup on cache miss
- **WHEN** `get_scope(key)` is called for a kind not in cache
- **THEN** the Schema SHALL be fetched from the store, scope extracted, and returned
