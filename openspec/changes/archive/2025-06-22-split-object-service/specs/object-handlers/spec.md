## MODIFIED Requirements

### Requirement: Create handler accepts POST to /apis/{group}/{version}/{kind}
The create handler SHALL extract `group`, `version`, and `kind` from the path. For Schema objects (`kind == "Schema"`), the handler SHALL extract `targetKind` and `targetGroup` from the body, generate the name as `{targetKind}.{targetGroup}`, construct an `ObjectMeta` with that name and any `labels` from `metadata.labels`, and call `SchemaService::create(key, meta, spec)`. If `targetKind` or `targetGroup` is missing or not a string, the handler SHALL return `AppError::InvalidSchema`. For non-Schema objects, the handler SHALL extract the object `name` from the body's `metadata.name` field and `labels` from `metadata.labels` (defaulting to empty if absent), extract the `spec` field from the body, construct an `ObjectMeta` with those values, and call `ObjectService::create(key, meta, spec)`. The handler SHALL validate that the request body contains only `metadata` and `spec` as top-level fields; any other field SHALL return `AppError::InvalidRequestBody`. The handler SHALL validate that `spec` is present and is a JSON object; if missing or not an object, the handler SHALL return `AppError::InvalidRequestBody`. The handler SHALL validate that `spec` is non-empty; if `spec` is an empty object `{}`, the handler SHALL return `AppError::InvalidRequestBody`. If `metadata.labels` is present but not an object type, the handler SHALL return an appropriate error response.

The handler SHALL NOT perform label, annotation, or finalizer format validation. Format validation is the responsibility of the service layer.

#### Scenario: Successful Schema create returns 201 with generated name
- **WHEN** a Schema is POSTed to `/apis/kapi.io/v1/Schema` with `targetKind: "Widget"` and `targetGroup: "example.io"`
- **THEN** the response is 201 Created with `metadata.name` set to `"Widget.example.io"`

#### Scenario: Schema create with missing targetKind returns InvalidSchema
- **WHEN** a Schema is POSTed without a `targetKind` field
- **THEN** the response is 422 with `InvalidSchema` error

#### Scenario: Schema create with missing targetGroup returns InvalidSchema
- **WHEN** a Schema is POSTed without a `targetGroup` field
- **THEN** the response is 422 with `InvalidSchema` error

#### Scenario: Successful object create returns 201
- **WHEN** a non-Schema object is POSTed to `/apis/example.io/v1/Widget` with `metadata.name` and `spec` containing domain data
- **THEN** the response is 201 Created with the `StoredObject` as JSON
- **AND** the response contains `metadata` with `name` and `system` with `resourceVersion`, `createdAt`, `updatedAt`

#### Scenario: Create object with labels
- **WHEN** a non-Schema object is POSTed with `metadata.labels: {"app": "nginx"}` and `spec` containing domain data
- **THEN** the response is 201 Created with `metadata.labels` containing the provided labels

#### Scenario: Create object without labels
- **WHEN** a non-Schema object is POSTed without `metadata.labels` but with `spec` containing domain data
- **THEN** the response is 201 Created with `metadata.labels` as an empty object

#### Scenario: Create object with invalid labels field type
- **WHEN** a POST request is received with `metadata.labels` as a non-object type (e.g., string or array)
- **THEN** the handler SHALL return an appropriate error response

#### Scenario: Create with missing spec returns 400
- **WHEN** a non-Schema object is POSTed without a `spec` field
- **THEN** the response is 400 Bad Request with `InvalidRequestBody` error

#### Scenario: Create with empty spec returns 400
- **WHEN** a non-Schema object is POSTed with `spec: {}`
- **THEN** the response is 400 Bad Request with `InvalidRequestBody` error

#### Scenario: Create with non-object spec returns 400
- **WHEN** a non-Schema object is POSTed with `spec` as a non-object type (e.g., string, array, number)
- **THEN** the response is 400 Bad Request with `InvalidRequestBody` error

#### Scenario: Create with unknown top-level field returns 400
- **WHEN** a non-Schema object is POSTed with a top-level field other than `metadata` or `spec`
- **THEN** the response is 400 Bad Request with `InvalidRequestBody` error

#### Scenario: Create with invalid data returns 422
- **WHEN** an object is POSTed that fails schema validation
- **THEN** the response is 422 with `SchemaValidation` error details

#### Scenario: Create for unregistered kind returns 404
- **WHEN** an object is POSTed for a kind with no registered Schema
- **THEN** the response is 404 with `NotFound` error

