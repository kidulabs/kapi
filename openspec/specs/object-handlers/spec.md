## Purpose

Define the Axum HTTP handlers and route wiring that expose the object CRUD API and SSE watch endpoint. Handlers are thin — they extract parameters, call the service, and return responses.
## Requirements
### Requirement: Create handler accepts POST to namespace-scoped and cluster-scoped URLs
The create handler SHALL support two URL patterns based on Schema scope:
- Namespaced: `POST /apis/{group}/{version}/namespaces/{namespace}/{kind}`
- Cluster-scoped: `POST /apis/{group}/{version}/{kind}`
- Cross-namespace (namespaced kinds only): `POST /apis/{group}/{version}/{kind}` — creates in "default" namespace

The handler SHALL extract `group`, `version`, `kind`, and optionally `namespace` from the path. The handler SHALL pass the extracted namespace (if any) to the service layer. The handler SHALL NOT look up Schema scope — that is the service's responsibility.

For Schema objects (`kind == "Schema"`), the handler SHALL extract `targetKind` and `targetGroup` from the body, generate the name as `{targetKind}.{targetGroup}`, construct an `ObjectMeta` with that name and any `labels` from `metadata.labels`, and call `SchemaService::create(key, meta, spec)`. Schema objects are cluster-scoped.

For non-Schema objects, the handler SHALL extract the object `name` from the body's `metadata.name` field and `labels` from `metadata.labels` (defaulting to empty if absent), extract the `spec` field from the body, construct an `ObjectMeta` with those values, and call `ObjectService::create(key, namespace, meta, spec)`. The handler SHALL discard `metadata.namespace` from the body — the URL namespace takes precedence.

The handler SHALL validate that the request body contains only `metadata` and `spec` as top-level fields; any other field SHALL return `AppError::InvalidRequestBody`. The handler SHALL validate that `spec` is present and is a JSON object; if missing or not an object, the handler SHALL return `AppError::InvalidRequestBody`. The handler SHALL validate that `spec` is non-empty; if `spec` is an empty object `{}`, the handler SHALL return `AppError::InvalidRequestBody`.

The handler SHALL NOT perform label, annotation, or finalizer format validation. Format validation is the responsibility of the service layer.

#### Scenario: Successful namespaced create returns 201
- **WHEN** a non-Schema object is POSTed to `/apis/example.io/v1/namespaces/production/Widget` with `metadata.name` and `spec`
- **THEN** the handler SHALL pass `namespace = Some("production")` to the service
- **AND** the response is 201 Created

#### Scenario: Create without namespace defaults to "default"
- **WHEN** a non-Schema object is POSTed to `/apis/example.io/v1/Widget` (namespaced kind)
- **THEN** the handler SHALL pass `namespace = None` to the service
- **AND** the service SHALL create the object in "default" namespace

#### Scenario: Create discards metadata.namespace
- **WHEN** a POST request includes `metadata.namespace: "staging"` but URL has `namespaces/production`
- **THEN** the handler SHALL discard `metadata.namespace` and pass URL namespace to service

#### Scenario: Successful Schema create returns 201 with generated name
- **WHEN** a Schema is POSTed to `/apis/kapi.io/v1/Schema` with `targetKind: "Widget"` and `targetGroup: "example.io"`
- **THEN** the response is 201 Created with `metadata.name` set to `"Widget.example.io"`

#### Scenario: Create with missing spec returns 400
- **WHEN** a non-Schema object is POSTed without a `spec` field
- **THEN** the response is 400 Bad Request with `InvalidRequestBody` error

#### Scenario: Create with empty spec returns 400
- **WHEN** a non-Schema object is POSTed with `spec: {}`
- **THEN** the response is 400 Bad Request with `InvalidRequestBody` error

#### Scenario: Create with unknown top-level field returns 400
- **WHEN** a non-Schema object is POSTed with a top-level field other than `metadata` or `spec`
- **THEN** the response is 400 Bad Request with `InvalidRequestBody` error

### Requirement: Get handler accepts GET to namespace-scoped and cluster-scoped URLs
The get handler SHALL support two URL patterns:
- Namespaced: `GET /apis/{group}/{version}/namespaces/{namespace}/{kind}/{name}`
- Cluster-scoped: `GET /apis/{group}/{version}/{kind}/{name}`

The handler SHALL extract path parameters including optional `namespace` and call `ObjectService::get(key, namespace, name)` or `SchemaService::get(key, name)` for Schema objects.

#### Scenario: Successful namespaced get returns 200
- **WHEN** an existing object is GETed at `/apis/example.io/v1/namespaces/production/Widget/my-widget`
- **THEN** the response is 200 OK with the `StoredObject` as JSON

#### Scenario: Successful cluster-scoped get returns 200
- **WHEN** an existing cluster-scoped object is GETed at `/apis/kapi.io/v1/Schema/Widget.example.io`
- **THEN** the response is 200 OK with the `StoredObject` as JSON

#### Scenario: Get missing object returns 404
- **WHEN** a non-existent object is GETed
- **THEN** the response is 404 with `NotFound` error

