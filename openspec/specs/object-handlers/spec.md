## Purpose

Define the Axum HTTP handlers and route wiring that expose the object CRUD API and SSE watch endpoint. Handlers are thin — they extract parameters, call the service, and return responses.

## Requirements

### Requirement: Create handler accepts POST to /apis/{group}/{version}/{kind}
The create handler SHALL extract `group`, `version`, and `kind` from the path, deserialize the request body as `serde_json::Value`, extract the object `name` from the body's `metadata.name` field, and call `ObjectService::create(key, name, data)`.

#### Scenario: Successful create returns 201
- **WHEN** a valid object is POSTed to `/apis/example.io/v1/Widget`
- **THEN** the response is 201 Created with the `StoredObject` as JSON

#### Scenario: Create with invalid data returns 422
- **WHEN** an object is POSTed that fails schema validation
- **THEN** the response is 422 with `SchemaValidation` error details

#### Scenario: Create for unregistered kind returns 404
- **WHEN** an object is POSTed for a kind with no registered Schema
- **THEN** the response is 404 with `NotFound` error

### Requirement: Get handler accepts GET to /apis/{group}/{version}/{kind}/{name}
The get handler SHALL extract path parameters and call `ObjectService::get(key, name)`.

#### Scenario: Successful get returns 200
- **WHEN** an existing object is GETed
- **THEN** the response is 200 OK with the `StoredObject` as JSON

#### Scenario: Get missing object returns 404
- **WHEN** a non-existent object is GETed
- **THEN** the response is 404 with `NotFound` error

### Requirement: List handler supports both list and watch modes
The list handler SHALL check for `?watch=true` query parameter. If present, it SHALL subscribe to the event bus and return an SSE stream. If absent, it SHALL call `ObjectService::list(key, opts)` and return JSON.

#### Scenario: List returns JSON
- **WHEN** GET `/apis/example.io/v1/Widget` without `?watch=true`
- **THEN** the response is 200 OK with `ListResponse` as JSON

#### Scenario: Watch returns SSE stream
- **WHEN** GET `/apis/example.io/v1/Widget?watch=true`
- **THEN** the response is an SSE stream of `WatchEvent` objects

#### Scenario: Watch events have correct SSE format
- **WHEN** an object is created while a watch is active
- **THEN** the SSE stream receives an event with `event: message` and the `WatchEvent` JSON as data

### Requirement: Update handler accepts PUT to /apis/{group}/{version}/{kind}/{name}
The update handler SHALL extract path parameters, deserialize the body as `StoredObject`, validate that the URL `key` and `name` match the object's `key` and `metadata.name`, and call `ObjectService::update(object)`.

#### Scenario: Successful update returns 200
- **WHEN** an object is PUT with a matching `resourceVersion`
- **THEN** the response is 200 OK with the updated `StoredObject` (new `resourceVersion`)

#### Scenario: Update with wrong version returns 409
- **WHEN** an object is PUT with a stale `resourceVersion`
- **THEN** the response is 409 with `Conflict` error

#### Scenario: Update with mismatched name returns 400
- **WHEN** the URL name does not match the object's `metadata.name`
- **THEN** the response is 400 Bad Request

### Requirement: Delete handler accepts DELETE to /apis/{group}/{version}/{kind}/{name}
The delete handler SHALL extract path parameters and call `ObjectService::delete(key, name)`.

#### Scenario: Successful delete returns 200
- **WHEN** an existing object is DELETEd
- **THEN** the response is 200 OK with the deleted `StoredObject` as JSON

#### Scenario: Delete Schema with objects returns 409
- **WHEN** a Schema is DELETEd and objects of the target kind exist
- **THEN** the response is 409 with `SchemaHasObjects` error including the kind and count

### Requirement: Routes are composed under /apis/{group}/{version}
The router SHALL define:
- `GET/POST /apis/{group}/{version}/{kind}` → list/create handlers
- `GET/PUT/DELETE /apis/{group}/{version}/{kind}/{name}` → get/update/delete handlers

Path parameters `group`, `version`, `kind`, and `name` SHALL be extracted using Axum's `Path` extractor.

#### Scenario: Route matches object CRUD path
- **WHEN** a POST request is sent to `/apis/example.io/v1/Widget`
- **THEN** the create handler is invoked with `group="example.io"`, `version="v1"`, `kind="Widget"`

#### Scenario: Route matches named object path
- **WHEN** a GET request is sent to `/apis/example.io/v1/Widget/my-widget`
- **THEN** the get handler is invoked with `group="example.io"`, `version="v1"`, `kind="Widget"`, `name="my-widget"`

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