### Requirement: List handler supports both list and watch modes
The list handler SHALL check for `?watch=true` query parameter. If present, it SHALL parse the `fieldSelector` and `labelSelector` query parameters using `FieldSelector::parse()` and `LabelSelector::parse()` into a `WatchFilter`, subscribe to the event bus with the filter, and return an SSE stream. When both `fieldSelector` and `labelSelector` are present on a watch request, the handler SHALL combine them with `WatchFilter::And`. If a `fieldSelector` or `labelSelector` is present on a non-watch (list) request, the handler SHALL parse the selectors using `FieldSelector::parse()` and `LabelSelector::parse()` and pass them to `ListOptions` for store-level filtering. Invalid selectors on either list or watch SHALL return 400 Bad Request.

#### Scenario: List returns JSON
- **WHEN** GET `/apis/example.io/v1/Widget` without `?watch=true`
- **THEN** the response is 200 OK with `ListResponse` as JSON

#### Scenario: Watch returns SSE stream
- **WHEN** GET `/apis/example.io/v1/Widget?watch=true`
- **THEN** the response is an SSE stream of `WatchEvent` objects

#### Scenario: Watch with fieldSelector filters by name
- **WHEN** GET `/apis/example.io/v1/Widget?watch=true&fieldSelector=metadata.name=my-widget`
- **THEN** the SSE stream only delivers events for objects with `metadata.name == "my-widget"`

#### Scenario: Watch without fieldSelector returns all events
- **WHEN** GET `/apis/example.io/v1/Widget?watch=true`
- **THEN** the SSE stream delivers all events for the Widget kind

#### Scenario: List with fieldSelector returns filtered results
- **WHEN** GET `/apis/example.io/v1/Widget?fieldSelector=metadata.name=my-widget` (without `?watch=true`)
- **THEN** the response is 200 OK with `ListResponse` containing only objects with `metadata.name == "my-widget"`

#### Scenario: List with invalid fieldSelector returns 400
- **WHEN** GET `/apis/example.io/v1/Widget?fieldSelector=metadata.namespace=default` (without `?watch=true`)
- **THEN** the response is 400 Bad Request with `InvalidFieldSelector` error

#### Scenario: Watch with labelSelector filters by label
- **WHEN** GET `/apis/example.io/v1/Widget?watch=true&labelSelector=app=nginx`
- **THEN** the SSE stream only delivers events for objects with label `app=nginx`

#### Scenario: Watch with both fieldSelector and labelSelector
- **WHEN** GET `/apis/example.io/v1/Widget?watch=true&fieldSelector=metadata.name=foo&labelSelector=app=nginx`
- **THEN** the handler SHALL combine them with `WatchFilter::And(FieldSelector(...), LabelSelector(...))` (both must match)

#### Scenario: List with labelSelector returns filtered results
- **WHEN** GET `/apis/example.io/v1/Widget?labelSelector=app=nginx` (without `?watch=true`)
- **THEN** the response is 200 OK with `ListResponse` containing only objects with label `app=nginx`

#### Scenario: List with both selectors
- **WHEN** GET `/apis/example.io/v1/Widget?fieldSelector=metadata.name=foo&labelSelector=app=nginx` (without `?watch=true`)
- **THEN** the handler SHALL parse both selectors and pass them to `ListOptions` for store-level filtering

#### Scenario: List with invalid labelSelector returns 400
- **WHEN** GET `/apis/example.io/v1/Widget?labelSelector=app=nginx` without `?watch=true` is valid; this covers the case of an invalid selector on list
- **THEN** if the `labelSelector` value is malformed, the handler SHALL return 400 Bad Request with `InvalidLabelSelector` error

#### Scenario: Invalid labelSelector on watch returns 400
- **WHEN** GET `/apis/example.io/v1/Widget?watch=true&labelSelector=invalid selector`
- **THEN** the response is 400 Bad Request with `InvalidLabelSelector` error indicating the format is invalid

#### Scenario: Invalid fieldSelector on watch returns 400
- **WHEN** GET `/apis/example.io/v1/Widget?watch=true&fieldSelector=metadata.namespace=default`
- **THEN** the response is 400 Bad Request with `InvalidFieldSelector` error indicating the field is not supported

#### Scenario: Malformed fieldSelector returns 400
- **WHEN** GET `/apis/example.io/v1/Widget?watch=true&fieldSelector=invalid-format`
- **THEN** the response is 400 Bad Request with `InvalidFieldSelector` error indicating the format is invalid