### Requirement: List handler supports namespace-scoped, cluster-scoped, and cross-namespace modes
The list handler SHALL support three URL patterns:
- Namespaced: `GET /apis/{group}/{version}/namespaces/{namespace}/{kind}` — list in specific namespace
- Cluster-scoped: `GET /apis/{group}/{version}/{kind}` — list cluster-scoped objects
- Cross-namespace: `GET /apis/{group}/{version}/{kind}` — list namespaced objects across all namespaces

The handler SHALL extract path parameters including optional `namespace` and pass to the service. The handler SHALL check for `?watch=true` query parameter. If present, it SHALL parse the `fieldSelector` and `labelSelector` query parameters into a `WatchFilter`, subscribe to the event bus with the filter, and return an SSE stream. When both `fieldSelector` and `labelSelector` are present on a watch request, the handler SHALL combine them with `WatchFilter::And`.

#### Scenario: Namespaced list returns objects in namespace
- **WHEN** GET `/apis/example.io/v1/namespaces/production/Widget`
- **THEN** the response is 200 OK with objects from "production" namespace only

#### Scenario: Cross-namespace list returns all objects
- **WHEN** GET `/apis/example.io/v1/Widget` (namespaced kind)
- **THEN** the response is 200 OK with objects from all namespaces

#### Scenario: Cluster-scoped list returns cluster objects
- **WHEN** GET `/apis/kapi.io/v1/Schema` (cluster-scoped kind)
- **THEN** the response is 200 OK with cluster-scoped Schema objects

#### Scenario: Watch returns SSE stream
- **WHEN** GET `/apis/example.io/v1/namespaces/production/Widget?watch=true`
- **THEN** the response is an SSE stream of `WatchEvent` objects

#### Scenario: Watch with fieldSelector filters by name
- **WHEN** GET `/apis/example.io/v1/namespaces/production/Widget?watch=true&fieldSelector=metadata.name=my-widget`
- **THEN** the SSE stream only delivers events for objects with `metadata.name == "my-widget"`

#### Scenario: List with labelSelector returns filtered results
- **WHEN** GET `/apis/example.io/v1/namespaces/production/Widget?labelSelector=app=nginx`
- **THEN** the response is 200 OK with objects matching the label selector

### Requirement: Update handler accepts PUT to namespace-scoped and cluster-scoped URLs
The update handler SHALL support two URL patterns:
- Namespaced: `PUT /apis/{group}/{version}/namespaces/{namespace}/{kind}/{name}`
- Cluster-scoped: `PUT /apis/{group}/{version}/{kind}/{name}`

The handler SHALL extract path parameters including optional `namespace`, deserialize the body as `StoredObject`, and validate that the URL `key`, `namespace`, and `name` match the object's `key`, `metadata.namespace`, and `metadata.name`. For namespace-scoped updates, if `metadata.namespace` is present and does not match the URL namespace, the handler SHALL return 400 Bad Request. If `metadata.namespace` is absent, the handler SHALL set it from the URL namespace.

For Schema objects (`kind == "Schema"`), the handler SHALL call `SchemaService::update(object)`. For non-Schema objects, the handler SHALL call `ObjectService::update(object)`.

The handler SHALL NOT modify `system` fields; the `resourceVersion` in `system` is used by the store for optimistic concurrency control.

#### Scenario: Successful update returns 200
- **WHEN** an object is PUT with a matching `system.resourceVersion`
- **THEN** the response is 200 OK with the updated `StoredObject`

#### Scenario: Update with mismatched namespace returns 400
- **WHEN** the URL namespace does not match the object's `metadata.namespace`
- **THEN** the response is 400 Bad Request

#### Scenario: Update with absent namespace in body uses URL namespace
- **WHEN** the URL has `namespaces/production` and body has no `metadata.namespace`
- **THEN** the handler SHALL set `metadata.namespace = Some("production")` before passing to service

#### Scenario: Update with wrong version returns 409
- **WHEN** an object is PUT with a stale `system.resourceVersion`
- **THEN** the response is 409 with `Conflict` error

### Requirement: Delete handler accepts DELETE to namespace-scoped and cluster-scoped URLs
The delete handler SHALL support two URL patterns:
- Namespaced: `DELETE /apis/{group}/{version}/namespaces/{namespace}/{kind}/{name}`
- Cluster-scoped: `DELETE /apis/{group}/{version}/{kind}/{name}`

The handler SHALL extract path parameters including optional `namespace`. For namespaced kinds, the namespace is required in the URL. The handler SHALL call `ObjectService::delete(key, namespace, name)` or `SchemaService::delete(key, name)` for Schema objects.

#### Scenario: Successful delete returns 200
- **WHEN** an existing object is DELETEd
- **THEN** the response is 200 OK with the deleted `StoredObject` as JSON

#### Scenario: Delete Schema with objects returns 409
- **WHEN** a Schema is DELETEd and objects of the target kind exist
- **THEN** the response is 409 with `SchemaHasObjects` error

