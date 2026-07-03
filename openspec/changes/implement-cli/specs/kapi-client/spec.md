## ADDED Requirements

### Requirement: Client provides low-level HTTP methods for all endpoints
The kapi-client library SHALL provide explicit methods for each kapi-server endpoint: `list`, `get`, `create`, `update`, `delete`, `get_status`, `update_status`, and `watch`. Each method SHALL map directly to one HTTP endpoint.

#### Scenario: List objects
- **WHEN** user calls `client.list(key, namespace, opts)`
- **THEN** client sends GET request to `/apis/{group}/{version}/{kind}` (or namespace-scoped route) and returns `ListResponse`

#### Scenario: Get single object
- **WHEN** user calls `client.get(key, namespace, name)`
- **THEN** client sends GET request to `/apis/{group}/{version}/{kind}/{name}` (or namespace-scoped route) and returns `StoredObject`

#### Scenario: Create object
- **WHEN** user calls `client.create(key, namespace, meta, spec)`
- **THEN** client sends POST request with JSON body and returns created `StoredObject`

#### Scenario: Update object
- **WHEN** user calls `client.update(namespace, obj)`
- **THEN** client sends PUT request with full `StoredObject` JSON and returns updated `StoredObject`

#### Scenario: Delete object
- **WHEN** user calls `client.delete(key, namespace, name)`
- **THEN** client sends DELETE request and returns deleted `StoredObject`

#### Scenario: Get status subresource
- **WHEN** user calls `client.get_status(key, namespace, name)`
- **THEN** client sends GET request to `/status` endpoint and returns `Option<Value>`

#### Scenario: Update status subresource
- **WHEN** user calls `client.update_status(key, namespace, name, status)`
- **THEN** client sends PUT request to `/status` endpoint and returns updated `StoredObject`

#### Scenario: Watch objects
- **WHEN** user calls `client.watch(key, filter)`
- **THEN** client sends GET request with `?watch=true` and returns async stream of `WatchEvent`

### Requirement: Client is route-agnostic
The client SHALL accept `namespace: Option<&str>` parameter and construct the appropriate URL. If `namespace` is `Some`, use namespace-scoped route. If `None`, use cluster-scoped route. The client SHALL NOT validate scope or reject mismatched namespace parameters.

#### Scenario: Cluster-scoped request
- **WHEN** user calls `client.get(key, None, name)`
- **THEN** client constructs URL `/apis/{group}/{version}/{kind}/{name}`

#### Scenario: Namespace-scoped request
- **WHEN** user calls `client.get(key, Some("default"), name)`
- **THEN** client constructs URL `/apis/{group}/{version}/namespaces/default/{kind}/{name}`

### Requirement: Client uses reqwest for HTTP
The client SHALL use the `reqwest` crate for HTTP requests. The client SHALL support configurable server URL via constructor parameter.

#### Scenario: Client initialization
- **WHEN** user creates `KapiClient::new("http://localhost:8080")`
- **THEN** client initializes reqwest client with base URL

### Requirement: Client handles SSE streams for watch
The client SHALL parse Server-Sent Events (SSE) from the watch endpoint and yield `WatchEvent` objects via an async stream. The client SHALL use `eventsource-stream` or equivalent for SSE parsing.

#### Scenario: Watch stream yields events
- **WHEN** server sends SSE events with `event: message` and `data: <WatchEvent JSON>`
- **THEN** client parses each event and yields `WatchEvent` to the stream

### Requirement: Client re-exports kapi-core types
The client SHALL re-export all types from `kapi-core` (`StoredObject`, `ResourceKey`, `WatchEvent`, `ListResponse`, `ListOptions`, `ObjectMeta`, `WatchFilter`, etc.) for convenience.

#### Scenario: User imports types from client
- **WHEN** user writes `use kapi_client::StoredObject`
- **THEN** type is available without separate `kapi-core` dependency

### Requirement: Client provides list_schemas helper
The client SHALL provide a `list_schemas()` method that queries the Schema endpoint and returns a list of registered schemas. This method is used by the CLI for short name resolution.

#### Scenario: List schemas
- **WHEN** user calls `client.list_schemas()`
- **THEN** client sends GET request to `/apis/kapi.io/v1/Schema` and returns `Vec<StoredObject>`