#### Scenario: Watch events have correct SSE format
- **WHEN** an object is created while a watch is active
- **THEN** the SSE stream receives an event with `event: message` and the `WatchEvent` JSON as data

### Requirement: Update handler accepts PUT to /apis/{group}/{version}/{kind}/{name}
The update handler SHALL extract path parameters, deserialize the body as `StoredObject`, validate that the URL `key` and `name` match the object's `key` and `metadata.name`. For Schema objects (`kind == "Schema"`), the handler SHALL call `SchemaService::update(object)`. For non-Schema objects, the handler SHALL call `ObjectService::update(object)`. Labels SHALL be passed through as part of the `StoredObject` body's `metadata` field. The handler SHALL NOT modify `system` fields; the `resourceVersion` in `system` is used by the store for optimistic concurrency control.

The handler SHALL NOT perform label, annotation, or finalizer format validation. Format validation is the responsibility of the service layer.

#### Scenario: Successful update returns 200
- **WHEN** an object is PUT with a matching `system.resourceVersion`
- **THEN** the response is 200 OK with the updated `StoredObject` (new `system.resourceVersion`)

#### Scenario: Update with wrong version returns 409
- **WHEN** an object is PUT with a stale `system.resourceVersion`
- **THEN** the response is 409 with `Conflict` error

#### Scenario: Update with mismatched name returns 400
- **WHEN** the URL name does not match the object's `metadata.name`
- **THEN** the response is 400 Bad Request

#### Scenario: Update object with changed labels
- **WHEN** a PUT request is received with a `StoredObject` body containing updated `metadata.labels`
- **THEN** the handler SHALL pass the full `StoredObject` (including new labels) to the service

#### Scenario: Update object removing all labels
- **WHEN** a PUT request is received with `metadata.labels: {}`
- **THEN** the handler SHALL pass the empty labels map to the service, which SHALL remove all existing labels

### Requirement: Delete handler accepts DELETE to /apis/{group}/{version}/{kind}/{name}
The delete handler SHALL extract path parameters. For Schema objects (`kind == "Schema"`), the handler SHALL call `SchemaService::delete(key, name)`. For non-Schema objects, the handler SHALL call `ObjectService::delete(key, name)`.

#### Scenario: Successful delete returns 200
- **WHEN** an existing object is DELETEd
- **THEN** the response is 200 OK with the deleted `StoredObject` as JSON

#### Scenario: Delete Schema with objects returns 409
- **WHEN** a Schema is DELETEd and objects of the target kind exist
- **THEN** the response is 409 with `SchemaHasObjects` error including the kind

### Requirement: Handler principle
The module documentation in `src/object/handler.rs` SHALL state: "Handlers extract parameters from HTTP requests, perform deserialization and structural validation (required fields, type checks), and delegate to the appropriate service. They never access the store, event bus, or schema registry directly. They do not perform domain format validation (labels, annotations, finalizers) — that is the service layer's responsibility."

#### Scenario: Handler module doc reflects principle
- **WHEN** the handler module documentation is read
- **THEN** it SHALL describe parameter extraction and structural validation as handler responsibilities, domain format validation as a service responsibility, and direct store/bus/registry access as prohibited

## REMOVED Requirements

### Requirement: Create handler validates label format eagerly
**Reason**: Handler-level validation is redundant. The service layer validates labels for all entry points (HTTP, future gRPC, tests). Removing duplication eliminates drift risk.
**Migration**: Label validation is performed by `ObjectService::create()` and `SchemaService::create()`. Invalid labels still return `AppError::InvalidLabel` with the same HTTP 400 response.

### Requirement: Create handler validates annotation format eagerly
**Reason**: Handler-level validation is redundant. The service layer validates annotations for all entry points.
**Migration**: Annotation validation is performed by `ObjectService::create()` and `SchemaService::create()`. Invalid annotations still return `AppError::InvalidAnnotation` with the same HTTP 400 response.

### Requirement: Update handler validates label format eagerly
**Reason**: Handler-level validation is redundant. The service layer validates labels for all entry points.
**Migration**: Label validation is performed by `ObjectService::update()` and `SchemaService::update()`. Invalid labels still return `AppError::InvalidLabel` with the same HTTP 400 response.

### Requirement: Update handler validates annotation format eagerly
**Reason**: Handler-level validation is redundant. The service layer validates annotations for all entry points.
**Migration**: Annotation validation is performed by `ObjectService::update()` and `SchemaService::update()`. Invalid annotations still return `AppError::InvalidAnnotation` with the same HTTP 400 response.