### Requirement: GET /status handler
The system SHALL provide a `get_status` handler for `GET /apis/{group}/{version}/{kind}/{name}/status` that extracts path parameters, calls `ObjectService::get_status`, and returns the status value as JSON. If the kind does not have a `statusSchema`, the handler SHALL return `404 Not Found` with `StatusSubresourceNotEnabled`.

#### Scenario: Get status for kind with statusSchema
- **WHEN** a GET request is made to `/status` for a kind with `statusSchema`
- **THEN** the handler returns `200 OK` with the status value as JSON

#### Scenario: Get status for kind without statusSchema
- **WHEN** a GET request is made to `/status` for a kind without `statusSchema`
- **THEN** the handler returns `404 Not Found` with `StatusSubresourceNotEnabled` error

### Requirement: PUT /status handler
The system SHALL provide an `update_status` handler for `PUT /apis/{group}/{version}/{kind}/{name}/status` that extracts path parameters, deserializes the request body as JSON, extracts the `status` field, and calls `ObjectService::update_status`. The handler SHALL return `200 OK` with the full `StoredObject` on success. If the kind does not have a `statusSchema`, the handler SHALL return `404 Not Found` with `StatusSubresourceNotEnabled`.

#### Scenario: Update status for kind with statusSchema
- **WHEN** a PUT request is made to `/status` with a valid status body for a kind with `statusSchema`
- **THEN** the handler returns `200 OK` with the full `StoredObject`

#### Scenario: Update status for kind without statusSchema
- **WHEN** a PUT request is made to `/status` for a kind without `statusSchema`
- **THEN** the handler returns `404 Not Found` with `StatusSubresourceNotEnabled` error

### Requirement: Handler principle
The module documentation in `src/object/handler.rs` SHALL state: "Handlers extract parameters from HTTP requests, perform deserialization and structural validation (required fields, type checks), and delegate to the appropriate service. They never access the store, event bus, or schema registry directly. They do not perform domain format validation (labels, annotations, finalizers) — that is the service layer's responsibility."

#### Scenario: Handler module doc reflects principle
- **WHEN** the handler module documentation is read
- **THEN** it SHALL describe parameter extraction and structural validation as handler responsibilities, domain format validation as a service responsibility, and direct store/bus/registry access as prohibited

### Requirement: Routes support namespace-scoped and cluster-scoped patterns
The router SHALL define:
- `GET/POST /apis/{group}/{version}/namespaces/{namespace}/{kind}` → list/create handlers (namespaced)
- `GET/PUT/DELETE /apis/{group}/{version}/namespaces/{namespace}/{kind}/{name}` → get/update/delete handlers (namespaced)
- `GET/POST /apis/{group}/{version}/{kind}` → list/create handlers (cluster-scoped or cross-namespace)
- `GET/PUT/DELETE /apis/{group}/{version}/{kind}/{name}` → get/update/delete handlers (cluster-scoped)
- `GET /apis/{group}/{version}/namespaces/{namespace}/{kind}/{name}/status` → `get_status` handler
- `PUT /apis/{group}/{version}/namespaces/{namespace}/{kind}/{name}/status` → `update_status` handler
- `GET /apis/{group}/{version}/{kind}/{name}/status` → `get_status` handler (cluster-scoped)
- `PUT /apis/{group}/{version}/{kind}/{name}/status` → `update_status` handler (cluster-scoped)

Path parameters `group`, `version`, `kind`, `namespace`, and `name` SHALL be extracted using Axum's `Path` extractor.

#### Scenario: Route matches namespaced object path
- **WHEN** a GET request is sent to `/apis/example.io/v1/namespaces/production/Widget/my-widget`
- **THEN** the get handler is invoked with `group="example.io"`, `version="v1"`, `kind="Widget"`, `namespace="production"`, `name="my-widget"`

#### Scenario: Route matches cluster-scoped object path
- **WHEN** a GET request is sent to `/apis/kapi.io/v1/Schema/Widget.example.io`
- **THEN** the get handler is invoked with `group="kapi.io"`, `version="v1"`, `kind="Schema"`, `name="Widget.example.io"`

#### Scenario: Route matches cross-namespace list path
- **WHEN** a GET request is sent to `/apis/example.io/v1/Widget`
- **THEN** the list handler is invoked with `group="example.io"`, `version="v1"`, `kind="Widget"`, `namespace=None`

### Requirement: SSE events use axum::response::sse::Event
Watch events SHALL be formatted as SSE events using `axum::response::sse::Event` with the `WatchEvent` serialized as JSON in the event data field.

#### Scenario: SSE event format
- **WHEN** a watch event is sent
- **THEN** the SSE output is:
  ```
  event: message
  data: {"event_type":"Added","object":{...}}

  ```

### Requirement: Handler location
All object handlers SHALL be defined in `src/object/handler.rs`. Route composition SHALL be defined in `src/routes.rs`.

#### Scenario: Handlers are in the correct module
- **WHEN** the project is built
- **THEN** `src/object/handler.rs` contains all object CRUD handlers and `src/routes.rs` contains route composition

